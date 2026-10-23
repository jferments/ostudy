//! Typst rendering for study-guide PDF export.
//!
//! The effective guide becomes JSON data that a fixed, compiled-in template
//! lays out. Learner- and model-authored strings stay data: the template
//! places them as text. Only math the exporter translated from its bounded
//! LaTeX subset is evaluated. The compiler sees an in-memory world holding the
//! template, the data, figure images, and fonts compiled into the binary plus
//! a few system fallbacks; it cannot read other files or reach the network.

use std::{collections::HashMap, io::Cursor, path::Path, sync::LazyLock};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use typst::{
    diag::{FileError, FileResult, SourceDiagnostic},
    foundations::{Bytes, Datetime, Duration},
    syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot},
    text::{Font, FontBook},
    utils::LazyHash,
    Library, LibraryExt, World,
};
use typst_layout::PagedDocument;

use super::{
    math_layout, math_source, source_label, EffectiveExportSection, EffectiveGuideExport,
    ExportBlock,
};
use crate::error::CommandError;

const TEMPLATE: &str = include_str!("typst/guide.typ");
const THEMES: &str = include_str!("typst/themes.typ");
const BLOCKS: &str = include_str!("typst/blocks.typ");
const BUNDLED_FONTS: &[&[u8]] = include!(concat!(env!("OUT_DIR"), "/pdf_fonts.rs"));

/// IBM Plex Sans Regular, the first face `build.rs` bundles: a Unicode sans
/// that annotated PDFs use for typed notes.
pub(crate) fn plain_sans_font() -> &'static [u8] {
    BUNDLED_FONTS[0]
}
const MAX_CRAM_TERMS: usize = 40;
const MAX_CRAM_WARNINGS: usize = 8;
// Kept LaTeX has no natural break points, so long source gets zero-width break
// opportunities at this interval.
const SOURCE_BREAK_CHARS: usize = 48;

/// Colour and type theme for an exported guide. Each has the same layout.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum StudyGuidePdfTheme {
    #[default]
    Navy,
    Sage,
    Slate,
    Plum,
}

impl StudyGuidePdfTheme {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Navy => "navy",
            Self::Sage => "sage",
            Self::Slate => "slate",
            Self::Plum => "plum",
        }
    }
}

/// An image placed in the guide, already encoded as PNG.
#[derive(Clone, Debug)]
pub(super) struct FigureImage(pub(super) Vec<u8>);

impl FigureImage {
    pub(super) fn from_rgba(rgba: image::RgbaImage) -> Option<Self> {
        let mut bytes = Vec::new();
        rgba.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .ok()?;
        Some(Self(bytes))
    }
}

pub(super) fn render(
    export: &EffectiveGuideExport,
    theme: StudyGuidePdfTheme,
    destination: &Path,
) -> Result<(), CommandError> {
    let bytes = render_bytes(export, theme)?;
    std::fs::write(destination, bytes).map_err(CommandError::io)
}

pub(super) fn render_bytes(
    export: &EffectiveGuideExport,
    theme: StudyGuidePdfTheme,
) -> Result<Vec<u8>, CommandError> {
    let pdf = compile(export, theme).and_then(|document| {
        typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
            .map_err(|diagnostics| export_error(describe(&diagnostics)))
    });
    // Compilation results are memoized process-wide; nothing is reused between
    // exports, so release them rather than holding them for the session.
    typst::comemo::evict(0);
    pdf
}

/// One laid-out page of the guide for the reader's canvas: its size in points
/// and the page as a standalone SVG.
#[derive(Clone, Debug)]
pub(super) struct CanvasPage {
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) svg: String,
}

/// Where a chapter, section, Practice button, cram sheet or self-test landed:
/// its 1-based page and its top-left corner in points from the page's.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(super) struct CanvasAnchor {
    pub(super) kind: String,
    pub(super) key: String,
    pub(super) page: usize,
    pub(super) x: f64,
    pub(super) y: f64,
}

