//! Local PDF export for the effective document study-guide view.
//!
//! Export is read-only with respect to learner data. The renderer composes one
//! frozen snapshot from the immutable generated guide plus compatible active
//! learner overlays, writes selectable text through the already-pinned pure-Rust
//! `oxidize-pdf` backend, and registers the result with the existing prepared-
//! export boundary. It never calls an AI provider or scrapes the React DOM.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use oxidize_pdf::{
    annotations::{HighlightMode, LinkAction, LinkAnnotation, LinkDestination},
    geometry::{Point, Rectangle},
    graphics::{Color, Image as PdfImage},
    structure::{Destination, NamedDestinations, OutlineItem, OutlineTree, PageDestination},
    Document, Font, Page,
};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    document_domain::SourceLocator,
    document_store,
    document_study_guides::{
        load_active_document_study_guide, DocumentStudyGuide, DocumentStudyGuideSection,
        StudyGuideOverlay, StudyGuideOverlayKind, StudyGuideOverlayState, StudyGuideSectionContent,
    },
    document_visual_evidence,
    error::CommandError,
    external_files::{PreparedExportDto, RegisterPreparedExport},
    paths::AppPaths,
    study_materials::{load_collection_study_guide, CollectionStudyGuide},
    util::validate_entity_id,
};

mod math_layout;
mod math_source;

const EXPORT_PURPOSE: &str = "study_guide_pdf";
const PAGE_WIDTH: f64 = 595.0;
const PAGE_HEIGHT: f64 = 842.0;
const MARGIN_X: f64 = 54.0;
const MARGIN_TOP: f64 = 64.0;
const MARGIN_BOTTOM: f64 = 48.0;
const BODY_SIZE: f64 = 10.25;
const BODY_LINE_HEIGHT: f64 = 14.25;
const MAX_EXPORT_SECTIONS: usize = 512;
const MAX_EXPORT_TEXT_CHARS: usize = 5_000_000;
const EMBEDDED_FONT_REGULAR_NAME: &str = "OStudyGuideSans";
const EMBEDDED_FONT_BOLD_NAME: &str = "OStudyGuideSansBold";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PrepareStudyGuidePdfInput {
    pub document_id: String,
    pub operation_id: String,
    #[serde(default)]
    pub include_annotations: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PrepareCollectionStudyGuidePdfInput {
    pub collection_id: String,
    pub operation_id: String,
    #[serde(default)]
    pub include_annotations: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StudyGuidePdfSummary {
    guide_revision: i64,
    section_count: i64,
    annotation_count: i64,
    #[serde(default = "legacy_math_fallback")]
    math_source_fallback: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedStudyGuidePdfDto {
    pub prepared_export: PreparedExportDto,
    pub guide_revision: i64,
    pub section_count: i64,
    pub annotation_count: i64,
    pub math_source_fallback: bool,
}

#[derive(Clone, Debug)]
struct EffectiveGuideExport {
    document_title: String,
    source_filename: String,
    guide_revision: i64,
    overview: String,
    learning_objectives: Vec<String>,
    key_concepts: Vec<String>,
    guide_notes: Vec<String>,
    chapters: Vec<EffectiveExportChapter>,
    sections: Vec<EffectiveExportSection>,
    annotation_count: usize,
}

#[derive(Clone, Debug)]
struct EffectiveExportChapter {
    title: String,
    introduction: String,
    section_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
struct EffectiveExportSection {
    title: String,
    source_locator: Option<SourceLocator>,
    body_lines: Vec<ExportBlock>,
    notes: Vec<String>,
    figures: Vec<ExportFigure>,
}

#[derive(Clone, Debug)]
enum InlinePiece {
    Text { value: String, width: f64 },
    Formula(math_layout::MathLayout),
}

#[derive(Clone, Debug, Default)]
struct InlineLine {
    pieces: Vec<InlinePiece>,
    width: f64,
    height: f64,
    depth: f64,
}

#[derive(Clone, Debug)]
enum FlowLine {
    Inline(InlineLine),
    Block(math_layout::MathLayout),
}

#[derive(Clone, Debug)]
struct ExportFigure {
    image: PdfImage,
    title: String,
    caption: String,
}

#[derive(Clone, Debug)]
enum ExportBlock {
    Heading(String),
    Paragraph(String),
    Bullets(Vec<String>),
    Numbered(Vec<String>),
    Terms(Vec<(String, String)>),
    Callout {
        label: String,
        title: String,
        body: String,
    },
    Table {
        title: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        caption: String,
    },
    WorkedExample {
        title: String,
        prompt: String,
        steps: Vec<String>,
        answer: String,
        takeaway: String,
    },
    RetrievalCheck {
        prompt: String,
        answer: String,
    },
    Spacer,
}

#[derive(Clone, Debug)]
enum PdfFontChoice {
    Custom { regular: String, bold: String },
}

impl PdfFontChoice {
    fn font(&self, bold: bool) -> Font {
        match self {
            Self::Custom {
                regular,
                bold: bold_name,
            } => Font::Custom(if bold { bold_name } else { regular }.clone()),
        }
    }
}

fn color_ink() -> Color {
    Color::rgb(0.10, 0.14, 0.18)
}
fn color_muted() -> Color {
    Color::rgb(0.38, 0.43, 0.48)
}
fn color_accent() -> Color {
    Color::rgb(0.08, 0.42, 0.45)
}
fn color_cover() -> Color {
    Color::rgb(0.07, 0.18, 0.25)
}
fn color_cover_text() -> Color {
    Color::rgb(0.96, 0.98, 0.99)
}
fn color_cover_muted() -> Color {
    Color::rgb(0.72, 0.81, 0.84)
}

#[tauri::command]
pub(crate) fn prepare_document_study_guide_pdf(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareStudyGuidePdfInput,
) -> Result<PreparedStudyGuidePdfDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    validate_entity_id(&input.operation_id, "study-guide export operation")?;

    if let Some((prepared_export, summary)) = database.with_connection(|connection| {
        crate::external_files::load_prepared_export_summary::<StudyGuidePdfSummary>(
            connection,
            paths.inner(),
            &input.operation_id,
            EXPORT_PURPOSE,
        )
    })? {
        return Ok(PreparedStudyGuidePdfDto {
            prepared_export,
            guide_revision: summary.guide_revision,
            section_count: summary.section_count,
            annotation_count: summary.annotation_count,
            math_source_fallback: summary.math_source_fallback,
        });
    }

    let effective = load_effective_export(
        database.inner(),
        paths.inner(),
        &input.document_id,
        input.include_annotations,
    )?;
    if effective.sections.len() > MAX_EXPORT_SECTIONS {
        return Err(CommandError::new(
            "study_guide_export_too_large",
            "The study guide contains too many sections to export safely.",
        ));
    }
    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "pdf");
    let destination = paths.resolve_relative(&relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    render_study_guide_pdf(&effective, &destination)?;

    let summary = StudyGuidePdfSummary {
        guide_revision: effective.guide_revision,
        section_count: i64::try_from(effective.sections.len()).unwrap_or(i64::MAX),
        annotation_count: i64::try_from(effective.annotation_count).unwrap_or(i64::MAX),
        math_source_fallback: math_source::requires_source_fallback(&effective),
    };
    let suggested_filename = format!(
        "{}-study-guide.pdf",
        portable_filename(&effective.document_title)
    );
    let prepared_export = database.with_connection(|connection| {
        crate::external_files::register_prepared_export(
            connection,
            paths.inner(),
            RegisterPreparedExport {
                id: &input.operation_id,
                purpose: EXPORT_PURPOSE,
                suggested_filename: &suggested_filename,
                media_type: "application/pdf",
                relative_path: &relative,
                summary: &summary,
            },
        )
    })?;

    Ok(PreparedStudyGuidePdfDto {
        prepared_export,
        guide_revision: summary.guide_revision,
        section_count: summary.section_count,
        annotation_count: summary.annotation_count,
        math_source_fallback: summary.math_source_fallback,
    })
}

#[tauri::command]
pub(crate) fn prepare_collection_study_guide_pdf(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: PrepareCollectionStudyGuidePdfInput,
) -> Result<PreparedStudyGuidePdfDto, CommandError> {
    validate_entity_id(&input.collection_id, "document collection")?;
    validate_entity_id(
        &input.operation_id,
        "collection study-guide export operation",
    )?;

    if let Some((prepared_export, summary)) = database.with_connection(|connection| {
        crate::external_files::load_prepared_export_summary::<StudyGuidePdfSummary>(
            connection,
            paths.inner(),
            &input.operation_id,
            EXPORT_PURPOSE,
        )
    })? {
        return Ok(PreparedStudyGuidePdfDto {
            prepared_export,
            guide_revision: summary.guide_revision,
            section_count: summary.section_count,
            annotation_count: summary.annotation_count,
            math_source_fallback: summary.math_source_fallback,
        });
    }

    let guide =
        load_collection_study_guide(database.inner(), &input.collection_id)?.ok_or_else(|| {
            CommandError::new(
                "collection_study_guide_not_found",
                "Create the collection study guide before exporting it to PDF.",
            )
        })?;
    let effective = compose_effective_collection_export(
        database.inner(),
        paths.inner(),
        guide,
        input.include_annotations,
    )?;
    if effective.sections.len() > MAX_EXPORT_SECTIONS {
        return Err(CommandError::new(
            "study_guide_export_too_large",
            "The collection study guide contains too many sections to export safely.",
        ));
    }
    let relative = AppPaths::prepared_export_relative_path(&input.operation_id, "pdf");
    let destination = paths.resolve_relative(&relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(CommandError::io)?;
    }
    render_study_guide_pdf(&effective, &destination)?;

    let summary = StudyGuidePdfSummary {
        guide_revision: effective.guide_revision,
        section_count: i64::try_from(effective.sections.len()).unwrap_or(i64::MAX),
        annotation_count: i64::try_from(effective.annotation_count).unwrap_or(i64::MAX),
        math_source_fallback: math_source::requires_source_fallback(&effective),
    };
    let suggested_filename = format!(
        "{}-study-guide.pdf",
        portable_filename(&effective.document_title)
    );
    let prepared_export = database.with_connection(|connection| {
        crate::external_files::register_prepared_export(
            connection,
            paths.inner(),
            RegisterPreparedExport {
                id: &input.operation_id,
                purpose: EXPORT_PURPOSE,
                suggested_filename: &suggested_filename,
                media_type: "application/pdf",
                relative_path: &relative,
                summary: &summary,
            },
        )
    })?;
    Ok(PreparedStudyGuidePdfDto {
        prepared_export,
        guide_revision: summary.guide_revision,
        section_count: summary.section_count,
        annotation_count: summary.annotation_count,
        math_source_fallback: summary.math_source_fallback,
    })
}

// Old prepared exports did not record math presentation. Warn conservatively
// on replay rather than claiming they contain typeset equations.
fn legacy_math_fallback() -> bool {
    true
}

fn load_effective_export(
    database: &Database,
    paths: &AppPaths,
    document_id: &str,
    include_annotations: bool,
) -> Result<EffectiveGuideExport, CommandError> {
    let guide = load_active_document_study_guide(database, document_id)?.ok_or_else(|| {
        CommandError::new(
            "document_study_guide_not_found",
            "Create a study guide before exporting it to PDF.",
        )
    })?;
    let (document_title, source_filename) = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT title, original_filename FROM documents WHERE id = ?1",
                [document_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_not_found",
                    "That document is no longer in the library.",
                )
            })
    })?;
    compose_effective_export(
        database,
        paths,
        document_id,
        document_title,
        source_filename,
        guide,
        include_annotations,
    )
}

