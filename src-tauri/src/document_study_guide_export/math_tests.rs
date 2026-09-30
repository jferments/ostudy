use super::*;
use oxidize_pdf::parser::{PdfDocument, PdfReader};
use std::io::Cursor;

fn fixture() -> EffectiveGuideExport {
    EffectiveGuideExport {
        document_title: "Math guide".into(),
        source_filename: "source.pdf".into(),
        guide_revision: 1,
        overview: r"Solve \(\frac{a}{b}\).".into(),
        learning_objectives: vec![],
        key_concepts: vec![],
        guide_notes: vec![],
        chapters: vec![],
        sections: vec![],
        annotation_count: 0,
    }
}

fn extracted_text(path: &Path) -> String {
    let parsed = PdfDocument::new(PdfReader::new(Cursor::new(fs::read(path).unwrap())).unwrap());
    (0..parsed.page_count().unwrap())
        .map(|page| parsed.extract_text_from_page(page).unwrap().text)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn supported_document_pdf_math_is_typeset_without_source_fallback_notice() {
    let export = fixture();
    assert!(!math_source::requires_source_fallback(&export));
    let path = std::env::temp_dir().join(format!("ostudy-guide-math-{}.pdf", std::process::id()));
    render_study_guide_pdf(&export, &path).unwrap();
    let text = extracted_text(&path);
    assert!(!text.contains("Math notation:"));
    assert!(!text.contains(r"\frac{a}{b}"));
    assert!(text.contains('a'));
    assert!(text.contains('b'));
    assert_eq!(export.overview, r"Solve \(\frac{a}{b}\).");
    fs::remove_file(path).unwrap();
}

#[test]
fn unsupported_document_pdf_math_preserves_source_and_discloses_fallback() {
    let mut export = fixture();
    export.overview = r"Review \(\overset{?}{x}\).".into();
    assert!(math_source::requires_source_fallback(&export));
    let path = std::env::temp_dir().join(format!(
        "ostudy-guide-math-fallback-{}.pdf",
        std::process::id()
    ));
    render_study_guide_pdf(&export, &path).unwrap();
    let text = extracted_text(&path);
    assert!(text.contains("Math notation:"));
    assert!(text.contains("LaTeX source"));
    assert!(text.contains(r"\overset{?}{x}"));
    fs::remove_file(path).unwrap();
}

#[test]
fn wide_formula_uses_bounded_source_fallback_lines() {
    let source = format!("\\(\\frac{{{}}}{{b}}\\)", "a".repeat(120));
    let mut export = fixture();
    export.overview = source.clone();
    assert!(math_source::requires_source_fallback(&export));
    let maximum_width = PAGE_WIDTH - (2.0 * MARGIN_X);
    let lines = source_fallback_lines(&source, BODY_SIZE, maximum_width);
    assert!(lines.len() > 1);
    for line in lines {
        let FlowLine::Inline(line) = line else {
            panic!("source fallback must remain text");
        };
        assert!(line.width <= maximum_width);
    }
}

#[test]
fn unicode_prose_and_native_math_remain_selectable() {
    let mut export = fixture();
    export.overview = r"Пример: \(\alpha \le \beta\) и ΔP.".into();
    assert!(!math_source::requires_source_fallback(&export));
    let path = std::env::temp_dir().join(format!(
        "ostudy-guide-unicode-math-{}.pdf",
        std::process::id()
    ));
    render_study_guide_pdf(&export, &path).unwrap();
    let text = extracted_text(&path);
    assert!(text.contains("Пример"));
    assert!(text.contains('α'));
    assert!(text.contains('β'));
    assert!(text.contains('≤'));
    fs::remove_file(path).unwrap();
}

#[test]
fn document_pdf_math_notice_scans_effective_content_not_debug_or_source_filenames() {
    let mut export = fixture();
    export.overview = "Plain prose".into();
    export.source_filename = r"\(filename\).pdf".into();
    assert!(!math_source::has_math(&export));
    assert!(!math_source::requires_source_fallback(&export));
    export.guide_notes.push(r"\[x_i\]".into());
    assert!(math_source::has_math(&export));
    assert!(!math_source::requires_source_fallback(&export));
    export.guide_notes.clear();
    export.sections.push(EffectiveExportSection {
        title: "Topic".into(),
        source_locator: Some(
            SourceLocator::new(
                crate::document_domain::SourceUnitKind::Section,
                vec![1],
                None,
            )
            .unwrap(),
        ),
        body_lines: vec![ExportBlock::Terms(vec![("Term".into(), "$$x$$".into())])],
        notes: vec![],
        figures: vec![],
    });
    assert!(math_source::has_math(&export));
    assert!(!math_source::requires_source_fallback(&export));
    export.sections[0].body_lines = vec![ExportBlock::Paragraph("````\n$$code$$\n````".into())];
    assert!(!math_source::has_math(&export));
    assert!(!math_source::requires_source_fallback(&export));
}

#[test]
fn document_pdf_math_summary_replay_defaults_conservatively_without_a_migration() {
    let old: StudyGuidePdfSummary = serde_json::from_value(
        serde_json::json!({"guideRevision":1,"sectionCount":1,"annotationCount":0}),
    )
    .unwrap();
    assert!(old.math_source_fallback);
    let current = StudyGuidePdfSummary {
        guide_revision: 1,
        section_count: 1,
        annotation_count: 0,
        math_source_fallback: false,
    };
    let saved = serde_json::to_value(&current).unwrap();
    assert_eq!(saved["mathSourceFallback"], false);
    assert!(
        !serde_json::from_value::<StudyGuidePdfSummary>(saved)
            .unwrap()
            .math_source_fallback
    );
}