/// The whole guide laid out exactly as the PDF export lays it out, as one SVG
/// per page, with where each chapter, section and Practice button landed.
pub(super) fn canvas(
    export: &EffectiveGuideExport,
    theme: StudyGuidePdfTheme,
) -> Result<(Vec<CanvasPage>, Vec<CanvasAnchor>), CommandError> {
    use typst::introspection::{Introspector, MetadataElem};
    let laid_out = compile(export, theme).map(|document| {
        let options = typst_svg::SvgOptions::default();
        let pages = document
            .pages()
            .iter()
            .map(|page| CanvasPage {
                width: page.frame.width().to_pt(),
                height: page.frame.height().to_pt(),
                svg: typst_svg::svg(page, &options),
            })
            .collect::<Vec<_>>();
        let label = typst::foundations::Label::new(typst::utils::PicoStr::intern("canvas-anchor"))
            .expect("static label");
        let anchors = document
            .introspector()
            .query(&typst::foundations::Selector::Label(label))
            .iter()
            .filter_map(|content| {
                let value = &content.to_packed::<MetadataElem>()?.value;
                serde_json::to_value(value)
                    .ok()
                    .and_then(|value| serde_json::from_value::<CanvasAnchor>(value).ok())
            })
            .collect::<Vec<_>>();
        (pages, anchors)
    });
    typst::comemo::evict(0);
    laid_out
}

fn compile(
    export: &EffectiveGuideExport,
    theme: StudyGuidePdfTheme,
) -> Result<PagedDocument, CommandError> {
    let mut files = HashMap::new();
    let data = guide_data(export, theme, &mut files);
    files.insert(
        file_id("/data.json"),
        Bytes::new(serde_json::to_vec(&data).map_err(|error| export_error(error.to_string()))?),
    );
    compile_world(TEMPLATE, files).map_err(export_error)
}

fn compile_world(template: &str, files: HashMap<FileId, Bytes>) -> Result<PagedDocument, String> {
    let world = GuideWorld::new(template, files);
    typst::compile::<PagedDocument>(&world)
        .output
        .map_err(|diagnostics| describe(&diagnostics))
}

/// Lays out another compiled-in template that imports the guide themes, with
/// `data` as its /data.json, and returns each page as a standalone SVG. The
/// world is the guide's: bundled fonts and nothing from disk or the network.
pub(crate) fn compile_template_svgs(template: &str, data: &Value) -> Result<Vec<String>, String> {
    let mut files = HashMap::new();
    files.insert(
        file_id("/data.json"),
        Bytes::new(serde_json::to_vec(data).map_err(|error| error.to_string())?),
    );
    let pages = compile_world(template, files).map(|document| {
        let options = typst_svg::SvgOptions::default();
        document
            .pages()
            .iter()
            .map(|page| typst_svg::svg(page, &options))
            .collect()
    });
    typst::comemo::evict(0);
    pages
}

/// The laid-out text of each page, including running headers and repeated
/// table headers that PDF text extraction skips as artifacts.
#[cfg(test)]
pub(super) fn page_texts(export: &EffectiveGuideExport, theme: StudyGuidePdfTheme) -> Vec<String> {
    fn collect(frame: &typst::layout::Frame, output: &mut String) {
        for (_, item) in frame.items() {
            match item {
                typst::layout::FrameItem::Group(group) => collect(&group.frame, output),
                typst::layout::FrameItem::Text(text) => {
                    output.push_str(&text.text);
                    output.push(' ');
                }
                _ => {}
            }
        }
    }
    compile(export, theme)
        .unwrap()
        .pages()
        .iter()
        .map(|page| {
            let mut text = String::new();
            collect(&page.frame, &mut text);
            text
        })
        .collect()
}

fn describe(diagnostics: &[SourceDiagnostic]) -> String {
    diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.to_string())
        .unwrap_or_else(|| "the layout engine reported an unknown error".to_string())
}

fn export_error(detail: String) -> CommandError {
    CommandError::new(
        "study_guide_pdf_export_failed",
        format!("Could not generate the study-guide PDF: {detail}"),
    )
}