fn compose_effective_export(
    database: &Database,
    paths: &AppPaths,
    document_id: &str,
    document_title: String,
    source_filename: String,
    guide: DocumentStudyGuide,
    include_annotations: bool,
) -> Result<EffectiveGuideExport, CommandError> {
    let mut guide_notes = Vec::new();
    let mut overlays_by_topic = HashMap::<String, Vec<&StudyGuideOverlay>>::new();
    for overlay in &guide.overlays {
        if overlay.state != StudyGuideOverlayState::Active {
            continue;
        }
        if let Some(topic_id) = overlay.topic_id.as_ref() {
            overlays_by_topic
                .entry(topic_id.clone())
                .or_default()
                .push(overlay);
        } else if include_annotations
            && overlay.overlay_kind == StudyGuideOverlayKind::GuideNote
            && !overlay.plain_text.trim().is_empty()
        {
            guide_notes.push(overlay.plain_text.trim().to_string());
        }
    }

    let mut sections = Vec::with_capacity(guide.sections.len());
    let mut annotation_count = guide_notes.len();
    for section in &guide.sections {
        let mut title = section.title.clone();
        let mut body_override = None::<String>;
        let mut notes = Vec::new();
        for overlay in overlays_by_topic
            .get(&section.topic_id)
            .into_iter()
            .flatten()
            .copied()
        {
            match overlay.overlay_kind {
                StudyGuideOverlayKind::TitleOverride if !overlay.plain_text.trim().is_empty() => {
                    title = overlay.plain_text.trim().to_string();
                }
                StudyGuideOverlayKind::BodyOverride if !overlay.plain_text.trim().is_empty() => {
                    body_override = Some(overlay.plain_text.trim().to_string());
                }
                StudyGuideOverlayKind::TopicNote | StudyGuideOverlayKind::Highlight
                    if include_annotations && !overlay.plain_text.trim().is_empty() =>
                {
                    notes.push(overlay.plain_text.trim().to_string());
                    annotation_count = annotation_count.saturating_add(1);
                }
                _ => {}
            }
        }
        let figures = if body_override.is_none() {
            load_export_figures(
                database,
                paths,
                document_id,
                &section.source_locator,
                &section.content.visual_explanations,
            )
        } else {
            Vec::new()
        };
        sections.push(EffectiveExportSection {
            title,
            source_locator: Some(section.source_locator.clone()),
            body_lines: body_override.map_or_else(
                || generated_section_blocks(section, figures.is_empty()),
                |replacement| vec![ExportBlock::Paragraph(replacement)],
            ),
            notes,
            figures,
        });
    }

    let chapters = if let Some(editorial) = guide.editorial.as_ref() {
        editorial
            .chapters
            .iter()
            .map(|chapter| {
                let section_indices = chapter
                    .topics
                    .iter()
                    .map(|topic| {
                        guide
                            .sections
                            .iter()
                            .position(|section| section.topic_id == topic.topic_id)
                            .ok_or_else(|| {
                                CommandError::new(
                                    "document_study_guide_invalid",
                                    "The study-guide editorial organization no longer matches its source-grounded sections.",
                                )
                            })
                    })
                    .collect::<Result<Vec<_>, CommandError>>()?;
                Ok(EffectiveExportChapter {
                    title: chapter.title.clone(),
                    introduction: chapter.introduction.clone(),
                    section_indices,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?
    } else {
        Vec::new()
    };
    let guide_title = guide
        .editorial
        .as_ref()
        .map(|editorial| editorial.title.clone())
        .unwrap_or(document_title);
    let export = EffectiveGuideExport {
        document_title: guide_title,
        source_filename,
        guide_revision: guide.revision,
        overview: guide.overview.summary,
        learning_objectives: guide.overview.learning_objectives,
        key_concepts: guide.overview.key_concepts,
        guide_notes,
        chapters,
        sections,
        annotation_count,
    };
    let character_count = effective_text(&export).chars().count();
    if character_count == 0 || character_count > MAX_EXPORT_TEXT_CHARS {
        return Err(CommandError::new(
            "study_guide_export_too_large",
            "The effective study guide is empty or exceeds the bounded PDF export size.",
        ));
    }
    Ok(export)
}

fn compose_effective_collection_export(
    database: &Database,
    paths: &AppPaths,
    guide: CollectionStudyGuide,
    include_annotations: bool,
) -> Result<EffectiveGuideExport, CommandError> {
    let mut guide_notes = Vec::new();
    let mut overlays_by_block = HashMap::<String, Vec<_>>::new();
    for overlay in &guide.overlays {
        if overlay.state != "active" {
            continue;
        }
        if let Some(block_key) = overlay.block_key.as_ref() {
            overlays_by_block
                .entry(block_key.clone())
                .or_default()
                .push(overlay);
        } else if include_annotations
            && overlay.overlay_kind == "guide_note"
            && !overlay.plain_text.trim().is_empty()
        {
            guide_notes.push(overlay.plain_text.trim().to_string());
        }
    }

    let mut sections = Vec::with_capacity(guide.content.sections.len());
    let mut annotation_count = guide_notes.len();
    for section in &guide.content.sections {
        let mut title = section.title.clone();
        let mut body_override = None::<String>;
        let mut notes = Vec::new();
        for overlay in overlays_by_block
            .get(&section.block_key)
            .into_iter()
            .flatten()
            .copied()
        {
            match overlay.overlay_kind.as_str() {
                "title_override" if !overlay.plain_text.trim().is_empty() => {
                    title = overlay.plain_text.trim().to_string();
                }
                "body_override" if !overlay.plain_text.trim().is_empty() => {
                    body_override = Some(overlay.plain_text.trim().to_string());
                }
                "topic_note" | "highlight"
                    if include_annotations && !overlay.plain_text.trim().is_empty() =>
                {
                    notes.push(overlay.plain_text.trim().to_string());
                    annotation_count = annotation_count.saturating_add(1);
                }
                _ => {}
            }
        }
        let figures = if body_override.is_none() {
            match (
                section.source_document_id.as_deref(),
                section.source_locator.as_ref(),
            ) {
                (Some(document_id), Some(locator)) => load_export_figures(
                    database,
                    paths,
                    document_id,
                    locator,
                    &section.content.visual_explanations,
                ),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        sections.push(EffectiveExportSection {
            title,
            source_locator: section.source_locator.clone(),
            body_lines: body_override.map_or_else(
                || generated_content_blocks(&section.content, figures.is_empty()),
                |replacement| vec![ExportBlock::Paragraph(replacement)],
            ),
            notes,
            figures,
        });
    }
    let export = EffectiveGuideExport {
        document_title: guide.title.clone(),
        source_filename: format!("Document collection · {}", guide.title),
        guide_revision: guide.revision,
        overview: guide.content.overview.summary,
        learning_objectives: guide.content.overview.learning_objectives,
        key_concepts: guide.content.overview.key_concepts,
        guide_notes,
        chapters: Vec::new(),
        sections,
        annotation_count,
    };
    let character_count = effective_text(&export).chars().count();
    if character_count == 0 || character_count > MAX_EXPORT_TEXT_CHARS {
        return Err(CommandError::new(
            "study_guide_export_too_large",
            "The effective collection study guide is empty or exceeds the bounded PDF export size.",
        ));
    }
    Ok(export)
}

fn load_export_figures(
    database: &Database,
    paths: &AppPaths,
    document_id: &str,
    locator: &SourceLocator,
    explanations: &[crate::document_study_guides::StudyGuideVisualExplanation],
) -> Vec<ExportFigure> {
    if explanations.is_empty() || locator.asset_ids.is_empty() {
        return Vec::new();
    }
    let assets = match document_store::load_source_assets(database, document_id) {
        Ok(assets) => assets,
        Err(error) => {
            crate::logging::warn_target(
                "ostudy.study_guide.pdf",
                "study_guide_pdf_figure_assets_unavailable",
                "Optional study-guide figures were omitted; text export remains available.",
                serde_json::json!({"documentId":document_id,"code":error.code}),
            );
            return Vec::new();
        }
    };
    let selected =
        document_store::select_source_assets(&locator.asset_ids, explanations.len().min(2));
    selected
        .into_iter()
        .zip(explanations.iter())
        .filter_map(|(asset_id, explanation)| {
            let asset = assets.iter().find(|asset| asset.asset_id == asset_id)?;
            let bytes = match document_visual_evidence::read_source_image_bytes(paths, asset) {
                Ok(bytes) => bytes,
                Err(_) => return None,
            };
            let decoded = image::load_from_memory(&bytes).ok()?.to_rgba8();
            let width = decoded.width();
            let height = decoded.height();
            let image = PdfImage::from_rgba_data(decoded.into_raw(), width, height).ok()?;
            Some(ExportFigure {
                image,
                title: explanation.title.clone(),
                caption: explanation.explanation.clone(),
            })
        })
        .collect()
}

fn generated_section_blocks(
    section: &DocumentStudyGuideSection,
    include_visual_text: bool,
) -> Vec<ExportBlock> {
    generated_content_blocks(&section.content, include_visual_text)
}

fn generated_content_blocks(
    content: &StudyGuideSectionContent,
    include_visual_text: bool,
) -> Vec<ExportBlock> {
    let mut blocks = vec![ExportBlock::Paragraph(content.overview.clone())];
    if !content.key_points.is_empty() {
        blocks.push(ExportBlock::Heading("Key points".to_string()));
        blocks.push(ExportBlock::Bullets(content.key_points.clone()));
    }
    for explanation in &content.explanations {
        blocks.push(ExportBlock::Heading(explanation.heading.clone()));
        blocks.push(ExportBlock::Paragraph(explanation.body.clone()));
    }
    if !content.important_terms.is_empty() {
        blocks.push(ExportBlock::Heading("Important terms".to_string()));
        blocks.push(ExportBlock::Terms(
            content
                .important_terms
                .iter()
                .map(|term| (term.term.clone(), term.explanation.clone()))
                .collect(),
        ));
    }
    if !content.process_steps.is_empty() {
        blocks.push(ExportBlock::Heading("Process".to_string()));
        blocks.push(ExportBlock::Numbered(content.process_steps.clone()));
    }
    if !content.comparisons.is_empty() {
        blocks.push(ExportBlock::Heading("Comparisons".to_string()));
        blocks.push(ExportBlock::Bullets(
            content
                .comparisons
                .iter()
                .map(|comparison| {
                    format!(
                        "{} vs. {} — {}",
                        comparison.subject_a, comparison.subject_b, comparison.relationship
                    )
                })
                .collect(),
        ));
    }
    if !content.common_misconceptions.is_empty() {
        blocks.push(ExportBlock::Heading("Common misconceptions".to_string()));
        blocks.push(ExportBlock::Bullets(content.common_misconceptions.clone()));
    }
    for callout in &content.callouts {
        blocks.push(ExportBlock::Callout {
            label: match callout.kind {
                crate::document_study_guides::StudyGuideCalloutKind::KeyIdea => "Key idea",
                crate::document_study_guides::StudyGuideCalloutKind::Warning => "Watch out",
                crate::document_study_guides::StudyGuideCalloutKind::Tip => "Study tip",
                crate::document_study_guides::StudyGuideCalloutKind::Note => "Note",
            }
            .to_string(),
            title: callout.title.clone(),
            body: callout.body.clone(),
        });
    }
    for table in &content.tables {
        blocks.push(ExportBlock::Table {
            title: table.title.clone(),
            columns: table.columns.clone(),
            rows: table.rows.clone(),
            caption: table.caption.clone(),
        });
    }
    for example in &content.worked_examples {
        blocks.push(ExportBlock::WorkedExample {
            title: example.title.clone(),
            prompt: example.prompt.clone(),
            steps: example.steps.clone(),
            answer: example.answer.clone(),
            takeaway: example.takeaway.clone(),
        });
    }
    for check in &content.retrieval_checks {
        blocks.push(ExportBlock::RetrievalCheck {
            prompt: check.prompt.clone(),
            answer: check.answer.clone(),
        });
    }
    if include_visual_text && !content.visual_explanations.is_empty() {
        blocks.push(ExportBlock::Heading("Visual evidence".to_string()));
        for visual in &content.visual_explanations {
            blocks.push(ExportBlock::Heading(visual.title.clone()));
            blocks.push(ExportBlock::Paragraph(visual.explanation.clone()));
        }
    }
    blocks.push(ExportBlock::Spacer);
    blocks
}

fn render_study_guide_pdf(
    export: &EffectiveGuideExport,
    destination: &Path,
) -> Result<(), CommandError> {
    let mut document = Document::new();
    document.set_title(&export.document_title);
    let font_choice = install_unicode_font(&mut document)?;
    let mut writer = PdfFlowWriter::new(document, font_choice);

    writer.cover_page(
        &export.document_title,
        &export.source_filename,
        export.sections.len(),
    )?;
    writer.page_break()?;
    writer.enable_running_pages(&export.document_title)?;

    writer.register_navigation("contents", "Contents");
    writer.heading("Contents", 20.0, 25.0)?;
    writer.muted_paragraph(
        "A source-grounded map of the topics in this guide.",
        10.0,
        14.0,
    )?;
    writer.spacer(5.0)?;
    writer.toc_entry("Overview", None, "overview")?;
    if export.chapters.is_empty() {
        for (index, section) in export.sections.iter().enumerate() {
            let destination = format!("section-{}", index + 1);
            writer.toc_entry(
                &format!("{}. {}", index + 1, section.title),
                section.source_locator.as_ref(),
                &destination,
            )?;
        }
    } else {
        for (index, chapter) in export.chapters.iter().enumerate() {
            writer.toc_entry(
                &format!("{}. {}", index + 1, chapter.title),
                None,
                &format!("chapter-{}", index + 1),
            )?;
        }
    }
    writer.spacer(12.0)?;
    writer.rule()?;
    writer.spacer(10.0)?;

    writer.ensure_space(80.0)?;
    writer.register_navigation("overview", "Overview");
    writer.heading("Overview", 20.0, 25.0)?;
    writer.paragraph(&export.overview, BODY_SIZE, BODY_LINE_HEIGHT, false)?;
    if !export.learning_objectives.is_empty() {
        writer.heading("Learning objectives", 14.0, 19.0)?;
        writer.bullets(&export.learning_objectives, false)?;
    }
    if !export.key_concepts.is_empty() {
        writer.heading("Key concepts", 14.0, 19.0)?;
        writer.bullets(&export.key_concepts, false)?;
    }
    if !export.guide_notes.is_empty() {
        writer.heading("My guide notes", 14.0, 19.0)?;
        writer.bullets(&export.guide_notes, false)?;
    }
    if math_source::requires_source_fallback(export) {
        writer.muted_paragraph(math_source::NOTICE, 9.0, 12.5)?;
    }
    writer.page_break()?;

    if export.chapters.is_empty() {
        for (index, section) in export.sections.iter().enumerate() {
            let heading = format!("{}. {}", index + 1, section.title);
            render_export_section(
                &mut writer,
                section,
                &heading,
                18.0,
                23.0,
                Some(&format!("section-{}", index + 1)),
            )?;
        }
    } else {
        for (chapter_index, chapter) in export.chapters.iter().enumerate() {
            writer.ensure_space(100.0)?;
            let chapter_heading = format!("{}. {}", chapter_index + 1, chapter.title);
            writer.register_navigation(
                &format!("chapter-{}", chapter_index + 1),
                &chapter_heading,
            );
            writer.heading(&chapter_heading, 19.0, 24.0)?;
            writer.paragraph(
                &chapter.introduction,
                BODY_SIZE,
                BODY_LINE_HEIGHT,
                false,
            )?;
            writer.spacer(8.0)?;
            for (section_position, section_index) in chapter.section_indices.iter().enumerate() {
                let section = export.sections.get(*section_index).ok_or_else(|| {
                    CommandError::new(
                        "document_study_guide_invalid",
                        "The study-guide chapter references a missing section.",
                    )
                })?;
                let heading = format!(
                    "{}.{}. {}",
                    chapter_index + 1,
                    section_position + 1,
                    section.title
                );
                render_export_section(&mut writer, section, &heading, 14.5, 19.5, None)?;
            }
            writer.spacer(10.0)?;
        }
    }

    writer.finish(destination)
}

fn render_export_section(
    writer: &mut PdfFlowWriter,
    section: &EffectiveExportSection,
    heading: &str,
    heading_size: f64,
    heading_line_height: f64,
    navigation: Option<&str>,
) -> Result<(), CommandError> {
    writer.ensure_space(90.0)?;
    if let Some(destination) = navigation {
        writer.register_navigation(destination, heading);
    }
    writer.heading(heading, heading_size, heading_line_height)?;
    if let Some(locator) = section.source_locator.as_ref() {
        writer.muted_paragraph(&source_label(locator), 9.25, 12.5)?;
    }
    writer.spacer(5.0)?;
    for block in &section.body_lines {
        writer.block(block)?;
    }
    for figure in &section.figures {
        writer.figure(figure)?;
    }
    if !section.notes.is_empty() {
        writer.heading("My notes", 13.0, 18.0)?;
        writer.bullets(&section.notes, false)?;
    }
    writer.spacer(12.0)?;
    Ok(())
}

struct PdfFlowWriter {
    document: Document,
    font: PdfFontChoice,
    page: Page,
    page_number: usize,
    y: f64,
    page_has_content: bool,
    running_title: Option<String>,
    running_pages: bool,
    navigation: Vec<(String, String, u32)>,
    image_sequence: usize,
}

impl PdfFlowWriter {
    fn new(document: Document, font: PdfFontChoice) -> Self {
        Self {
            document,
            font,
            page: Page::a4(),
            page_number: 1,
            y: PAGE_HEIGHT - MARGIN_TOP,
            page_has_content: false,
            running_title: None,
            running_pages: false,
            navigation: Vec::new(),
            image_sequence: 0,
        }
    }

    fn cover_page(
        &mut self,
        title: &str,
        source_filename: &str,
        section_count: usize,
    ) -> Result<(), CommandError> {
        self.page
            .graphics()
            .set_fill_color(color_cover())
            .rectangle(0.0, 0.0, PAGE_WIDTH, PAGE_HEIGHT)
            .fill();
        self.page
            .graphics()
            .set_fill_color(color_accent())
            .rectangle(0.0, 0.0, PAGE_WIDTH, 72.0)
            .fill();
        self.page
            .graphics()
            .set_stroke_color(Color::rgb(0.68, 0.79, 0.82))
            .set_line_width(0.8)
            .move_to(MARGIN_X, PAGE_HEIGHT - 78.0)
            .line_to(PAGE_WIDTH - MARGIN_X, PAGE_HEIGHT - 78.0)
            .stroke();
        self.absolute_text(
            "OSTUDY  /  STUDY GUIDE",
            MARGIN_X,
            PAGE_HEIGHT - 60.0,
            9.5,
            true,
            color_cover_muted(),
        )?;
        let title_lines = wrap_text(title, PAGE_WIDTH - (2.0 * MARGIN_X), 29.0);
        let mut y = PAGE_HEIGHT - 145.0;
        for line in title_lines {
            self.absolute_text(&line, MARGIN_X, y, 29.0, true, color_cover_text())?;
            y -= 35.0;
        }
        y -= 18.0;
        self.absolute_text(
            &format!("Source: {source_filename}"),
            MARGIN_X,
            y,
            11.0,
            false,
            color_cover_muted(),
        )?;
        y -= 22.0;
        self.absolute_text(
            &format!(
                "{section_count} source-grounded section{}",
                if section_count == 1 { "" } else { "s" }
            ),
            MARGIN_X,
            y,
            10.0,
            false,
            color_cover_muted(),
        )?;
        self.absolute_text(
            "SOURCE-GROUNDED  /  LOCAL EXPORT",
            MARGIN_X,
            38.0,
            9.0,
            true,
            color_cover_text(),
        )?;
        self.page_has_content = true;
        Ok(())
    }

    fn enable_running_pages(&mut self, title: &str) -> Result<(), CommandError> {
        self.running_title = Some(title.to_string());
        self.running_pages = true;
        self.draw_running_header()
    }

    fn draw_running_header(&mut self) -> Result<(), CommandError> {
        if !self.running_pages {
            return Ok(());
        }
        if let Some(title) = self.running_title.clone() {
            self.absolute_text(
                &compact_header(&title),
                MARGIN_X,
                PAGE_HEIGHT - 31.0,
                8.5,
                true,
                color_muted(),
            )?;
        }
        self.page
            .graphics()
            .set_stroke_color(Color::rgb(0.82, 0.84, 0.86))
            .set_line_width(0.6)
            .move_to(MARGIN_X, PAGE_HEIGHT - 42.0)
            .line_to(PAGE_WIDTH - MARGIN_X, PAGE_HEIGHT - 42.0)
            .stroke();
        self.y = PAGE_HEIGHT - MARGIN_TOP;
        Ok(())
    }

    fn current_page_index(&self) -> u32 {
        u32::try_from(self.page_number.saturating_sub(1)).unwrap_or(u32::MAX)
    }

    fn register_navigation(&mut self, name: &str, title: &str) {
        let page = self.current_page_index();
        if self
            .navigation
            .iter()
            .any(|(existing, _, _)| existing == name)
        {
            return;
        }
        self.navigation
            .push((name.to_string(), title.to_string(), page));
    }

    fn toc_entry(
        &mut self,
        title: &str,
        locator: Option<&SourceLocator>,
        destination: &str,
    ) -> Result<(), CommandError> {
        let title_lines = wrap_text(title, PAGE_WIDTH - (2.0 * MARGIN_X), 10.5);
        let source = locator.map(source_label);
        let source_lines = source
            .as_deref()
            .map(|value| wrap_text(value, PAGE_WIDTH - (2.0 * MARGIN_X), 9.0))
            .unwrap_or_default();
        let needed = title_lines.len() as f64 * 14.0 + source_lines.len() as f64 * 12.0 + 9.0;
        self.ensure_space(needed)?;
        let top = self.y + 3.0;
        self.paragraph(title, 10.5, 14.0, true)?;
        if let Some(source) = source {
            self.muted_paragraph(&source, 9.0, 12.0)?;
        }
        let bottom = self.y + 2.0;
        let rect = Rectangle::new(
            Point::new(MARGIN_X, bottom),
            Point::new(PAGE_WIDTH - MARGIN_X, top),
        );
        let link = LinkAnnotation::new(
            rect,
            LinkAction::GoTo(LinkDestination::Named(destination.to_string())),
        )
        .with_highlight_mode(HighlightMode::None);
        self.page.add_annotation(link.to_annotation());
        Ok(())
    }

    fn heading(&mut self, text: &str, size: f64, line_height: f64) -> Result<(), CommandError> {
        let available_width = PAGE_WIDTH - (2.0 * MARGIN_X);
        let line_count = wrap_text(text, available_width, size).len().max(1) as f64;
        let heading_height = (line_count * line_height) + (line_height * 0.35).max(2.0);
        self.ensure_space(heading_height + (2.0 * BODY_LINE_HEIGHT))?;
        self.text_color(text, size, line_height, true, 0.0, color_accent())
    }

    fn paragraph(
        &mut self,
        text: &str,
        size: f64,
        line_height: f64,
        bold: bool,
    ) -> Result<(), CommandError> {
        self.text_color(text, size, line_height, bold, 0.0, color_ink())
    }

    fn muted_paragraph(
        &mut self,
        text: &str,
        size: f64,
        line_height: f64,
    ) -> Result<(), CommandError> {
        self.text_color(text, size, line_height, false, 0.0, color_muted())
    }

    fn rule(&mut self) -> Result<(), CommandError> {
        self.ensure_space(3.0)?;
        self.page
            .graphics()
            .set_stroke_color(Color::rgb(0.82, 0.84, 0.86))
            .set_line_width(0.6)
            .move_to(MARGIN_X, self.y)
            .line_to(PAGE_WIDTH - MARGIN_X, self.y)
            .stroke();
        self.y -= 3.0;
        Ok(())
    }

    fn bullets(&mut self, items: &[String], numbered: bool) -> Result<(), CommandError> {
        for (index, item) in items.iter().enumerate() {
            let prefix = if numbered {
                format!("{}. ", index + 1)
            } else {
                "• ".to_string()
            };
            self.text_color(
                &format!("{prefix}{item}"),
                BODY_SIZE,
                BODY_LINE_HEIGHT,
                false,
                12.0,
                color_ink(),
            )?;
        }
        Ok(())
    }

    fn block(&mut self, block: &ExportBlock) -> Result<(), CommandError> {
        match block {
            ExportBlock::Heading(text) => self.heading(text, 12.5, 17.5),
            ExportBlock::Paragraph(text) => {
                self.paragraph(text, BODY_SIZE, BODY_LINE_HEIGHT, false)
            }
            ExportBlock::Bullets(items) => self.bullets(items, false),
            ExportBlock::Numbered(items) => self.bullets(items, true),
            ExportBlock::Terms(entries) => {
                for (term, explanation) in entries {
                    self.text_color(
                        &format!("{term}: {explanation}"),
                        BODY_SIZE,
                        BODY_LINE_HEIGHT,
                        false,
                        12.0,
                        color_ink(),
                    )?;
                }
                Ok(())
            }
            ExportBlock::Callout { label, title, body } => self.callout(label, title, body),
            ExportBlock::Table {
                title,
                columns,
                rows,
                caption,
            } => self.table(title, columns, rows, caption),
            ExportBlock::WorkedExample {
                title,
                prompt,
                steps,
                answer,
                takeaway,
            } => {
                self.heading(&format!("Worked example: {title}"), 12.5, 17.5)?;
                self.paragraph(prompt, BODY_SIZE, BODY_LINE_HEIGHT, false)?;
                self.bullets(steps, true)?;
                self.paragraph(
                    &format!("Answer: {answer}"),
                    BODY_SIZE,
                    BODY_LINE_HEIGHT,
                    true,
                )?;
                if !takeaway.is_empty() {
                    self.muted_paragraph(
                        &format!("Takeaway: {takeaway}"),
                        BODY_SIZE,
                        BODY_LINE_HEIGHT,
                    )?;
                }
                Ok(())
            }
            ExportBlock::RetrievalCheck { prompt, answer } => {
                self.heading("Check yourself", 12.5, 17.5)?;
                self.paragraph(prompt, BODY_SIZE, BODY_LINE_HEIGHT, true)?;
                self.muted_paragraph(&format!("Answer: {answer}"), BODY_SIZE, BODY_LINE_HEIGHT)
            }
            ExportBlock::Spacer => self.spacer(5.0),
        }
    }

    fn figure(&mut self, figure: &ExportFigure) -> Result<(), CommandError> {
        let pixel_width = f64::from(figure.image.width().max(1));
        let pixel_height = f64::from(figure.image.height().max(1));
        let maximum_width = PAGE_WIDTH - (2.0 * MARGIN_X);
        let maximum_height = 280.0;
        let scale = (maximum_width / pixel_width)
            .min(maximum_height / pixel_height)
            .min(1.0);
        let width = (pixel_width * scale).max(1.0);
        let height = (pixel_height * scale).max(1.0);
        let caption = if figure.title.trim().is_empty() {
            figure.caption.trim().to_string()
        } else if figure.caption.trim().is_empty() {
            figure.title.trim().to_string()
        } else {
            format!("{} — {}", figure.title.trim(), figure.caption.trim())
        };
        let caption_height = if caption.is_empty() {
            0.0
        } else {
            wrap_text(&caption, maximum_width, 9.0).len() as f64 * 12.5 + 5.0
        };
        self.ensure_space(height + caption_height + 12.0)?;
        let x = MARGIN_X + ((maximum_width - width) / 2.0);
        let bottom = self.y - height;
        self.image_sequence = self.image_sequence.saturating_add(1);
        let name = format!("StudyGuideFigure{}", self.image_sequence);
        self.page.add_image(name.clone(), figure.image.clone());
        self.page
            .draw_image(&name, x, bottom, width, height)
            .map_err(pdf_error)?;
        self.y = bottom - 6.0;
        self.page_has_content = true;
        if !caption.is_empty() {
            self.muted_paragraph(&caption, 9.0, 12.5)?;
        }
        self.spacer(6.0)
    }

    fn callout(&mut self, label: &str, title: &str, body: &str) -> Result<(), CommandError> {
        let width = PAGE_WIDTH - (2.0 * MARGIN_X);
        let inner_width = width - 28.0;
        let heading = format!("{label}: {title}");
        let heading_lines = wrap_text(&heading, inner_width, 10.5);
        let body_lines = wrap_text(body, inner_width, 9.75);
        let height =
            18.0 + (heading_lines.len() as f64 * 14.0) + (body_lines.len() as f64 * 13.0) + 10.0;
        self.ensure_space(height + 8.0)?;
        let bottom = self.y - height;
        self.page
            .graphics()
            .set_fill_color(Color::rgb(0.93, 0.97, 0.97))
            .rectangle(MARGIN_X, bottom, width, height)
            .fill();
        self.page
            .graphics()
            .set_fill_color(color_accent())
            .rectangle(MARGIN_X, bottom, 4.0, height)
            .fill();
        let mut line_y = self.y - 17.0;
        for line in heading_lines {
            self.absolute_text(&line, MARGIN_X + 14.0, line_y, 10.5, true, color_accent())?;
            line_y -= 14.0;
        }
        line_y -= 2.0;
        for line in body_lines {
            self.absolute_text(&line, MARGIN_X + 14.0, line_y, 9.75, false, color_ink())?;
            line_y -= 13.0;
        }
        self.y = bottom - 8.0;
        self.page_has_content = true;
        Ok(())
    }

    fn table(
        &mut self,
        title: &str,
        columns: &[String],
        rows: &[Vec<String>],
        caption: &str,
    ) -> Result<(), CommandError> {
        if columns.len() < 2 || rows.is_empty() {
            return Ok(());
        }
        self.heading(title, 12.5, 17.5)?;
        let table_width = PAGE_WIDTH - (2.0 * MARGIN_X);
        let column_width = table_width / columns.len() as f64;
        self.draw_table_row(columns, column_width, true, false)?;
        for (index, row) in rows.iter().enumerate() {
            let height = table_row_height(row, column_width, 8.8, 11.5);
            if self.y - height < MARGIN_BOTTOM {
                self.commit_page()?;
                self.draw_table_row(columns, column_width, true, false)?;
            }
            self.draw_table_row(row, column_width, false, index % 2 == 1)?;
        }
        self.spacer(4.0)?;
        if !caption.is_empty() {
            self.muted_paragraph(caption, 9.0, 12.5)?;
        }
        Ok(())
    }

    fn draw_table_row(
        &mut self,
        cells: &[String],
        column_width: f64,
        header: bool,
        shaded: bool,
    ) -> Result<(), CommandError> {
        let font_size = if header { 8.9 } else { 8.8 };
        let line_height = 11.5;
        let height = table_row_height(cells, column_width, font_size, line_height);
        self.ensure_space(height)?;
        let top = self.y;
        let bottom = top - height;
        let fill = if header {
            Color::rgb(0.88, 0.94, 0.94)
        } else if shaded {
            Color::rgb(0.975, 0.98, 0.98)
        } else {
            Color::rgb(1.0, 1.0, 1.0)
        };
        self.page
            .graphics()
            .set_fill_color(fill)
            .rectangle(MARGIN_X, bottom, column_width * cells.len() as f64, height)
            .fill();
        let border = Color::rgb(0.78, 0.81, 0.82);
        self.page
            .graphics()
            .set_stroke_color(border)
            .set_line_width(0.45)
            .move_to(MARGIN_X, top)
            .line_to(MARGIN_X + column_width * cells.len() as f64, top)
            .move_to(MARGIN_X, bottom)
            .line_to(MARGIN_X + column_width * cells.len() as f64, bottom)
            .stroke();
        for index in 0..=cells.len() {
            let x = MARGIN_X + column_width * index as f64;
            self.page
                .graphics()
                .set_stroke_color(border)
                .set_line_width(0.45)
                .move_to(x, top)
                .line_to(x, bottom)
                .stroke();
        }
        for (index, cell) in cells.iter().enumerate() {
            let lines = wrap_text(cell, column_width - 10.0, font_size);
            let mut line_y = top - 11.0;
            for line in lines {
                self.absolute_text(
                    &line,
                    MARGIN_X + column_width * index as f64 + 5.0,
                    line_y,
                    font_size,
                    header,
                    color_ink(),
                )?;
                line_y -= line_height;
            }
        }
        self.y = bottom;
        self.page_has_content = true;
        Ok(())
    }

    fn spacer(&mut self, points: f64) -> Result<(), CommandError> {
        self.ensure_space(points)?;
        self.y -= points;
        Ok(())
    }

    fn page_break(&mut self) -> Result<(), CommandError> {
        if self.page_has_content {
            self.commit_page()?;
        }
        Ok(())
    }

    fn text_color(
        &mut self,
        text: &str,
        size: f64,
        line_height: f64,
        bold: bool,
        indent: f64,
        color: Color,
    ) -> Result<(), CommandError> {
        if math_source::contains_math(text) {
            return self.math_text(text, size, line_height, bold, indent, color);
        }
        let available_width = PAGE_WIDTH - (2.0 * MARGIN_X) - indent;
        let lines = wrap_text(text, available_width, size);
        for line in lines {
            self.ensure_space(line_height)?;
            self.page
                .text()
                .set_font(self.font.font(bold), size)
                .set_fill_color(color)
                .at(MARGIN_X + indent, self.y)
                .write(&line)
                .map_err(pdf_error)?;
            self.page_has_content = true;
            self.y -= line_height;
        }
        self.spacer((line_height * 0.35).max(2.0))
    }

    fn math_text(
        &mut self,
        text: &str,
        size: f64,
        line_height: f64,
        bold: bool,
        indent: f64,
        color: Color,
    ) -> Result<(), CommandError> {
        let maximum_width = PAGE_WIDTH - (2.0 * MARGIN_X) - indent;
        let mut flow = Vec::<FlowLine>::new();
        let mut current = InlineLine::default();
        let mut pending_space = false;

        for segment in math_source::segments(text) {
            match segment {
                math_source::MathSegment::Text(value) => {
                    let normalized = value.replace('\r', "");
                    for (line_index, line) in normalized.split('\n').enumerate() {
                        if line_index > 0 {
                            flow.push(FlowLine::Inline(std::mem::take(&mut current)));
                            pending_space = false;
                        }
                        let mut cursor = 0_usize;
                        while cursor < line.len() {
                            let rest = &line[cursor..];
                            let first = rest.chars().next().expect("nonempty text suffix");
                            let whitespace = first.is_whitespace();
                            let mut end = first.len_utf8();
                            for (offset, character) in rest[first.len_utf8()..].char_indices() {
                                if character.is_whitespace() != whitespace {
                                    end = first.len_utf8() + offset;
                                    break;
                                }
                                end = first.len_utf8() + offset + character.len_utf8();
                            }
                            let token = &rest[..end];
                            cursor += end;
                            if whitespace {
                                pending_space = true;
                                continue;
                            }
                            let space_width = if pending_space && !current.pieces.is_empty() {
                                approximate_text_width(" ", size)
                            } else {
                                0.0
                            };
                            let token_width = approximate_text_width(token, size);
                            if !current.pieces.is_empty()
                                && current.width + space_width + token_width > maximum_width
                            {
                                flow.push(FlowLine::Inline(std::mem::take(&mut current)));
                            }
                            if pending_space && !current.pieces.is_empty() {
                                let width = approximate_text_width(" ", size);
                                current.pieces.push(InlinePiece::Text {
                                    value: " ".to_string(),
                                    width,
                                });
                                current.width += width;
                            }
                            current.pieces.push(InlinePiece::Text {
                                value: token.to_string(),
                                width: token_width,
                            });
                            current.width += token_width;
                            current.height = current.height.max(size * 0.78);
                            current.depth = current.depth.max(size * 0.22);
                            pending_space = false;
                        }
                    }
                }
                math_source::MathSegment::Formula {
                    latex,
                    source,
                    block,
                } => {
                    let base_size = if block { size * 1.08 } else { size };
                    let mut formula = math_source::native_layout(latex, base_size);
                    if let Some(layout) = formula.as_ref() {
                        if layout.width > maximum_width {
                            let scale = (maximum_width / layout.width).clamp(0.62, 1.0);
                            formula = math_source::native_layout(latex, base_size * scale);
                        }
                    }
                    if block {
                        if !current.pieces.is_empty() {
                            flow.push(FlowLine::Inline(std::mem::take(&mut current)));
                        }
                        pending_space = false;
                        if let Some(layout) = formula.filter(|layout| layout.width <= maximum_width)
                        {
                            flow.push(FlowLine::Block(layout));
                        } else {
                            flow.extend(source_fallback_lines(source, size, maximum_width));
                        }
                    } else if let Some(layout) =
                        formula.filter(|layout| layout.width <= maximum_width)
                    {
                        let space_width = if pending_space && !current.pieces.is_empty() {
                            approximate_text_width(" ", size)
                        } else {
                            0.0
                        };
                        if !current.pieces.is_empty()
                            && current.width + space_width + layout.width > maximum_width
                        {
                            flow.push(FlowLine::Inline(std::mem::take(&mut current)));
                        }
                        if pending_space && !current.pieces.is_empty() {
                            let width = approximate_text_width(" ", size);
                            current.pieces.push(InlinePiece::Text {
                                value: " ".to_string(),
                                width,
                            });
                            current.width += width;
                        }
                        current.height = current.height.max(layout.height);
                        current.depth = current.depth.max(layout.depth);
                        current.width += layout.width;
                        current.pieces.push(InlinePiece::Formula(layout));
                        pending_space = false;
                    } else {
                        if !current.pieces.is_empty() {
                            flow.push(FlowLine::Inline(std::mem::take(&mut current)));
                        }
                        flow.extend(source_fallback_lines(source, size, maximum_width));
                        pending_space = false;
                    }
                }
            }
        }
        if !current.pieces.is_empty() || flow.is_empty() {
            flow.push(FlowLine::Inline(current));
        }

        for line in flow {
            match line {
                FlowLine::Inline(line) if line.pieces.is_empty() => {
                    self.ensure_space(line_height)?;
                    self.y -= line_height;
                }
                FlowLine::Inline(line) => {
                    let height = line.height.max(size * 0.78);
                    let depth = line.depth.max(size * 0.22);
                    let used = (height + depth + 2.0).max(line_height);
                    self.ensure_space(used)?;
                    let baseline = self.y - (height - (size * 0.78)).max(0.0);
                    let mut x = MARGIN_X + indent;
                    for piece in line.pieces {
                        match piece {
                            InlinePiece::Text { value, width } => {
                                self.absolute_text(&value, x, baseline, size, bold, color)?;
                                x += width;
                            }
                            InlinePiece::Formula(layout) => {
                                self.draw_math_layout(&layout, x, baseline, bold, color)?;
                                x += layout.width;
                            }
                        }
                    }
                    self.page_has_content = true;
                    self.y -= used;
                }
                FlowLine::Block(layout) => {
                    let used = layout.height + layout.depth + (line_height * 0.55);
                    self.ensure_space(used)?;
                    let x = MARGIN_X + indent + ((maximum_width - layout.width) / 2.0);
                    let baseline = self.y - layout.height;
                    self.draw_math_layout(&layout, x, baseline, bold, color)?;
                    self.page_has_content = true;
                    self.y = baseline - layout.depth - (line_height * 0.35);
                }
            }
        }
        self.spacer((line_height * 0.35).max(2.0))
    }

    fn draw_math_layout(
        &mut self,
        layout: &math_layout::MathLayout,
        x: f64,
        baseline: f64,
        bold: bool,
        color: Color,
    ) -> Result<(), CommandError> {
        for operation in &layout.ops {
            match operation {
                math_layout::DrawOp::Text {
                    x: offset_x,
                    y: offset_y,
                    text,
                    size,
                } => {
                    self.absolute_text(text, x + offset_x, baseline + offset_y, *size, bold, color)?
                }
                math_layout::DrawOp::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    width,
                } => {
                    self.page
                        .graphics()
                        .set_stroke_color(color)
                        .set_line_width(*width)
                        .move_to(x + x1, baseline + y1)
                        .line_to(x + x2, baseline + y2)
                        .stroke();
                }
            }
        }
        Ok(())
    }

    fn absolute_text(
        &mut self,
        text: &str,
        x: f64,
        y: f64,
        size: f64,
        bold: bool,
        color: Color,
    ) -> Result<(), CommandError> {
        self.page
            .text()
            .set_font(self.font.font(bold), size)
            .set_fill_color(color)
            .at(x, y)
            .write(text)
            .map_err(pdf_error)?;
        Ok(())
    }

    fn ensure_space(&mut self, needed: f64) -> Result<(), CommandError> {
        if self.y - needed < MARGIN_BOTTOM {
            self.commit_page()?;
        }
        Ok(())
    }

    fn commit_page(&mut self) -> Result<(), CommandError> {
        if self.page_has_content && self.running_pages {
            let footer = format!("oStudy  •  Study guide  •  Page {}", self.page_number);
            self.absolute_text(&footer, MARGIN_X, 24.0, 8.0, false, color_muted())?;
        }
        let next = Page::a4();
        let finished = std::mem::replace(&mut self.page, next);
        self.document.add_page(finished);
        self.page_number = self.page_number.saturating_add(1);
        self.y = PAGE_HEIGHT - MARGIN_TOP;
        self.page_has_content = false;
        self.draw_running_header()?;
        Ok(())
    }

    fn finish(mut self, destination: &Path) -> Result<(), CommandError> {
        if self.page_has_content {
            self.commit_page()?;
        }
        if !self.navigation.is_empty() {
            let mut named = NamedDestinations::new();
            let mut outline = OutlineTree::new();
            for (name, title, page) in &self.navigation {
                let target = Destination::fit(PageDestination::PageNumber(*page));
                named.add_destination(name.clone(), target.to_array());
                outline.add_item(
                    OutlineItem::new(title.clone())
                        .with_destination(Destination::fit(PageDestination::PageNumber(*page))),
                );
            }
            self.document.set_named_destinations(named);
            self.document.set_outline(outline);
        }
        self.document.save(destination).map_err(pdf_error)
    }
}

fn table_row_height(cells: &[String], column_width: f64, font_size: f64, line_height: f64) -> f64 {
    let lines = cells
        .iter()
        .map(|cell| wrap_text(cell, column_width - 10.0, font_size).len())
        .max()
        .unwrap_or(1);
    (lines as f64 * line_height) + 10.0
}

fn compact_header(value: &str) -> String {
    let clean = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.chars().count() <= 72 {
        return clean;
    }
    let mut result = clean.chars().take(69).collect::<String>();
    result.push_str("...");
    result
}

fn install_unicode_font(document: &mut Document) -> Result<PdfFontChoice, CommandError> {
    let Some((regular_path, bold_path)) = find_unicode_fonts() else {
        return Err(CommandError::new(
            "study_guide_pdf_font_missing",
            "oStudy could not find local Unicode TrueType/OpenType fonts for study-guide PDF export.",
        ));
    };
    let regular_bytes = fs::read(&regular_path).map_err(CommandError::io)?;
    let bold_bytes = fs::read(&bold_path).map_err(CommandError::io)?;
    document
        .add_font_from_bytes(EMBEDDED_FONT_REGULAR_NAME, regular_bytes)
        .map_err(pdf_error)?;
    document
        .add_font_from_bytes(EMBEDDED_FONT_BOLD_NAME, bold_bytes)
        .map_err(pdf_error)?;
    Ok(PdfFontChoice::Custom {
        regular: EMBEDDED_FONT_REGULAR_NAME.to_string(),
        bold: EMBEDDED_FONT_BOLD_NAME.to_string(),
    })
}

fn find_unicode_fonts() -> Option<(PathBuf, PathBuf)> {
    let pairs = [
        (
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
        ),
        (
            "/usr/share/fonts/opentype/noto/NotoSans-Regular.otf",
            "/usr/share/fonts/opentype/noto/NotoSans-Bold.otf",
        ),
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        ),
        (
            "/system/fonts/NotoSans-Regular.ttf",
            "/system/fonts/NotoSans-Bold.ttf",
        ),
        (
            "/system/fonts/Roboto-Regular.ttf",
            "/system/fonts/Roboto-Bold.ttf",
        ),
        (
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\arialbd.ttf",
        ),
    ];
    for (regular, bold) in pairs {
        let regular = PathBuf::from(regular);
        let bold = PathBuf::from(bold);
        if regular.is_file() && bold.is_file() {
            return Some((regular, bold));
        }
    }
    [
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
    .map(|path| (path.clone(), path))
}

fn source_fallback_lines(source: &str, size: f64, maximum_width: f64) -> Vec<FlowLine> {
    let approximate_character_width = (size * 0.53).max(1.0);
    let maximum = (maximum_width / approximate_character_width)
        .floor()
        .max(8.0) as usize;
    math_source::wrap_source(source, maximum)
        .into_iter()
        .map(|value| {
            let width = approximate_text_width(&value, size);
            FlowLine::Inline(InlineLine {
                pieces: vec![InlinePiece::Text { value, width }],
                width,
                height: size * 0.78,
                depth: size * 0.22,
            })
        })
        .collect()
}

fn approximate_text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|character| {
            if character.is_whitespace() {
                size * 0.32
            } else if character.is_ascii_punctuation() {
                size * 0.42
            } else {
                size * 0.53
            }
        })
        .sum()
}

fn wrap_text(text: &str, available_width: f64, font_size: f64) -> Vec<String> {
    let approximate_character_width = (font_size * 0.53).max(1.0);
    let maximum = (available_width / approximate_character_width)
        .floor()
        .max(8.0) as usize;
    if math_source::contains_math(text) {
        return math_source::wrap_source(text, maximum);
    }
    let mut lines = Vec::new();
    for paragraph in text.replace('\r', "").split('\n') {
        if paragraph.trim().is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if current.is_empty() && word.chars().count() > maximum {
                split_long_word(word, maximum, &mut lines, &mut current);
                continue;
            }
            let proposed = current
                .chars()
                .count()
                .saturating_add(usize::from(!current.is_empty()))
                .saturating_add(word.chars().count());
            if proposed > maximum && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn split_long_word(word: &str, maximum: usize, lines: &mut Vec<String>, current: &mut String) {
    let mut chunk = String::new();
    for character in word.chars() {
        chunk.push(character);
        if chunk.chars().count() >= maximum {
            lines.push(std::mem::take(&mut chunk));
        }
    }
    *current = chunk;
}

fn source_label(locator: &SourceLocator) -> String {
    let label = if locator.unit_numbers.len() == 1 {
        locator.unit_kind.singular_label()
    } else {
        locator.unit_kind.plural_label()
    };
    match (locator.unit_numbers.first(), locator.unit_numbers.last()) {
        (Some(first), Some(last)) if first == last => format!("Source {label} {first}"),
        (Some(first), Some(last)) => format!("Source {label} {first}–{last}"),
        _ => format!("Source {}", locator.unit_kind.plural_label()),
    }
}

fn portable_filename(value: &str) -> String {
    let mut result = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    result = result.trim().trim_matches('.').trim().to_string();
    if result.is_empty() {
        "study-guide".to_string()
    } else {
        result.chars().take(120).collect()
    }
}

fn effective_text(export: &EffectiveGuideExport) -> String {
    let mut text = format!(
        "{}\n{}\n{}\n",
        export.document_title, export.source_filename, export.overview
    );
    text.push_str(&export.learning_objectives.join("\n"));
    text.push_str(&export.key_concepts.join("\n"));
    text.push_str(&export.guide_notes.join("\n"));
    for chapter in &export.chapters {
        text.push_str(&chapter.title);
        text.push_str(&chapter.introduction);
    }
    for section in &export.sections {
        text.push_str(&section.title);
        for block in &section.body_lines {
            text.push_str(&format!("{block:?}"));
        }
        text.push_str(&section.notes.join("\n"));
    }
    text
}

fn pdf_error(error: impl std::fmt::Display) -> CommandError {
    CommandError::new(
        "study_guide_pdf_export_failed",
        format!("Could not generate the study-guide PDF: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    use crate::document_domain::SourceUnitKind;
    use oxidize_pdf::parser::{PdfDocument, PdfReader};

    fn test_export() -> EffectiveGuideExport {
        EffectiveGuideExport {
            document_title: "Cardiovascular Physiology".to_string(),
            source_filename: "lecture.pdf".to_string(),
            guide_revision: 2,
            overview: "Understand preload and cardiac output.".to_string(),
            learning_objectives: vec!["Explain preload.".to_string()],
            key_concepts: vec!["Frank-Starling mechanism".to_string()],
            guide_notes: vec!["Review this before lab.".to_string()],
            chapters: Vec::new(),
            sections: vec![EffectiveExportSection {
                title: "Preload".to_string(),
                source_locator: Some(
                    SourceLocator::new(SourceUnitKind::Page, vec![2, 3], None).unwrap(),
                ),
                body_lines: vec![ExportBlock::Paragraph(
                    "Preload reflects ventricular filling before systole.".to_string(),
                )],
                notes: vec!["Compare with afterload.".to_string()],
                figures: Vec::new(),
            }],
            annotation_count: 2,
        }
    }

    #[test]
    fn rich_semantic_blocks_survive_generated_export_projection() {
        use crate::document_study_guides::{
            StudyGuideCallout, StudyGuideCalloutKind, StudyGuideRetrievalCheck, StudyGuideTable,
            StudyGuideWorkedExample,
        };
        let mut content = crate::document_study_guides::StudyGuideSectionContent {
            overview: "Overview".to_string(),
            ..Default::default()
        };
        content.callouts.push(StudyGuideCallout {
            kind: StudyGuideCalloutKind::KeyIdea,
            title: "Remember".to_string(),
            body: "Preload reflects filling.".to_string(),
        });
        content.tables.push(StudyGuideTable {
            title: "Compare".to_string(),
            columns: vec!["Concept".to_string(), "Meaning".to_string()],
            rows: vec![vec!["Preload".to_string(), "Filling".to_string()]],
            caption: "High-yield comparison".to_string(),
        });
        content.worked_examples.push(StudyGuideWorkedExample {
            title: "Trace a change".to_string(),
            prompt: "Venous return rises.".to_string(),
            steps: vec!["Filling rises.".to_string()],
            answer: "Preload rises.".to_string(),
            takeaway: "Follow cause before effect.".to_string(),
        });
        content.retrieval_checks.push(StudyGuideRetrievalCheck {
            prompt: "What is preload?".to_string(),
            answer: "Ventricular filling/stretch before systole.".to_string(),
        });
        let section = DocumentStudyGuideSection {
            topic_id: "a".repeat(32),
            parent_topic_id: None,
            sort_index: 0,
            title: "Preload".to_string(),
            topic_summary: String::new(),
            content,
            content_text: String::new(),
            content_hash: "b".repeat(64),
            source_locator: SourceLocator::new(SourceUnitKind::Page, vec![2], None).unwrap(),
        };
        let blocks = generated_section_blocks(&section, true);
        let text = format!("{blocks:?}");
        for expected in ["Remember", "Compare", "Trace a change", "What is preload?"] {
            assert!(text.contains(expected));
        }
    }

    #[test]
    fn source_labels_use_natural_unit_names() {
        let page = SourceLocator::new(SourceUnitKind::Page, vec![3], None).unwrap();
        let slides = SourceLocator::new(SourceUnitKind::Slide, vec![4, 5], None).unwrap();
        assert_eq!(source_label(&page), "Source page 3");
        assert_eq!(source_label(&slides), "Source slides 4–5");
    }

    #[test]
    fn wrapping_is_deterministic_and_bounded() {
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let first = wrap_text(text, 100.0, 10.0);
        let second = wrap_text(text, 100.0, 10.0);
        assert_eq!(first, second);
        assert!(first.len() > 1);
    }

    #[test]
    fn wrapped_heading_moves_with_following_body_instead_of_splitting_at_page_bottom() {
        let mut document = Document::new();
        document.set_title("Heading pagination fixture");
        let font = install_unicode_font(&mut document).unwrap();
        let mut writer = PdfFlowWriter::new(document, font);
        writer
            .enable_running_pages("Heading pagination fixture")
            .unwrap();
        writer
            .paragraph("Earlier page content.", BODY_SIZE, BODY_LINE_HEIGHT, false)
            .unwrap();
        writer.y = MARGIN_BOTTOM + 60.0;
        let heading = "A deliberately long study-guide heading that wraps across several lines and must move as one readable unit with the explanation that immediately follows it rather than leaving part of the heading stranded on the previous page";
        writer.heading(heading, 12.5, 17.5).unwrap();
        writer
            .paragraph(
                "Following explanation stays with the wrapped heading.",
                BODY_SIZE,
                BODY_LINE_HEIGHT,
                false,
            )
            .unwrap();
        let root = std::env::temp_dir().join(format!(
            "ostudy-study-guide-heading-pagination-{}.pdf",
            std::process::id()
        ));
        writer.finish(&root).unwrap();

        let parsed =
            PdfDocument::new(PdfReader::new(Cursor::new(fs::read(&root).unwrap())).unwrap());
        assert_eq!(parsed.page_count().unwrap(), 2);
        let first = parsed.extract_text_from_page(0).unwrap().text;
        let second = parsed.extract_text_from_page(1).unwrap().text;
        assert!(first.contains("Earlier page content."));
        assert!(!first.contains("deliberately long study-guide heading"));
        assert!(second.contains("deliberately long study-guide heading"));
        assert!(second.contains("Following explanation stays with"));
        fs::remove_file(root).unwrap();
    }

    #[test]
    fn long_tables_repeat_headers_after_page_breaks() {
        let mut document = Document::new();
        document.set_title("Table fixture");
        let font = install_unicode_font(&mut document).unwrap();
        let mut writer = PdfFlowWriter::new(document, font);
        writer.enable_running_pages("Table fixture").unwrap();
        let columns = vec!["Concept".to_string(), "Meaning".to_string()];
        let rows = (0..70)
            .map(|index| {
                vec![
                    format!("Concept {index}"),
                    "A compact explanation that remains selectable.".to_string(),
                ]
            })
            .collect::<Vec<_>>();
        writer
            .table("Reference table", &columns, &rows, "")
            .unwrap();
        let root = std::env::temp_dir().join(format!(
            "ostudy-study-guide-table-{}.pdf",
            std::process::id()
        ));
        writer.finish(&root).unwrap();
        let bytes = fs::read(&root).unwrap();
        let reader = PdfReader::new(Cursor::new(bytes)).unwrap();
        let parsed = PdfDocument::new(reader);
        let page_count = parsed.page_count().unwrap();
        assert!(page_count > 1);
        for page in 0..page_count {
            let text = parsed.extract_text_from_page(page).unwrap().text;
            assert!(text.contains("Concept"));
            assert!(text.contains("Meaning"));
        }
        fs::remove_file(root).unwrap();
    }

    #[test]
    fn rendered_pdf_omits_internal_guide_revision() {
        let export = test_export();
        let root = std::env::temp_dir().join(format!(
            "ostudy-study-guide-no-revision-{}.pdf",
            std::process::id()
        ));
        render_study_guide_pdf(&export, &root).unwrap();
        let bytes = fs::read(&root).unwrap();
        let reader = PdfReader::new(Cursor::new(bytes)).unwrap();
        let parsed = PdfDocument::new(reader);
        let text = parsed.extract_text_from_page(0).unwrap().text;
        assert!(text.contains("Cardiovascular Physiology"));
        assert!(!text.contains("Guide revision"));
        assert!(!text.contains("Revision 2"));
        fs::remove_file(root).unwrap();
    }

    #[test]
    fn editorial_pdf_uses_major_chapters_for_contents_and_nested_section_headings() {
        let mut export = test_export();
        export.document_title = "Cardiovascular drugs".to_string();
        export.overview = "Cardiovascular pharmacology connects mechanisms with treatment goals and clinical safety. The guide moves from hemodynamic foundations into therapeutic pathways.".to_string();
        export.sections.push(EffectiveExportSection {
            title: "Renin–angiotensin system".to_string(),
            source_locator: Some(
                SourceLocator::new(SourceUnitKind::Page, vec![18, 19], None).unwrap(),
            ),
            body_lines: vec![ExportBlock::Paragraph(
                "Renin–angiotensin signaling links vascular tone with sodium balance.".to_string(),
            )],
            notes: Vec::new(),
            figures: Vec::new(),
        });
        export.chapters = vec![EffectiveExportChapter {
            title: "Hemodynamic foundations".to_string(),
            introduction: "These sections establish the physiologic relationships needed to understand later cardiovascular therapies.".to_string(),
            section_indices: vec![0, 1],
        }];
        let root = std::env::temp_dir().join(format!(
            "ostudy-study-guide-editorial-{}.pdf",
            std::process::id()
        ));
        render_study_guide_pdf(&export, &root).unwrap();
        let parsed =
            PdfDocument::new(PdfReader::new(Cursor::new(fs::read(&root).unwrap())).unwrap());
        assert!(parsed.page_count().unwrap() >= 3);
        let cover = parsed.extract_text_from_page(0).unwrap().text;
        let contents = parsed.extract_text_from_page(1).unwrap().text;
        let all = (0..parsed.page_count().unwrap())
            .map(|page| parsed.extract_text_from_page(page).unwrap().text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(cover.contains("Cardiovascular drugs"));
        assert!(contents.contains("1. Hemodynamic foundations"));
        assert!(!contents.contains("1.1. Preload"));
        assert!(!contents.contains("1.2. Renin"));
        assert!(all.contains("These sections establish the physiologic relationships"));
        assert!(all.contains("1.1. Preload"));
        assert!(all.contains("1.2. Renin"));
        fs::remove_file(root).unwrap();
    }

    #[test]
    fn rendered_pdf_embeds_verified_figure_shape_and_selectable_caption() {
        let mut export = test_export();
        export.sections[0].figures.push(ExportFigure {
            image: PdfImage::from_rgba_data(vec![255, 0, 0, 255], 1, 1).unwrap(),
            title: "Pressure-volume loop".to_string(),
            caption: "Trace filling before ejection.".to_string(),
        });
        let root = std::env::temp_dir().join(format!(
            "ostudy-study-guide-figure-{}.pdf",
            std::process::id()
        ));
        render_study_guide_pdf(&export, &root).unwrap();
        let bytes = fs::read(&root).unwrap();
        let raw = String::from_utf8_lossy(&bytes);
        assert!(raw.contains("/Subtype /Image"));
        let reader = PdfReader::new(Cursor::new(bytes)).unwrap();
        let parsed = PdfDocument::new(reader);
        let text = (0..parsed.page_count().unwrap())
            .map(|page| parsed.extract_text_from_page(page).unwrap().text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Pressure-volume loop"));
        assert!(text.contains("Trace filling before ejection."));
        fs::remove_file(root).unwrap();
    }

    #[test]
    fn rendered_pdf_contains_bookmarks_named_destinations_and_toc_links() {
        let export = test_export();
        let root = std::env::temp_dir().join(format!(
            "ostudy-study-guide-navigation-{}.pdf",
            std::process::id()
        ));
        render_study_guide_pdf(&export, &root).unwrap();
        let bytes = fs::read(&root).unwrap();
        let raw = String::from_utf8_lossy(&bytes);
        assert!(raw.contains("/Outlines"));
        assert!(raw.contains("/Dests"));
        assert!(raw.contains("/Subtype /Link"));
        assert!(raw.contains("(section-1)"));
        fs::remove_file(root).unwrap();
    }

    #[test]
    fn unicode_pdf_round_trips_as_selectable_text() {
        let export = test_export();
        let mut document = Document::new();
        document.set_title(&export.document_title);
        let font = install_unicode_font(&mut document).unwrap();
        let mut writer = PdfFlowWriter::new(document, font);
        let root = std::env::temp_dir().join(format!(
            "ostudy-study-guide-export-{}.pdf",
            std::process::id()
        ));
        writer.heading(&export.document_title, 20.0, 26.0).unwrap();
        writer
            .paragraph("β preload – naïve", BODY_SIZE, BODY_LINE_HEIGHT, false)
            .unwrap();
        writer.finish(&root).unwrap();
        let bytes = fs::read(&root).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let reader = PdfReader::new(Cursor::new(bytes)).unwrap();
        let parsed = PdfDocument::new(reader);
        let text = parsed.extract_text_from_page(0).unwrap().text;
        assert!(text.contains("Cardiovascular Physiology"));
        assert!(text.contains("β preload – naïve"));
        fs::remove_file(root).unwrap();
    }
}

#[cfg(test)]
mod export_contract_tests {
    use super::EXPORT_PURPOSE;

    #[test]
    fn document_and_collection_guides_share_the_schema_supported_pdf_purpose() {
        assert_eq!(EXPORT_PURPOSE, "study_guide_pdf");
    }
}

#[cfg(test)]
mod math_tests;
