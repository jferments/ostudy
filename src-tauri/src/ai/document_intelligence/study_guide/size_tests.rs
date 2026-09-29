use super::*;
use crate::{
    document_study_guides::{
        StudyGuideCallout, StudyGuideCalloutKind, StudyGuideExplanation, StudyGuideVisualExplanation,
    },
    knowledge::authoring::tests::catalog_fixture,
};

fn narrative_at_length(length: usize, unicode: bool) -> String {
    let prefix = "A triangle has three straight sides in the stated geometric setting. ";
    let suffix = " Only under the stated condition; x ≤ −2.050e1 units.";
    let fill = length - prefix.chars().count() - suffix.chars().count();
    format!(
        "{prefix}{}{suffix}",
        if unicode { "λ" } else { "x" }.repeat(fill)
    )
}

#[test]
fn reported_4553_character_overview_survives_admission_normalization_and_checkpoint_roundtrip() {
    let catalog = catalog_fixture();
    for unicode in [false, true] {
        let overview = narrative_at_length(4_553, unicode);
        let value = json!({"content": {"overview": overview}, "knowledgeUnitIds": ["K0"]});
        let section = parse_section_with_images(value, &catalog, false).unwrap();
        assert_eq!(section.content.overview.chars().count(), 4_553);
        assert_eq!(section.content.overview, overview);
        let stored = serde_json::to_string(&section).unwrap();
        let restored: GroundedStudyGuideSection = serde_json::from_str(&stored).unwrap();
        let normalized = validate_study_guide_section_content(restored.content).unwrap();
        assert_eq!(normalized.overview, overview);
        assert_eq!(restored.catalog_fingerprint, catalog.fingerprint().unwrap());
    }
}

#[test]
fn longer_explanations_and_visuals_are_not_misclassified_as_invalid_prose() {
    let body = narrative_at_length(6_000, false);
    let section = StudyGuideSectionContent {
        overview: narrative_at_length(4_553, false),
        explanations: vec![StudyGuideExplanation {
            heading: "Mechanism".into(),
            body: body.clone(),
        }],
        callouts: vec![StudyGuideCallout {
            kind: StudyGuideCalloutKind::Warning,
            title: "Qualification".into(),
            body: body.clone(),
        }],
        visual_explanations: vec![StudyGuideVisualExplanation {
            title: "Attached geometry".into(),
            explanation: body.clone(),
        }],
        ..Default::default()
    };
    let value = json!({"content": section, "knowledgeUnitIds": ["K0"]});
    let parsed = parse_section_with_images(value, &catalog_fixture(), true).unwrap();
    assert_eq!(parsed.content.explanations[0].body, body);
    assert_eq!(parsed.content.callouts[0].body, body);
    assert_eq!(parsed.content.visual_explanations[0].explanation, body);
}

#[test]
fn actual_resource_bounds_remain_hard_and_distinct_from_style() {
    let overview = narrative_at_length(MAX_OVERVIEW_CHARS, true);
    let section = StudyGuideSectionContent {
        overview,
        ..Default::default()
    };
    assert!(validate_generated_safety(&section).is_ok());
    let mut oversized = section.clone();
    oversized.overview.push('λ');
    let error = validate_generated_safety(&oversized).unwrap_err();
    assert_eq!(error.code, "ai_document_output_resource");
    assert!(!error.message.contains("too wordy"));
    assert!(!error.message.contains("outline"));
    assert_eq!(oversized.overview.chars().count(), MAX_OVERVIEW_CHARS + 1);
    let empty = StudyGuideSectionContent::default();
    assert_eq!(
        validate_generated_safety(&empty).unwrap_err().code,
        "ai_document_output_invalid"
    );
}

#[test]
fn many_individually_valid_fields_cannot_exceed_aggregate_capacity() {
    let section = StudyGuideSectionContent {
        overview: narrative_at_length(4_553, false),
        explanations: (0..5)
            .map(|index| StudyGuideExplanation {
                heading: format!("Concept {index}"),
                body: narrative_at_length(MAX_EXPLANATION_BODY_CHARS, false),
            })
            .collect(),
        ..Default::default()
    };
    assert_eq!(
        validate_generated_safety(&section).unwrap_err().code,
        "ai_document_output_resource"
    );
}

#[test]
fn schema_and_admission_use_same_bounds_and_never_trim_table_headers() {
    let schema = study_guide_section_schema();
    assert_eq!(
        schema["properties"]["overview"]["maxLength"],
        MAX_OVERVIEW_CHARS
    );
    assert_eq!(
        schema["properties"]["explanations"]["items"]["properties"]["body"]["maxLength"],
        MAX_EXPLANATION_BODY_CHARS
    );
    assert_eq!(
        schema["properties"]["tables"]["items"]["properties"]["columns"]["items"]["maxLength"],
        MAX_TABLE_CELL_CHARS
    );
    let header = "h".repeat(MAX_TABLE_CELL_CHARS);
    let mut value = json!({"content": {"overview": narrative_at_length(200, false),
        "tables": [{"title": "Test", "columns": [header, "Value"], "rows": [["Condition", "x ≤ −2"]], "caption": ""}]}, "knowledgeUnitIds": ["K0"]});
    let parsed = parse_section_with_images(value.clone(), &catalog_fixture(), false).unwrap();
    assert_eq!(parsed.content.tables[0].columns[0], header);
    value["content"]["tables"][0]["columns"][0] = json!(format!("{header}x"));
    assert_eq!(
        parse_section_with_images(value.clone(), &catalog_fixture(), false)
            .unwrap_err()
            .code,
        "ai_document_output_resource"
    );
    value["content"]["tables"][0]["columns"][0] = json!(header);
    value["content"]["tables"][0]["rows"] = json!([["broken"]]);
    assert_eq!(
        parse_section_with_images(value, &catalog_fixture(), false)
            .unwrap_err()
            .code,
        "ai_document_output_invalid"
    );
}

#[test]
fn unknown_support_and_wrong_types_remain_invalid_even_for_a_long_valid_overview() {
    let mut value = json!({"content": {"overview": narrative_at_length(4_553, true)}, "knowledgeUnitIds": ["K999"]});
    assert!(
        parse_section_with_images_style_advisory(value.clone(), &catalog_fixture(), false).is_err()
    );
    value["knowledgeUnitIds"] = json!(["K0"]);
    value["content"]["explanations"] = json!([{"heading": "Heading", "body": {"private": "text"}}]);
    let error =
        parse_section_with_images_style_advisory(value, &catalog_fixture(), false).unwrap_err();
    assert_eq!(error.code, "ai_document_output_invalid");
    assert!(!error.message.contains("private"));
}