fn file_id(path: &str) -> FileId {
    RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(path).expect("static virtual path"),
    )
    .intern()
}

struct GuideWorld {
    main: Source,
    /// The modules templates may import: the themes and the shared blocks.
    modules: [Source; 2],
    files: HashMap<FileId, Bytes>,
}

impl GuideWorld {
    fn new(template: &str, files: HashMap<FileId, Bytes>) -> Self {
        Self {
            main: Source::new(file_id("/main.typ"), template.to_string()),
            modules: [
                Source::new(file_id("/themes.typ"), THEMES.to_string()),
                Source::new(file_id("/blocks.typ"), BLOCKS.to_string()),
            ],
            files,
        }
    }
}

impl World for GuideWorld {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &FONTS.0
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        std::iter::once(&self.main)
            .chain(&self.modules)
            .find(|source| source.id() == id)
            .cloned()
            .ok_or(FileError::AccessDenied)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.get(&id).cloned().ok_or(FileError::AccessDenied)
    }

    fn font(&self, index: usize) -> Option<Font> {
        FONTS.1.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

static LIBRARY: LazyLock<LazyHash<Library>> = LazyLock::new(|| LazyHash::new(Library::default()));

static FONTS: LazyLock<(LazyHash<FontBook>, Vec<Font>)> = LazyLock::new(|| {
    let mut fonts = BUNDLED_FONTS
        .iter()
        .flat_map(|bytes| Font::iter(Bytes::new(*bytes)))
        .collect::<Vec<_>>();
    // Libertinus Serif is the serif body face and New Computer Modern Math the
    // math face. The rest of typst-assets is not needed.
    fonts.extend(
        typst_assets::fonts()
            .flat_map(|bytes| Font::iter(Bytes::new(bytes)))
            .filter(|font| {
                let family = font.info().family.as_str();
                family.starts_with("Libertinus Serif")
                    || family.starts_with("New Computer Modern Math")
                    || family == "DejaVu Sans Mono"
            }),
    );
    // System Unicode faces broaden script coverage when the device has them.
    for path in SYSTEM_FALLBACK_FONTS {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.extend(Font::iter(Bytes::new(bytes)));
        }
    }
    (LazyHash::new(FontBook::from_fonts(&fonts)), fonts)
});

const SYSTEM_FALLBACK_FONTS: &[&str] = &[
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
    "/usr/share/fonts/opentype/noto/NotoSans-Regular.otf",
    "/usr/share/fonts/opentype/noto/NotoSans-Bold.otf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/system/fonts/NotoSans-Regular.ttf",
    "/system/fonts/NotoSans-Bold.ttf",
    "/system/fonts/NotoSansCJK-Regular.ttc",
    "C:\\Windows\\Fonts\\arial.ttf",
    "C:\\Windows\\Fonts\\arialbd.ttf",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
];

/// Splits text into template runs: plain text, translated math, or LaTeX kept
/// as visible source when it is outside the bounded subset.
pub(crate) fn runs(text: &str) -> Value {
    let mut result = Vec::new();
    for segment in math_source::segments(text) {
        match segment {
            math_source::MathSegment::Text(value) if !value.is_empty() => {
                result.push(json!({ "t": value }));
            }
            math_source::MathSegment::Text(_) => {}
            math_source::MathSegment::Formula {
                latex,
                source,
                block,
            } => match math_layout::typst_markup(latex) {
                Ok(markup) if math_source::native_layout(latex, 10.0).is_some() => {
                    result.push(json!({ "m": markup, "block": block, "src": source }));
                }
                _ => result.push(json!({
                    "src": math_source::wrap_source(source, SOURCE_BREAK_CHARS).join("\u{200b}"),
                })),
            },
        }
    }
    Value::Array(result)
}

fn run_list(items: &[String]) -> Value {
    Value::Array(items.iter().map(|item| runs(item)).collect())
}

