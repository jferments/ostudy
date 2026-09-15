use super::*;
use oxidize_pdf::parser::{PdfDocument, PdfReader};
use std::io::Cursor;

fn fixture() -> EffectiveGuideExport {
    EffectiveGuideExport { document_title: "Math guide".into(), source_filename: "source.pdf".into(),
        guide_revision: 1, overview: r"Solve \(\frac{a}{b}\).".into(), learning_objectives: vec![],
        key_concepts: vec![], guide_notes: vec![], sections: vec![], annotation_count: 0 }
}

#[test]
fn document_pdf_math_export_discloses_source_and_keeps_equations_selectable() {
    let export = fixture();
    let path = std::env::temp_dir().join(format!("ostudy-guide-math-{}.pdf", std::process::id()));
    render_study_guide_pdf(&export, &path).unwrap();
    let parsed = PdfDocument::new(PdfReader::new(Cursor::new(fs::read(&path).unwrap())).unwrap());
    let text = parsed.extract_text_from_page(0).unwrap().text;
    assert!(text.contains("Math notation:"));
    assert!(text.contains("LaTeX source"));
    assert!(text.contains(r"\frac{a}{b}"));
    assert_eq!(export.overview, r"Solve \(\frac{a}{b}\).");
    fs::remove_file(path).unwrap();
}

#[test]
fn document_pdf_math_notice_scans_effective_content_not_debug_or_source_filenames() {
    let mut export = fixture();
    export.overview = "Plain prose".into();
    export.source_filename = r"\(filename\).pdf".into();
    assert!(!math_source::has_math(&export));
    export.guide_notes.push(r"\[x_i\]".into());
    assert!(math_source::has_math(&export));
    export.guide_notes.clear();
    export.sections.push(EffectiveExportSection {
        title: "Topic".into(), source_locator: SourceLocator::new(crate::document_domain::SourceUnitKind::Section, vec![1], None).unwrap(),
        body_lines: vec![ExportBlock::Terms(vec![("Term".into(), "$$x$$".into())])], notes: vec![],
    });
    assert!(math_source::has_math(&export));
    export.sections[0].body_lines = vec![ExportBlock::Paragraph("````\n$$code$$\n````".into())];
    assert!(!math_source::has_math(&export));
}

#[test]
fn document_pdf_math_summary_replay_defaults_conservatively_without_a_migration() {
    let old: StudyGuidePdfSummary = serde_json::from_value(serde_json::json!({"guideRevision":1,"sectionCount":1,"annotationCount":0})).unwrap();
    assert!(old.math_source_fallback);
    let current = StudyGuidePdfSummary { guide_revision: 1, section_count: 1, annotation_count: 0, math_source_fallback: false };
    let saved = serde_json::to_value(&current).unwrap();
    assert_eq!(saved["mathSourceFallback"], false);
    assert!(!serde_json::from_value::<StudyGuidePdfSummary>(saved).unwrap().math_source_fallback);
}
