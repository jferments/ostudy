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

use oxidize_pdf::{Document, Font, Page};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    document_domain::{SourceLocator, SourceUnitKind},
    document_study_guides::{
        load_active_document_study_guide, DocumentStudyGuide, DocumentStudyGuideSection,
        StudyGuideOverlay, StudyGuideOverlayKind, StudyGuideOverlayState,
    },
    error::CommandError,
    external_files::{PreparedExportDto, RegisterPreparedExport},
    paths::AppPaths,
    util::validate_entity_id,
};

const EXPORT_PURPOSE: &str = "study_guide_pdf";
const PAGE_WIDTH: f64 = 595.0;
const PAGE_HEIGHT: f64 = 842.0;
const MARGIN_X: f64 = 48.0;
const MARGIN_TOP: f64 = 50.0;
const MARGIN_BOTTOM: f64 = 46.0;
const BODY_SIZE: f64 = 10.5;
const BODY_LINE_HEIGHT: f64 = 14.0;
const MAX_EXPORT_SECTIONS: usize = 512;
const MAX_EXPORT_TEXT_CHARS: usize = 5_000_000;
const EMBEDDED_FONT_NAME: &str = "OStudyGuideSans";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PrepareStudyGuidePdfInput {
    pub document_id: String,
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
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedStudyGuidePdfDto {
    pub prepared_export: PreparedExportDto,
    pub guide_revision: i64,
    pub section_count: i64,
    pub annotation_count: i64,
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
    sections: Vec<EffectiveExportSection>,
    annotation_count: usize,
}

#[derive(Clone, Debug)]
struct EffectiveExportSection {
    title: String,
    source_locator: SourceLocator,
    body_lines: Vec<ExportBlock>,
    notes: Vec<String>,
}

#[derive(Clone, Debug)]
enum ExportBlock {
    Heading(String),
    Paragraph(String),
    Bullets(Vec<String>),
    Numbered(Vec<String>),
    Terms(Vec<(String, String)>),
    Spacer,
}

#[derive(Clone, Debug)]
enum PdfFontChoice {
    Custom(String),
    Builtin,
}

impl PdfFontChoice {
    fn font(&self, bold: bool) -> Font {
        match self {
            Self::Custom(name) => Font::Custom(name.clone()),
            Self::Builtin if bold => Font::HelveticaBold,
            Self::Builtin => Font::Helvetica,
        }
    }
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
        });
    }

    let effective = load_effective_export(
        database.inner(),
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
    })
}

fn load_effective_export(
    database: &Database,
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
    compose_effective_export(document_title, source_filename, guide, include_annotations)
}

fn compose_effective_export(
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
        } else if include_annotations && overlay.overlay_kind == StudyGuideOverlayKind::GuideNote {
            if !overlay.plain_text.trim().is_empty() {
                guide_notes.push(overlay.plain_text.trim().to_string());
            }
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
        sections.push(EffectiveExportSection {
            title,
            source_locator: section.source_locator.clone(),
            body_lines: body_override.map_or_else(
                || generated_section_blocks(section),
                |replacement| vec![ExportBlock::Paragraph(replacement)],
            ),
            notes,
        });
    }

    let export = EffectiveGuideExport {
        document_title,
        source_filename,
        guide_revision: guide.revision,
        overview: guide.overview.summary,
        learning_objectives: guide.overview.learning_objectives,
        key_concepts: guide.overview.key_concepts,
        guide_notes,
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

fn generated_section_blocks(section: &DocumentStudyGuideSection) -> Vec<ExportBlock> {
    let content = &section.content;
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
    if !content.visual_explanations.is_empty() {
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

    writer.heading(&export.document_title, 22.0, 28.0)?;
    writer.paragraph("Study guide", 12.0, 17.0, true)?;
    writer.paragraph(
        &format!("Source: {}", export.source_filename),
        BODY_SIZE,
        BODY_LINE_HEIGHT,
        false,
    )?;
    writer.paragraph(
        &format!("Guide revision: {}", export.guide_revision),
        BODY_SIZE,
        BODY_LINE_HEIGHT,
        false,
    )?;
    writer.spacer(10.0)?;

    writer.heading("Overview", 16.0, 21.0)?;
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

    writer.heading("Contents", 16.0, 21.0)?;
    for (index, section) in export.sections.iter().enumerate() {
        writer.paragraph(
            &format!(
                "{}. {} ({})",
                index + 1,
                section.title,
                source_label(&section.source_locator)
            ),
            BODY_SIZE,
            BODY_LINE_HEIGHT,
            false,
        )?;
    }
    writer.page_break()?;

    for (index, section) in export.sections.iter().enumerate() {
        writer.heading(&format!("{}. {}", index + 1, section.title), 17.0, 23.0)?;
        writer.paragraph(&source_label(&section.source_locator), 9.5, 13.0, false)?;
        writer.spacer(5.0)?;
        for block in &section.body_lines {
            writer.block(block)?;
        }
        if !section.notes.is_empty() {
            writer.heading("My notes", 13.0, 18.0)?;
            writer.bullets(&section.notes, false)?;
        }
        writer.spacer(12.0)?;
    }

    writer.finish(destination)
}

struct PdfFlowWriter {
    document: Document,
    font: PdfFontChoice,
    page: Page,
    page_number: usize,
    y: f64,
    page_has_content: bool,
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
        }
    }

    fn heading(&mut self, text: &str, size: f64, line_height: f64) -> Result<(), CommandError> {
        self.text(text, size, line_height, true, 0.0)
    }

    fn paragraph(
        &mut self,
        text: &str,
        size: f64,
        line_height: f64,
        bold: bool,
    ) -> Result<(), CommandError> {
        self.text(text, size, line_height, bold, 0.0)
    }

    fn bullets(&mut self, items: &[String], numbered: bool) -> Result<(), CommandError> {
        for (index, item) in items.iter().enumerate() {
            let prefix = if numbered {
                format!("{}. ", index + 1)
            } else {
                "• ".to_string()
            };
            self.text(
                &format!("{prefix}{item}"),
                BODY_SIZE,
                BODY_LINE_HEIGHT,
                false,
                12.0,
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
                    self.text(
                        &format!("{term}: {explanation}"),
                        BODY_SIZE,
                        BODY_LINE_HEIGHT,
                        false,
                        12.0,
                    )?;
                }
                Ok(())
            }
            ExportBlock::Spacer => self.spacer(5.0),
        }
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

    fn text(
        &mut self,
        text: &str,
        size: f64,
        line_height: f64,
        bold: bool,
        indent: f64,
    ) -> Result<(), CommandError> {
        let available_width = PAGE_WIDTH - (2.0 * MARGIN_X) - indent;
        let lines = wrap_text(text, available_width, size);
        for line in lines {
            self.ensure_space(line_height)?;
            self.page
                .text()
                .set_font(self.font.font(bold), size)
                .at(MARGIN_X + indent, self.y)
                .write(&line)
                .map_err(pdf_error)?;
            self.page_has_content = true;
            self.y -= line_height;
        }
        self.spacer((line_height * 0.35).max(2.0))
    }

    fn ensure_space(&mut self, needed: f64) -> Result<(), CommandError> {
        if self.y - needed < MARGIN_BOTTOM {
            self.commit_page()?;
        }
        Ok(())
    }

    fn commit_page(&mut self) -> Result<(), CommandError> {
        if self.page_has_content {
            let footer = format!("oStudy • Study guide • Page {}", self.page_number);
            self.page
                .text()
                .set_font(self.font.font(false), 8.0)
                .at(MARGIN_X, 24.0)
                .write(&footer)
                .map_err(pdf_error)?;
        }
        let next = Page::a4();
        let finished = std::mem::replace(&mut self.page, next);
        self.document.add_page(finished);
        self.page_number = self.page_number.saturating_add(1);
        self.y = PAGE_HEIGHT - MARGIN_TOP;
        self.page_has_content = false;
        Ok(())
    }

    fn finish(mut self, destination: &Path) -> Result<(), CommandError> {
        if self.page_has_content {
            self.commit_page()?;
        }
        self.document.save(destination).map_err(pdf_error)
    }
}

