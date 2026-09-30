//! Refuse lossy assembly at the durable content boundary. Whitespace
//! normalization is allowed; dropping fields, array elements or text is not.
use serde_json::Value;

use crate::{
    document_study_guides::{validate_study_guide_section_content, StudyGuideSectionContent},
    error::CommandError,
};

pub(super) fn normalize(
    content: StudyGuideSectionContent,
) -> Result<StudyGuideSectionContent, CommandError> {
    let before = serde_json::to_value(&content).map_err(CommandError::serialization)?;
    let normalized = validate_study_guide_section_content(content)?;
    let after = serde_json::to_value(&normalized).map_err(CommandError::serialization)?;
    if !preserves_content(&before, &after) {
        return Err(CommandError::new(
            "document_study_guide_capacity",
            "The assembled topic exceeds the current durable section capacity. No generated text or qualifications were truncated; validated partition checkpoints remain available for recovery.",
        ));
    }
    Ok(normalized)
}

fn preserves_content(before: &Value, after: &Value) -> bool {
    match (before, after) {
        (Value::String(before), Value::String(after)) => {
            before.split_whitespace().eq(after.split_whitespace())
        }
        (Value::Array(before), Value::Array(after)) => {
            before.len() == after.len()
                && before
                    .iter()
                    .zip(after)
                    .all(|(before, after)| preserves_content(before, after))
        }
        (Value::Object(before), Value::Object(after)) => {
            before.len() == after.len()
                && before.iter().all(|(key, value)| {
                    after
                        .get(key)
                        .is_some_and(|after| preserves_content(value, after))
                })
        }
        _ => before == after,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nested_content_comparison_allows_whitespace_not_qualifier_or_item_loss() {
        assert!(preserves_content(
            &json!({"body": " First\n\ncondition. "}),
            &json!({"body": "First condition."})
        ));
        for changed in [
            json!({"body": "First", "values": ["x ≤ −2 units"]}),
            json!({"body": "First condition.", "values": []}),
            json!({"body": "First condition.", "values": ["x ≤ 2 units"]}),
            json!({"body": "First condition."}),
        ] {
            assert!(!preserves_content(
                &json!({"body": "First condition.", "values": ["x ≤ −2 units"]}),
                &changed
            ));
        }
    }

    #[test]
    fn real_normalizer_cannot_silently_discard_merged_items() {
        let content = StudyGuideSectionContent {
            overview: "A complete supported explanation of a bounded topic.".into(),
            key_points: (0..97)
                .map(|index| format!("Distinct supported statement {index}."))
                .collect(),
            ..Default::default()
        };
        let error = normalize(content).unwrap_err();
        assert_eq!(error.code, "document_study_guide_capacity");
    }

    #[test]
    fn long_prose_survives_normalization_unchanged() {
        let overview = format!(
            "{}Only under the stated condition.",
            "This paragraph explains a supported mechanism. ".repeat(120)
        );
        let normalized = normalize(StudyGuideSectionContent {
            overview: overview.clone(),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(normalized.overview, overview);
    }
}
