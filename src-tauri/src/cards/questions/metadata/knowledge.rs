//! Read-only semantic projections. Authored answer relations are not facts
//! certified by oStudy. Never use the private projection as pre-answer context.

use super::{add_optional, bounded_join, invalid, rich_text, QuestionSearchText};
use crate::cards::questions::{PublicQuestion, QuestionBody, QuestionError, ValidatedQuestion};

impl ValidatedQuestion {
    /// A complete, labeled answer relation for one scheduled presentation.
    /// Occlusion excludes the original image, mask coordinates and sibling keys.
    pub fn knowledge_text(&self, key: &str) -> Result<QuestionSearchText, QuestionError> {
        let prompt = self.prompt_text(key)?;
        let mut answer = Vec::new();
        match &self.definition().question {
            QuestionBody::SingleChoice {
                options,
                correct_option_id,
                explanation,
                ..
            } => {
                for option in options {
                    answer.push(format!(
                        "{} option: {}",
                        if option.id == *correct_option_id {
                            "Correct"
                        } else {
                            "Incorrect"
                        },
                        rich_text(&option.content),
                    ));
                    add_optional(&mut answer, &option.rationale);
                }
                add_optional(&mut answer, explanation);
            }
            QuestionBody::SelectAll {
                options,
                correct_option_ids,
                explanation,
                ..
            } => {
                if correct_option_ids.is_empty() {
                    answer.push("Correct selection: none of the options.".to_string());
                }
                for option in options {
                    answer.push(format!(
                        "{} option: {}",
                        if correct_option_ids.contains(&option.id) {
                            "Correct"
                        } else {
                            "Incorrect"
                        },
                        rich_text(&option.content),
                    ));
                    add_optional(&mut answer, &option.rationale);
                }
                add_optional(&mut answer, explanation);
            }
            QuestionBody::TrueFalse {
                correct,
                explanation,
                corrected_statement,
                ..
            } => {
                answer.push(format!(
                    "Authored verdict: {}",
                    if *correct { "True" } else { "False" }
                ));
                if let Some(correction) = corrected_statement {
                    answer.push(format!("Corrected statement: {}", rich_text(correction)));
                }
                add_optional(&mut answer, explanation);
            }
            QuestionBody::Matching {
                pairs, explanation, ..
            } => {
                for pair in pairs {
                    answer.push(format!(
                        "Match: {} => {}",
                        rich_text(&pair.left.content),
                        rich_text(&pair.right.content),
                    ));
                    add_optional(&mut answer, &pair.rationale);
                }
                add_optional(&mut answer, explanation);
            }
            QuestionBody::Grouping {
                categories,
                items,
                explanation,
                ..
            } => {
                for item in items {
                    let category = categories
                        .iter()
                        .find(|category| category.id == item.category_id)
                        .ok_or_else(|| invalid("A question category is unavailable."))?;
                    answer.push(format!(
                        "Group: {} => {}",
                        rich_text(&item.item.content),
                        rich_text(&category.content),
                    ));
                    add_optional(&mut answer, &item.rationale);
                }
                add_optional(&mut answer, explanation);
            }
            QuestionBody::Ordering {
                items, explanation, ..
            } => {
                answer.push("Authored order under the question's criterion:".to_string());
                for (index, item) in items.iter().enumerate() {
                    answer.push(format!("{}. {}", index + 1, rich_text(&item.content)));
                }
                add_optional(&mut answer, explanation);
            }
            QuestionBody::ImageOcclusion { questions, .. } => {
                let question = questions
                    .iter()
                    .find(|question| question.id == key)
                    .ok_or_else(|| invalid("The image-occlusion target does not exist."))?;
                answer.push(rich_text(&question.answer));
                add_optional(&mut answer, &question.explanation);
            }
        }
        Ok(QuestionSearchText {
            prompt,
            answer: bounded_join(answer)?,
        })
    }
}

