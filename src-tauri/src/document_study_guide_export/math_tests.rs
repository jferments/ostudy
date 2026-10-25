use super::*;

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

fn extracted_text(export: &EffectiveGuideExport) -> String {
    super::tests::rendered_pages(export, StudyGuidePdfTheme::Navy)
        .1
        .join("\n")
}

#[test]
fn supported_document_pdf_math_is_typeset_without_source_fallback_notice() {
    let export = fixture();
    assert!(!math_source::requires_source_fallback(&export));
    let text = extracted_text(&export);
    assert!(!text.contains("Math notation:"));
    assert!(!text.contains(r"\frac{a}{b}"));
    // Typst writes math letters as Unicode mathematical italic.
    assert!(text.contains('𝑎'));
    assert!(text.contains('𝑏'));
    assert_eq!(export.overview, r"Solve \(\frac{a}{b}\).");
}

#[test]
fn unsupported_document_pdf_math_preserves_source_and_discloses_fallback() {
    let mut export = fixture();
    export.overview = r"Review \(\overset{?}{x}\).".into();
    assert!(math_source::requires_source_fallback(&export));
    let text = extracted_text(&export);
    assert!(text.contains("Math notation:"));
    assert!(text.contains("LaTeX source"));
    assert!(text.contains(r"\overset{?}{x}"));
}

#[test]
fn wide_formula_is_kept_as_wrapped_source() {
    let source = format!("\\(\\frac{{{}}}{{b}}\\)", "a".repeat(120));
    let mut export = fixture();
    export.overview = source;
    assert!(math_source::requires_source_fallback(&export));
    let text = extracted_text(&export);
    assert!(text.contains("Math notation:"));
    assert!(text.contains(r"\frac{"));
}

#[test]
fn unicode_prose_and_native_math_remain_selectable() {
    let mut export = fixture();
    export.overview = r"Пример: \(\alpha \le \beta\) и ΔP.".into();
    assert!(!math_source::requires_source_fallback(&export));
    let text = extracted_text(&export);
    assert!(text.contains("Пример"));
    assert!(text.contains('𝛼'));
    assert!(text.contains('𝛽'));
    assert!(text.contains('≤'));
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
        key: "topic-math".into(),
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
        citations: None,
        terms: vec![],
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
