#[cfg(test)]
mod stage3_v2_tests {
    use super::*;

    #[test]
    fn inserts_block_values_without_flattening_lists() {
        let field_id = "11111111111111111111111111111111".to_string();
        let layout = json!({
            "type": "doc",
            "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": "Effects:"}]},
                {"type": TEMPLATE_BLOCK_NODE, "attrs": {"fieldId": field_id}}
            ]
        });
        let value_doc = json!({
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
        let values = HashMap::from([(
            field_id,
            ValidatedTemplateValue {
                doc: value_doc,
                asset_ids: Vec::new(),
            },
        )]);
        let rendered = render_template_document(&layout, &values).expect("render should work");
        assert_eq!(
            rendered["content"][1]["type"],
            Value::String("orderedList".to_string())
        );
    }

    #[test]
    fn rejects_block_values_inside_sentences() {
        let field_id = "11111111111111111111111111111111".to_string();
        let layout = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": TEMPLATE_INLINE_NODE, "attrs": {"fieldId": field_id}}]
            }]
        });
        let value_doc = json!({
            "type": "doc",
            "content": [{"type": "image", "attrs": {
                "assetId": "22222222222222222222222222222222",
                "src": "study-asset://22222222222222222222222222222222"
            }}]
        });
        let values = HashMap::from([(
            field_id,
            ValidatedTemplateValue {
                doc: value_doc,
                asset_ids: vec!["22222222222222222222222222222222".to_string()],
            },
        )]);
        let error = render_template_document(&layout, &values)
            .expect_err("block content should fail inline insertion");
        assert_eq!(error.code, "template_block_content_inline");
    }

    #[test]
    fn cloze_rotation_uses_one_card_schedule_review_count() {
        let parsed = parse_cloze("{{c1::Alpha}} and {{c2::Beta}}")
            .expect("cloze should parse");
        assert_eq!(
            cloze_target_from_variant(&parsed, "one_at_a_time", None, 0)
                .expect("first target"),
            Some(1)
        );
        assert_eq!(
            cloze_target_from_variant(&parsed, "one_at_a_time", None, 1)
                .expect("second target"),
            Some(2)
        );
        assert_eq!(
            cloze_target_from_variant(&parsed, "one_at_a_time", None, 2)
                .expect("wrapped target"),
            Some(1)
        );
    }

    #[test]
    fn inline_placeholders_accept_optional_display_labels() {
        let field_id = "11111111111111111111111111111111".to_string();
        let fields = vec![CardTemplateFieldDto {
            id: field_id.clone(),
            name: "Concept".to_string(),
            description: String::new(),
            position: 0,
        }];
        let layout = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": TEMPLATE_INLINE_NODE,
                    "attrs": {"fieldId": field_id, "label": "Concept"}
                }]
            }]
        });
        assert_eq!(
            validate_template_layout(&layout, &fields, "question")
                .expect("labeled placeholder should validate"),
            1
        );
    }
}