fn install_unicode_font(document: &mut Document) -> Result<PdfFontChoice, CommandError> {
    let Some(path) = find_unicode_font() else {
        return Err(CommandError::new(
            "study_guide_pdf_font_missing",
            "oStudy could not find a local Unicode TrueType/OpenType font for study-guide PDF export.",
        ));
    };
    let bytes = fs::read(&path).map_err(CommandError::io)?;
    document
        .add_font_from_bytes(EMBEDDED_FONT_NAME, bytes)
        .map_err(pdf_error)?;
    Ok(PdfFontChoice::Custom(EMBEDDED_FONT_NAME.to_string()))
}

fn find_unicode_font() -> Option<PathBuf> {
    [
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/opentype/noto/NotoSans-Regular.otf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/system/fonts/NotoSans-Regular.ttf",
        "/system/fonts/Roboto-Regular.ttf",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "C:\\Windows\\Fonts\\arial.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
}

fn wrap_text(text: &str, available_width: f64, font_size: f64) -> Vec<String> {
    let approximate_character_width = (font_size * 0.53).max(1.0);
    let maximum = (available_width / approximate_character_width)
        .floor()
        .max(8.0) as usize;
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
            sections: vec![EffectiveExportSection {
                title: "Preload".to_string(),
                source_locator: SourceLocator::new(SourceUnitKind::Page, vec![2, 3], None).unwrap(),
                body_lines: vec![ExportBlock::Paragraph(
                    "Preload reflects ventricular filling before systole.".to_string(),
                )],
                notes: vec!["Compare with afterload.".to_string()],
            }],
            annotation_count: 2,
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
    fn ascii_pdf_round_trips_as_selectable_text() {
        let export = test_export();
        let mut document = Document::new();
        document.set_title(&export.document_title);
        let writer = PdfFlowWriter::new(document, PdfFontChoice::Builtin);
        let root = std::env::temp_dir().join(format!(
            "ostudy-study-guide-export-{}.pdf",
            std::process::id()
        ));
        let mut writer = writer;
        writer.heading(&export.document_title, 20.0, 26.0).unwrap();
        writer
            .paragraph(&export.overview, BODY_SIZE, BODY_LINE_HEIGHT, false)
            .unwrap();
        writer.finish(&root).unwrap();
        let bytes = fs::read(&root).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let reader = PdfReader::new(Cursor::new(bytes)).unwrap();
        let parsed = PdfDocument::new(reader);
        let text = parsed.extract_text_from_page(0).unwrap().text;
        assert!(text.contains("Cardiovascular Physiology"));
        assert!(text.contains("Understand preload"));
        fs::remove_file(root).unwrap();
    }
}