fn guide_data(
    export: &EffectiveGuideExport,
    theme: StudyGuidePdfTheme,
    files: &mut HashMap<FileId, Bytes>,
) -> Value {
    let mut figure_count = 0_usize;
    let sections = export
        .sections
        .iter()
        .map(|section| section_data(section, files, &mut figure_count))
        .collect::<Vec<_>>();
    let chapters = export
        .chapters
        .iter()
        .map(|chapter| {
            json!({
                "key": chapter.key,
                "title": runs(&chapter.title),
                "intro": runs(&chapter.introduction),
                "sections": chapter.section_indices,
            })
        })
        .collect::<Vec<_>>();
    let cram = CramSheet::collect(export);
    json!({
        "title": export.document_title,
        "source": export.source_filename,
        "sectionCount": export.sections.len(),
        "theme": theme.key(),
        "overview": runs(&export.overview),
        "objectives": run_list(&export.learning_objectives),
        "keyConcepts": run_list(&export.key_concepts),
        "guideNotes": run_list(&export.guide_notes),
        "mathNotice": math_source::requires_source_fallback(export),
        "mathNoticeText": math_source::NOTICE,
        "chapters": chapters,
        "sections": sections,
        "keyIdeas": cram.key_ideas,
        "terms": cram.terms,
        "warnings": cram.warnings,
        "checks": cram.checks,
    })
}

