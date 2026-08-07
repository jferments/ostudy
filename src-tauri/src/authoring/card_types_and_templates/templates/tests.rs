use super::*;

fn test_field() -> CardTemplateFieldDto {
    CardTemplateFieldDto {
        id: "11111111111111111111111111111111".to_string(),
        name: "Concept".to_string(),
        description: String::new(),
        position: 0,
    }
}

fn test_lookup() -> AutomaticTemplateFieldLookup {
    automatic_template_field_lookup_from_dtos(&[test_field()]).expect("field lookup should build")
}

fn test_values(document: Value, asset_ids: Vec<String>) -> HashMap<String, ValidatedTemplateValue> {
    HashMap::from([(
        test_field().id,
        ValidatedTemplateValue {
            doc: document,
            asset_ids,
        },
    )])
}

#[test]
fn resolves_typed_field_names_to_stable_ids() {
    let layout = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{
                "type": "text",
                "text": "Before {{  Concept }} after"
            }]
        }]
    });
    let normalized = normalize_typed_template_layout(&layout, &test_lookup(), "question")
        .expect("typed placeholder should normalize");
    let content = normalized["content"][0]["content"]
        .as_array()
        .expect("paragraph content should exist");
    assert_eq!(content.len(), 3);
    assert_eq!(content[0]["text"], Value::String("Before ".to_string()));
    assert_eq!(
        content[1]["type"],
        Value::String(TEMPLATE_INLINE_NODE.to_string())
    );
    assert_eq!(
        content[1]["attrs"]["fieldId"],
        Value::String(test_field().id)
    );
    assert_eq!(
        content[1]["attrs"]["label"],
        Value::String("Concept".to_string())
    );
    assert_eq!(content[2]["text"], Value::String(" after".to_string()));
}

#[test]
fn keeps_simple_single_paragraph_values_inline() {
    let layout = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{
                "type": "text",
                "text": "Before {{Concept}} after"
            }]
        }]
    });
    let value = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{
                "type": "text",
                "text": "alpha",
                "marks": [{"type": "bold"}]
            }]
        }]
    });
    let rendered = materialize_automatic_template_document(
        &layout,
        &test_lookup(),
        &test_values(value, Vec::new()),
        "question",
    )
    .expect("simple value should render inline");
    let content = rendered["content"][0]["content"]
        .as_array()
        .expect("paragraph content should exist");
    assert_eq!(content.len(), 3);
    assert_eq!(content[0]["text"], Value::String("Before ".to_string()));
    assert_eq!(content[1]["text"], Value::String("alpha".to_string()));
    assert_eq!(
        content[1]["marks"][0]["type"],
        Value::String("bold".to_string())
    );
    assert_eq!(content[2]["text"], Value::String(" after".to_string()));
}

#[test]
fn promotes_lists_to_blocks_and_splits_surrounding_paragraphs() {
    let layout = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{
                "type": "text",
                "text": "Effects: {{Concept}} Continue here."
            }]
        }]
    });
    let value = json!({
        "type": "doc",
        "content": [{
            "type": "orderedList",
            "attrs": {"start": 1},
            "content": [{
                "type": "listItem",
                "content": [{
                    "type": "paragraph",
                    "content": [{"type": "text", "text": "First"}]
                }]
            }]
        }]
    });
    let rendered = materialize_automatic_template_document(
        &layout,
        &test_lookup(),
        &test_values(value, Vec::new()),
        "answer",
    )
    .expect("list should be promoted to a block");
    let blocks = rendered["content"]
        .as_array()
        .expect("document blocks should exist");
    assert_eq!(blocks.len(), 3);
    assert_eq!(blocks[0]["type"], Value::String("paragraph".to_string()));
    assert_eq!(
        blocks[0]["content"][0]["text"],
        Value::String("Effects: ".to_string())
    );
    assert_eq!(blocks[1]["type"], Value::String("orderedList".to_string()));
    assert_eq!(blocks[2]["type"], Value::String("paragraph".to_string()));
    assert_eq!(
        blocks[2]["content"][0]["text"],
        Value::String(" Continue here.".to_string())
    );
}

#[test]
fn promotes_image_only_values_without_empty_wrapper_paragraphs() {
    let asset_id = "22222222222222222222222222222222".to_string();
    let layout = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": "{{Concept}}"}]
        }]
    });
    let value = json!({
        "type": "doc",
        "content": [{
            "type": "image",
            "attrs": {
                "assetId": asset_id,
                "src": "study-asset://22222222222222222222222222222222",
                "alt": "Diagram"
            }
        }]
    });
    let rendered = materialize_automatic_template_document(
        &layout,
        &test_lookup(),
        &test_values(value, vec![asset_id]),
        "answer",
    )
    .expect("image should be promoted to a block");
    let blocks = rendered["content"]
        .as_array()
        .expect("document blocks should exist");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["type"], Value::String("image".to_string()));
}

#[test]
fn normalizes_legacy_block_placeholders_to_neutral_placeholders() {
    let field = test_field();
    let layout = json!({
        "type": "doc",
        "content": [{
            "type": TEMPLATE_BLOCK_NODE,
            "attrs": {"fieldId": field.id, "label": field.name}
        }]
    });
    let normalized = normalize_typed_template_layout(&layout, &test_lookup(), "answer")
        .expect("legacy block should normalize");
    assert_eq!(
        normalized["content"][0]["type"],
        Value::String("paragraph".to_string())
    );
    assert_eq!(
        normalized["content"][0]["content"][0]["type"],
        Value::String(TEMPLATE_INLINE_NODE.to_string())
    );
}

#[test]
fn rejects_unknown_and_incomplete_typed_placeholders() {
    let unknown = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": "{{Missing}}"}]
        }]
    });
    let error = normalize_typed_template_layout(&unknown, &test_lookup(), "question")
        .expect_err("unknown field should fail");
    assert_eq!(error.code, "unknown_template_field");

    let incomplete = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": "{{Concept}"}]
        }]
    });
    let error = normalize_typed_template_layout(&incomplete, &test_lookup(), "question")
        .expect_err("incomplete braces should fail");
    assert_eq!(error.code, "invalid_template_placeholder");
}

#[test]
fn cloze_rotation_uses_one_card_schedule_review_count() {
    let parsed = parse_cloze("{{c1::Alpha}} and {{c2::Beta}}").expect("cloze should parse");
    assert_eq!(
        cloze_target_from_variant(&parsed, "one_at_a_time", None, 0).expect("first target"),
        Some(1)
    );
    assert_eq!(
        cloze_target_from_variant(&parsed, "one_at_a_time", None, 1).expect("second target"),
        Some(2)
    );
    assert_eq!(
        cloze_target_from_variant(&parsed, "one_at_a_time", None, 2).expect("wrapped target"),
        Some(1)
    );
}

#[test]
fn inline_placeholders_accept_optional_display_labels() {
    let field = test_field();
    let fields = vec![field.clone()];
    let layout = json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{
                "type": TEMPLATE_INLINE_NODE,
                "attrs": {"fieldId": field.id, "label": field.name}
            }]
        }]
    });
    assert_eq!(
        validate_template_layout(&layout, &fields, "question")
            .expect("labeled placeholder should validate"),
        1
    );
}