impl PublicQuestion {
    /// Consume the already-redacted, instance-ordered public projection rather
    /// than independently shuffling or reading the author's private definition.
    pub fn knowledge_prompt_text(&self) -> Result<String, QuestionError> {
        let mut parts = Vec::new();
        match self {
            Self::SingleChoice { prompt, options } | Self::SelectAll { prompt, options } => {
                parts.push(rich_text(prompt));
                for (index, option) in options.iter().enumerate() {
                    parts.push(format!(
                        "Option {}: {}",
                        index + 1,
                        rich_text(&option.content)
                    ));
                }
            }
            Self::TrueFalse { statement } => parts.push(rich_text(statement)),
            Self::Matching {
                prompt,
                items,
                targets,
            } => {
                parts.push(rich_text(prompt));
                for (index, item) in items.iter().enumerate() {
                    parts.push(format!("Item {}: {}", index + 1, rich_text(&item.content)));
                }
                for (index, item) in targets.iter().enumerate() {
                    parts.push(format!(
                        "Target {}: {}",
                        index + 1,
                        rich_text(&item.content)
                    ));
                }
            }
            Self::Grouping {
                prompt,
                items,
                categories,
            } => {
                parts.push(rich_text(prompt));
                for (index, item) in items.iter().enumerate() {
                    parts.push(format!("Item {}: {}", index + 1, rich_text(&item.content)));
                }
                for category in categories {
                    parts.push(format!("Category: {}", rich_text(&category.content)));
                }
            }
            Self::Ordering { prompt, items } => {
                parts.push(rich_text(prompt));
                for (index, item) in items.iter().enumerate() {
                    parts.push(format!(
                        "Displayed item {}: {}",
                        index + 1,
                        rich_text(&item.content)
                    ));
                }
            }
            Self::ImageOcclusion { prompt, .. } => {
                parts.push(rich_text(prompt));
                parts.push("The masked image is not available to this text context.".to_string());
            }
        }
        bounded_join(parts)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    fn rich(text: &str) -> Value {
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
    }

    fn admit(body: Value) -> ValidatedQuestion {
        ValidatedQuestion::parse(&json!({"schemaVersion":1,"question":body}).to_string()).unwrap()
    }

    #[test]
    fn semantic_relations_preserve_keys_instead_of_flattening_them_into_unlabeled_text() {
        let a = "a".repeat(32);
        let b = "b".repeat(32);
        let c = "c".repeat(32);
        let d = "d".repeat(32);
        let options = json!([{"id":a,"content":rich("two")},
            {"id":b,"content":rich("three"),"rationale":rich("Not even.")}]);
        let fixtures = vec![
            (
                json!({"kind":"single_choice","prompt":rich("Which is even?"),
                "options":options,"correctOptionId":a}),
                "Correct option: two",
            ),
            (
                json!({"kind":"select_all","prompt":rich("Which are negative?"),
                "options":options,"correctOptionIds":[]}),
                "Correct selection: none",
            ),
            (
                json!({"kind":"true_false","statement":rich("Two is odd."),"correct":false,
                "correctedStatement":rich("Two is even.")}),
                "Corrected statement: Two is even.",
            ),
            (
                json!({"kind":"matching","prompt":rich("Match terms."),"pairs":[
                {"left":{"id":a,"content":rich("cat")},"right":{"id":c,"content":rich("mammal")}},
                {"left":{"id":b,"content":rich("robin")},"right":{"id":d,"content":rich("bird")}}]}),
                "Match: cat => mammal",
            ),
            (
                json!({"kind":"grouping","prompt":rich("Group numbers."),
                "categories":[{"id":a,"content":rich("even")},{"id":b,"content":rich("odd")}],
                "items":[{"item":{"id":c,"content":rich("two")},"categoryId":a},
                    {"item":{"id":d,"content":rich("three")},"categoryId":b}]}),
                "Group: two => even",
            ),
            (
                json!({"kind":"ordering","prompt":rich("Increasing order."),
                "items":[{"id":a,"content":rich("two")},{"id":b,"content":rich("three")}]}),
                "1. two\n2. three",
            ),
        ];
        for (body, expected) in fixtures {
            let question = admit(body);
            let projection = question.knowledge_text("main").unwrap();
            assert!(
                projection.answer.contains(expected),
                "{}",
                projection.answer
            );
            assert!(!projection.answer.contains(&a));
            assert!(question.knowledge_text("missing").is_err());
        }
    }

    #[test]
    fn occlusion_semantic_projection_excludes_original_image_geometry_and_sibling_answers() {
        let image = "a".repeat(32);
        let mask = "b".repeat(32);
        let first = "c".repeat(32);
        let second = "d".repeat(32);
        let question = admit(json!({"kind":"image_occlusion","imageAssetId":image,
            "masks":[{"id":mask,"rect":{"x":0.1,"y":0.2,"width":0.3,"height":0.4}}],
            "questions":[
                {"id":first,"prompt":rich("Name target."),"answer":rich("First answer"),
                    "maskIds":[mask],"mode":"hide_target"},
                {"id":second,"prompt":rich("Describe target."),"answer":rich("Sibling secret"),
                    "maskIds":[mask],"mode":"hide_all"}]}));
        let projection = question.knowledge_text(&first).unwrap();
        let serialized = format!("{} {}", projection.prompt, projection.answer);
        assert!(serialized.contains("First answer"));
        for forbidden in [&image, &mask, "Sibling secret", "0.1", "maskIds"] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn public_projection_keeps_display_order_without_option_ids_or_keys() {
        use crate::cards::questions::RichItem;
        let public = PublicQuestion::Ordering {
            prompt: rich("Increasing order."),
            items: vec![
                RichItem {
                    id: "b".repeat(32),
                    content: rich("three"),
                },
                RichItem {
                    id: "a".repeat(32),
                    content: rich("two"),
                },
            ],
        };
        let text = public.knowledge_prompt_text().unwrap();
        assert!(text.contains("Displayed item 1: three\nDisplayed item 2: two"));
        assert!(!text.contains(&"a".repeat(32)));
        assert!(!text.contains("Authored order"));
    }
}