fn section_data(
    section: &EffectiveExportSection,
    files: &mut HashMap<FileId, Bytes>,
    figure_count: &mut usize,
) -> Value {
    let mut lead_used = false;
    let blocks = section
        .body_lines
        .iter()
        .map(|block| {
            let lead = !lead_used && matches!(block, ExportBlock::Paragraph(_));
            lead_used = true;
            block_data(block, lead)
        })
        .collect::<Vec<_>>();
    let figures = section
        .figures
        .iter()
        .map(|figure| {
            // Paths repeat across exports so the compiler's interned file ids
            // stay bounded.
            let path = format!("/figures/{figure_count}.png");
            *figure_count += 1;
            files.insert(file_id(&path), Bytes::new(figure.image.0.clone()));
            json!({
                "path": path,
                "title": runs(&figure.title),
                "caption": runs(&figure.caption),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "key": section.key,
        "title": runs(&section.title),
        "source": section.source_locator.as_ref().map(source_label),
        "blocks": blocks,
        "figures": figures,
        "notes": run_list(&section.notes),
    })
}

fn block_data(block: &ExportBlock, lead: bool) -> Value {
    match block {
        ExportBlock::Heading(text) => json!({ "kind": "heading", "text": runs(text) }),
        ExportBlock::Paragraph(text) => json!({
            "kind": if lead { "lead" } else { "paragraph" },
            "text": runs(text),
        }),
        ExportBlock::Bullets(items) => json!({ "kind": "bullets", "items": run_list(items) }),
        ExportBlock::Numbered(items) => json!({ "kind": "numbered", "items": run_list(items) }),
        ExportBlock::Terms(terms) => json!({ "kind": "terms", "terms": term_list(terms) }),
        ExportBlock::Callout { label, title, body } => json!({
            "kind": "callout",
            "label": label,
            "title": runs(title),
            "body": runs(body),
        }),
        ExportBlock::Table {
            title,
            columns,
            rows,
            caption,
        } => {
            // Rows are padded or trimmed to the header so the grid stays whole.
            let width = columns.len().max(1);
            let rows = rows
                .iter()
                .map(|row| {
                    Value::Array(
                        (0..width)
                            .map(|index| runs(row.get(index).map_or("", String::as_str)))
                            .collect(),
                    )
                })
                .collect::<Vec<_>>();
            let columns = if columns.is_empty() {
                vec![runs("")]
            } else {
                columns.iter().map(|column| runs(column)).collect()
            };
            json!({
                "kind": "table",
                "title": runs(title),
                "columns": columns,
                "rows": rows,
                "caption": runs(caption),
            })
        }
        ExportBlock::WorkedExample {
            title,
            prompt,
            steps,
            answer,
            takeaway,
        } => json!({
            "kind": "example",
            "title": runs(title),
            "prompt": runs(prompt),
            "steps": run_list(steps),
            "answer": runs(answer),
            "takeaway": runs(takeaway),
        }),
        ExportBlock::RetrievalCheck { prompt, answer } => json!({
            "kind": "check",
            "prompt": runs(prompt),
            "answer": runs(answer),
        }),
        ExportBlock::Spacer => json!({ "kind": "spacer" }),
    }
}

fn term_list(terms: &[(String, String)]) -> Value {
    Value::Array(
        terms
            .iter()
            .map(|(term, meaning)| json!([runs(term), runs(meaning)]))
            .collect(),
    )
}

/// End-of-guide review pages, gathered from the guide's own content: key-idea
/// callouts, terms, warnings and misconceptions, and every retrieval check.
#[derive(Default)]
struct CramSheet {
    key_ideas: Vec<Value>,
    terms: Vec<Value>,
    warnings: Vec<Value>,
    checks: Vec<Value>,
}

impl CramSheet {
    fn collect(export: &EffectiveGuideExport) -> Self {
        let mut sheet = Self::default();
        let mut seen_terms = std::collections::HashSet::new();
        for section in &export.sections {
            let mut previous_heading = None::<&str>;
            for block in &section.body_lines {
                match block {
                    ExportBlock::Callout { label, title, body } if label == "Key idea" => {
                        sheet
                            .key_ideas
                            .push(json!({ "title": runs(title), "body": runs(body) }));
                    }
                    ExportBlock::Callout { label, body, .. } if label == "Watch out" => {
                        sheet.warnings.push(runs(body));
                    }
                    ExportBlock::Bullets(items)
                        if previous_heading == Some("Common misconceptions") =>
                    {
                        sheet.warnings.extend(items.iter().map(|item| runs(item)));
                    }
                    ExportBlock::Terms(terms) => {
                        for (term, meaning) in terms {
                            if seen_terms.insert(term.trim().to_lowercase()) {
                                sheet.terms.push(json!([runs(term), runs(meaning)]));
                            }
                        }
                    }
                    ExportBlock::RetrievalCheck { prompt, answer } => {
                        sheet.checks.push(json!({
                            "prompt": runs(prompt),
                            "answer": runs(answer),
                            "section": section.title,
                        }));
                    }
                    _ => {}
                }
                previous_heading = match block {
                    ExportBlock::Heading(text) => Some(text.as_str()),
                    _ => None,
                };
            }
        }
        sheet.terms.truncate(MAX_CRAM_TERMS);
        sheet.warnings.truncate(MAX_CRAM_WARNINGS);
        sheet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_font_is_compiled_in() {
        for family in [
            "IBM Plex Sans",
            "Lato",
            "Sora",
            "Roboto Slab",
            "Caladea",
            "Libertinus Serif",
            "New Computer Modern Math",
            "DejaVu Sans Mono",
        ] {
            assert!(
                FONTS.0.contains_family(&family.to_lowercase()),
                "{family} is missing from the PDF font book"
            );
        }
        for theme in ["navy", "sage", "slate", "plum"] {
            assert!(THEMES.contains(&format!("  {theme}: (")), "{theme}");
        }
    }

    #[test]
    fn template_compiles_without_warnings() {
        let mut export = crate::document_study_guide_export::tests::test_export();
        export.overview = r"Rate \(\frac{\Delta V}{\Delta t}\) and $$x^2$$.".into();
        let mut files = HashMap::new();
        let data = guide_data(&export, StudyGuidePdfTheme::Sage, &mut files);
        files.insert(
            file_id("/data.json"),
            Bytes::new(serde_json::to_vec(&data).unwrap()),
        );
        let world = GuideWorld::new(TEMPLATE, files);
        let compiled = typst::compile::<PagedDocument>(&world);
        let messages = compiled
            .warnings
            .iter()
            .map(|warning| warning.message.to_string())
            .collect::<Vec<_>>();
        assert!(compiled.output.is_ok());
        assert!(messages.is_empty(), "{messages:?}");
    }
}
