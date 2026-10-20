//! Local PDF export for the effective document study-guide view.
//!
//! Export is read-only with respect to learner data. The renderer composes one
//! frozen snapshot from the immutable generated guide plus compatible active
//! learner overlays, writes selectable text through the already-pinned pure-Rust
//! `oxidize-pdf` backend, and registers the result with the existing prepared-
//! export boundary. It never calls an AI provider or scrapes the React DOM.

use std::{collections::HashMap, fs};

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    document_domain::SourceLocator,
    document_store,
    document_study_guides::{
        editorial::StudyGuideEditorial, load_active_document_study_guide, DocumentStudyGuide,
        DocumentStudyGuideSection, StudyGuideOverlay, StudyGuideOverlayKind,
        StudyGuideOverlayState, StudyGuideSectionContent,
    },
    document_visual_evidence,
    error::CommandError,
    external_files::{PreparedExportDto, RegisterPreparedExport},
    paths::AppPaths,
    study_materials::{load_collection_study_guide, CollectionStudyGuide},
    util::validate_entity_id,
};

mod guide_typst;
mod math_layout;
mod math_source;

pub(crate) use guide_typst::StudyGuidePdfTheme;

const EXPORT_PURPOSE: &str = "study_guide_pdf";
const MAX_EXPORT_SECTIONS: usize = 512;
const MAX_EXPORT_TEXT_CHARS: usize = 5_000_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PrepareStudyGuidePdfInput {
    pub document_id: String,
    pub operation_id: String,
    #[serde(default)]
    pub include_annotations: bool,
    #[serde(default)]
    pub theme: StudyGuidePdfTheme,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PrepareCollectionStudyGuidePdfInput {
    pub collection_id: String,
    pub operation_id: String,
    #[serde(default)]
    pub include_annotations: bool,
    #[serde(default)]
    pub theme: StudyGuidePdfTheme,
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
struct ExportFigure {
    image: guide_typst::FigureImage,
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
    guide_typst::render(&effective, input.theme, &destination)?;

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
    guide_typst::render(&effective, input.theme, &destination)?;

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

    let editorial_titles = editorial_topic_titles(guide.editorial.as_ref());
    let mut sections = Vec::with_capacity(guide.sections.len());
    let mut annotation_count = guide_notes.len();
    for section in &guide.sections {
        let mut title = editorial_titles
            .get(section.topic_id.as_str())
            .copied()
            .unwrap_or(section.title.as_str())
            .to_string();
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

fn editorial_topic_titles(editorial: Option<&StudyGuideEditorial>) -> HashMap<&str, &str> {
    editorial
        .into_iter()
        .flat_map(|editorial| editorial.chapters.iter())
        .flat_map(|chapter| chapter.topics.iter())
        .map(|topic| (topic.topic_id.as_str(), topic.title.as_str()))
        .collect()
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
            let image = guide_typst::FigureImage::from_rgba(decoded)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    use crate::document_domain::SourceUnitKind;
    use oxidize_pdf::parser::{PdfDocument, PdfReader};

    pub(super) fn test_export() -> EffectiveGuideExport {
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

    pub(super) fn rendered_pages(
        export: &EffectiveGuideExport,
        theme: StudyGuidePdfTheme,
    ) -> (Vec<u8>, Vec<String>) {
        let bytes = guide_typst::render_bytes(export, theme).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let parsed = PdfDocument::new(PdfReader::new(Cursor::new(bytes.clone())).unwrap());
        let pages = (0..parsed.page_count().unwrap())
            .map(|page| parsed.extract_text_from_page(page).unwrap().text)
            .collect();
        (bytes, pages)
    }

    fn rich_section(title: &str) -> EffectiveExportSection {
        EffectiveExportSection {
            title: title.to_string(),
            source_locator: Some(SourceLocator::new(SourceUnitKind::Page, vec![4], None).unwrap()),
            body_lines: vec![
                ExportBlock::Paragraph("Afterload is the load the ventricle ejects against.".into()),
                ExportBlock::Heading("Key points".into()),
                ExportBlock::Bullets(vec!["Raised by hypertension.".into()]),
                ExportBlock::Numbered(vec![
                    "Pressure rises.".into(),
                    "Valve opens.".into(),
                    "Blood ejects.".into(),
                ]),
                ExportBlock::Terms(vec![("Afterload".into(), "Ejection load.".into())]),
                ExportBlock::Callout {
                    label: "Key idea".into(),
                    title: "Load opposes ejection".into(),
                    body: "Higher afterload lowers stroke volume.".into(),
                },
                ExportBlock::Callout {
                    label: "Watch out".into(),
                    title: "Not preload".into(),
                    body: "Afterload is not filling.".into(),
                },
                ExportBlock::Table {
                    title: "Compare".into(),
                    columns: vec!["Concept".into(), "Meaning".into()],
                    rows: vec![vec!["Afterload".into(), "Ejection load".into()], vec!["Short".into()]],
                    caption: "High-yield comparison".into(),
                },
                ExportBlock::WorkedExample {
                    title: "Trace a change".into(),
                    prompt: "Aortic stenosis develops.".into(),
                    steps: vec!["Outflow narrows.".into()],
                    answer: "Afterload rises.".into(),
                    takeaway: "Follow cause before effect.".into(),
                },
                ExportBlock::RetrievalCheck {
                    prompt: "What raises afterload?".into(),
                    answer: "Hypertension and stenosis.".into(),
                },
                ExportBlock::Spacer,
            ],
            notes: Vec::new(),
            figures: Vec::new(),
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
    fn editorial_topic_titles_replace_outline_labels_before_pdf_overrides() {
        use crate::document_study_guides::editorial::{
            StudyGuideChapter, StudyGuideEditorial, StudyGuideTopicHeading,
        };
        let topic_id = "a".repeat(32);
        let editorial = StudyGuideEditorial {
            title: "Cardiovascular drugs".to_string(),
            chapters: vec![StudyGuideChapter {
                id: "b".repeat(32),
                title: "Hemodynamic foundations".to_string(),
                introduction: "A readable chapter introduction.".to_string(),
                topics: vec![StudyGuideTopicHeading {
                    topic_id: topic_id.clone(),
                    title: "Preload and cardiac output".to_string(),
                }],
            }],
        };
        let titles = editorial_topic_titles(Some(&editorial));
        assert_eq!(
            titles.get(topic_id.as_str()).copied(),
            Some("Preload and cardiac output")
        );
    }

    #[test]
    fn source_labels_use_natural_unit_names() {
        let page = SourceLocator::new(SourceUnitKind::Page, vec![3], None).unwrap();
        let slides = SourceLocator::new(SourceUnitKind::Slide, vec![4, 5], None).unwrap();
        assert_eq!(source_label(&page), "Source page 3");
        assert_eq!(source_label(&slides), "Source slides 4–5");
    }

    #[test]
    fn rendered_pdf_opens_on_a_cover_and_omits_internal_guide_revision() {
        let (_, pages) = rendered_pages(&test_export(), StudyGuidePdfTheme::Navy);
        let all = pages.join("\n");
        assert!(pages[0].contains("Physiology"));
        assert!(pages[0].contains("1 section ·"));
        assert!(all.contains("Preload reflects ventricular filling before systole."));
        assert!(all.contains("Compare with afterload."));
        assert!(all.contains("Review this before lab."));
        assert!(!all.contains("Guide revision"));
        assert!(!all.contains("Revision 2"));
    }

    #[test]
    fn every_theme_renders_every_block_kind_and_the_review_pages() {
        let mut export = test_export();
        export.sections.push(rich_section("Afterload"));
        for theme in [
            StudyGuidePdfTheme::Navy,
            StudyGuidePdfTheme::Sage,
            StudyGuidePdfTheme::Slate,
            StudyGuidePdfTheme::Plum,
        ] {
            let (_, pages) = rendered_pages(&export, theme);
            let all = pages.join("\n");
            for expected in [
                "Raised by hypertension.",
                "Valve opens.",
                "Ejection load.",
                "Load opposes ejection",
                "High-yield comparison",
                "Follow cause before effect.",
                "Cram sheet",
                "Self-test",
                "What raises afterload?",
                "Hypertension and stenosis.",
            ] {
                assert!(all.contains(expected), "{theme:?} lost {expected:?}");
            }
        }
    }

    #[test]
    fn cram_sheet_and_self_test_come_from_the_guide_content() {
        let mut export = test_export();
        export.sections.push(rich_section("Afterload"));
        let (_, pages) = rendered_pages(&export, StudyGuidePdfTheme::Navy);
        let cram = pages
            .iter()
            .find(|page| page.contains("The whole guide on one page."))
            .expect("cram sheet page");
        assert!(cram.contains("Frank-Starling mechanism"));
        assert!(cram.contains("Higher afterload lowers stroke volume."));
        assert!(cram.contains("Afterload is not filling."));
        let test_page = pages
            .iter()
            .find(|page| page.contains("answers follow the questions"))
            .expect("self-test page");
        assert!(test_page.contains("What raises afterload?"));
        assert!(test_page.contains("Hypertension and stenosis."));

        let plain = test_export();
        let (_, pages) = rendered_pages(
            &EffectiveGuideExport {
                key_concepts: Vec::new(),
                ..plain
            },
            StudyGuidePdfTheme::Navy,
        );
        assert!(!pages.join("\n").contains("Cram sheet"));
        assert!(!pages.join("\n").contains("Self-test"));
    }

    #[test]
    fn editorial_pdf_uses_major_chapters_for_contents_and_nested_section_headings() {
        let mut export = test_export();
        export.document_title = "Cardiovascular drugs".to_string();
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
        let (_, pages) = rendered_pages(&export, StudyGuidePdfTheme::Navy);
        let cover = &pages[0];
        let all = pages.join("\n");
        assert!(cover.contains("Cardiovascular drugs"));
        assert!(cover.contains("Hemodynamic foundations"));
        assert!(!cover.contains("Renin"));
        assert!(all.contains("These sections establish the physiologic relationships"));
        assert!(all.contains("1.1"));
        assert!(all.contains("1.2"));
        assert!(all.contains("Renin–angiotensin system"));
    }

    #[test]
    fn long_guides_move_contents_to_their_own_page() {
        let mut export = test_export();
        for index in 0..14 {
            export.sections.push(EffectiveExportSection {
                title: format!("Topic number {index}"),
                ..export.sections[0].clone()
            });
        }
        let (_, pages) = rendered_pages(&export, StudyGuidePdfTheme::Navy);
        assert!(pages[0].contains("more on the next page"));
        assert!(pages[1].contains("Topic number 13"));
    }

    #[test]
    fn long_tables_break_across_pages_and_repeat_headers() {
        let mut export = test_export();
        export.sections[0].body_lines.push(ExportBlock::Table {
            title: "Reference table".into(),
            columns: vec!["Concept".into(), "Meaning".into()],
            rows: (0..90)
                .map(|index| {
                    vec![
                        format!("Concept {index}"),
                        "A compact explanation that remains selectable.".into(),
                    ]
                })
                .collect(),
            caption: String::new(),
        });
        let pages = guide_typst::page_texts(&export, StudyGuidePdfTheme::Navy);
        let table_pages = pages
            .iter()
            .filter(|page| page.contains("A compact explanation"))
            .collect::<Vec<_>>();
        assert!(table_pages.len() > 1);
        for page in table_pages {
            assert!(page.contains("Concept Meaning"));
        }
    }

    #[test]
    fn rendered_pdf_embeds_figure_image_and_selectable_caption() {
        let mut export = test_export();
        let mut pixels = image::RgbaImage::new(4, 3);
        pixels.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        export.sections[0].figures.push(ExportFigure {
            image: guide_typst::FigureImage::from_rgba(pixels).unwrap(),
            title: "Pressure-volume loop".to_string(),
            caption: "Trace filling before ejection.".to_string(),
        });
        let (bytes, pages) = rendered_pages(&export, StudyGuidePdfTheme::Navy);
        assert!(String::from_utf8_lossy(&bytes).contains("/Subtype/Image"));
        let all = pages.join("\n");
        assert!(all.contains("Pressure-volume loop"));
        assert!(all.contains("Trace filling before ejection."));
    }

    #[test]
    fn rendered_pdf_contains_bookmarks_and_contents_links() {
        let (bytes, _) = rendered_pages(&test_export(), StudyGuidePdfTheme::Navy);
        let raw = String::from_utf8_lossy(&bytes);
        assert!(raw.contains("/Type/Outlines"));
        assert!(raw.contains("/Title(1 Preload)"));
        assert!(raw.contains("/Subtype/Link"));
    }

    #[test]
    fn learner_text_is_placed_as_text_not_evaluated_as_markup() {
        let mut export = test_export();
        export.document_title = "#panic() *b* $x$".to_string();
        export.overview = "#read(\"/data.json\") ]) #{panic()} <label> @ref".to_string();
        export.sections[0].title = "#set page(width: 1pt)".to_string();
        let (_, pages) = rendered_pages(&export, StudyGuidePdfTheme::Navy);
        let all = pages.join("\n");
        assert!(all.contains("#panic() *b* $x$"));
        assert!(all.contains("#read(\"/data.json\")"));
        assert!(all.contains("#set page(width: 1pt)"));
    }

    #[test]
    fn unicode_text_round_trips_as_selectable_text() {
        let mut export = test_export();
        export.overview = "β preload – naïve".to_string();
        let (_, pages) = rendered_pages(&export, StudyGuidePdfTheme::Sage);
        assert!(pages.join("\n").contains("β preload – naïve"));
    }

    #[test]
    fn themes_parse_from_ipc_and_default_to_navy() {
        let input: PrepareStudyGuidePdfInput = serde_json::from_value(serde_json::json!({
            "documentId": "a".repeat(32),
            "operationId": "b".repeat(32),
        }))
        .unwrap();
        assert_eq!(input.theme, StudyGuidePdfTheme::Navy);
        let input: PrepareCollectionStudyGuidePdfInput = serde_json::from_value(serde_json::json!({
            "collectionId": "a".repeat(32),
            "operationId": "b".repeat(32),
            "theme": "plum",
        }))
        .unwrap();
        assert_eq!(input.theme, StudyGuidePdfTheme::Plum);
        assert!(serde_json::from_value::<PrepareStudyGuidePdfInput>(serde_json::json!({
            "documentId": "a".repeat(32),
            "operationId": "b".repeat(32),
            "theme": "neon",
        }))
        .is_err());
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

