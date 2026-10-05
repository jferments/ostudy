//! Derived authoring/search identity, never a pre-answer study projection.
//! Rich fields remain canonical documents. These values neither render content
//! nor own another editable copy; persistence rebuilds them from admitted data.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{invalid, QuestionBody, QuestionError, QuestionKind, ValidatedQuestion};

mod knowledge;

const MAX_SEARCH_CHARS: usize = 200_000;

impl QuestionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SingleChoice => "single_choice",
            Self::SelectAll => "select_all",
            Self::TrueFalse => "true_false",
            Self::Matching => "matching",
            Self::Grouping => "grouping",
            Self::Ordering => "ordering",
            Self::ImageOcclusion => "image_occlusion",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SingleChoice => "Single-best-answer",
            Self::SelectAll => "Select all that apply",
            Self::TrueFalse => "True / false",
            Self::Matching => "Matching",
            Self::Grouping => "Grouping",
            Self::Ordering => "Ordering",
            Self::ImageOcclusion => "Image occlusion",
        }
    }
}

pub struct QuestionSearchText {
    pub prompt: String,
    pub answer: String,
}

impl ValidatedQuestion {
    /// Study metadata contains only the stem, not ordered answers, pairs,
    /// rationales or other targets from the author's private search projection.
    pub fn prompt_text(&self, presentation_key: &str) -> Result<String, QuestionError> {
        if !self
            .presentation_keys()
            .iter()
            .any(|key| key == presentation_key)
        {
            return Err(invalid("The question presentation does not exist."));
        }
        let stem = match &self.definition().question {
            QuestionBody::SingleChoice { prompt, .. }
            | QuestionBody::SelectAll { prompt, .. }
            | QuestionBody::Matching { prompt, .. }
            | QuestionBody::Grouping { prompt, .. }
            | QuestionBody::Ordering { prompt, .. } => prompt,
            QuestionBody::TrueFalse { statement, .. } => statement,
            QuestionBody::ImageOcclusion { questions, .. } => {
                &questions
                    .iter()
                    .find(|question| question.id == presentation_key)
                    .ok_or_else(|| invalid("The image-occlusion target does not exist."))?
                    .prompt
            }
        };
        bounded_join(vec![rich_text(stem)])
    }

    /// The answer key on one line for authoring lists, such as a deck's card
    /// list, where it sits beside the prompt as a basic card's answer does.
    /// It reveals the key, so study views never use it.
    pub fn answer_summary(&self, presentation_key: &str) -> Result<String, QuestionError> {
        if !self
            .presentation_keys()
            .iter()
            .any(|key| key == presentation_key)
        {
            return Err(invalid("The question presentation does not exist."));
        }
        let parts: Vec<String> = match &self.definition().question {
            QuestionBody::SingleChoice {
                options,
                correct_option_id,
                ..
            } => options
                .iter()
                .filter(|option| &option.id == correct_option_id)
                .map(|option| rich_text(&option.content))
                .collect(),
            QuestionBody::SelectAll {
                options,
                correct_option_ids,
                ..
            } => options
                .iter()
                .filter(|option| correct_option_ids.contains(&option.id))
                .map(|option| rich_text(&option.content))
                .collect(),
            QuestionBody::TrueFalse {
                correct,
                corrected_statement,
                ..
            } => {
                // A corrected statement repairs a false one; it says nothing
                // about a true statement.
                vec![match (correct, corrected_statement) {
                    (false, Some(statement)) => format!("False: {}", rich_text(statement)),
                    (false, None) => "False".to_string(),
                    (true, _) => "True".to_string(),
                }]
            }
            QuestionBody::Matching { pairs, .. } => pairs
                .iter()
                .map(|pair| {
                    format!(
                        "{} → {}",
                        rich_text(&pair.left.content),
                        rich_text(&pair.right.content)
                    )
                })
                .collect(),
            QuestionBody::Grouping {
                categories, items, ..
            } => categories
                .iter()
                .map(|category| {
                    let members = items
                        .iter()
                        .filter(|item| item.category_id == category.id)
                        .map(|item| rich_text(&item.item.content))
                        .collect::<Vec<_>>();
                    format!("{}: {}", rich_text(&category.content), members.join(", "))
                })
                .collect(),
            QuestionBody::Ordering { items, .. } => vec![items
                .iter()
                .map(|item| rich_text(&item.content))
                .collect::<Vec<_>>()
                .join(" → ")],
            QuestionBody::ImageOcclusion { questions, .. } => questions
                .iter()
                .filter(|question| question.id == presentation_key)
                .map(|question| rich_text(&question.answer))
                .collect(),
        };
        Ok(parts.join("; "))
    }

    /// Includes private fields for authoring search. Never expose this as a hint.
    pub fn search_text(&self) -> Result<QuestionSearchText, QuestionError> {
        let mut prompt = Vec::new();
        let mut answer = Vec::new();
        match &self.definition().question {
            QuestionBody::SingleChoice {
                prompt: stem,
                options,
                explanation,
                ..
            }
            | QuestionBody::SelectAll {
                prompt: stem,
                options,
                explanation,
                ..
            } => {
                prompt.push(rich_text(stem));
                for option in options {
                    prompt.push(rich_text(&option.content));
                    add_optional(&mut answer, &option.rationale);
                }
                add_optional(&mut answer, explanation);
            }
            QuestionBody::TrueFalse {
                statement,
                correct,
                explanation,
                corrected_statement,
            } => {
                prompt.push(rich_text(statement));
                answer.push(if *correct { "True" } else { "False" }.to_string());
                add_optional(&mut answer, corrected_statement);
                add_optional(&mut answer, explanation);
            }
            QuestionBody::Matching {
                prompt: stem,
                pairs,
                explanation,
            } => {
                prompt.push(rich_text(stem));
                for pair in pairs {
                    prompt.push(rich_text(&pair.left.content));
                    prompt.push(rich_text(&pair.right.content));
                    add_optional(&mut answer, &pair.rationale);
                }
                add_optional(&mut answer, explanation);
            }
            QuestionBody::Grouping {
                prompt: stem,
                categories,
                items,
                explanation,
            } => {
                prompt.push(rich_text(stem));
                prompt.extend(categories.iter().map(|item| rich_text(&item.content)));
                for item in items {
                    prompt.push(rich_text(&item.item.content));
                    add_optional(&mut answer, &item.rationale);
                }
                add_optional(&mut answer, explanation);
            }
            QuestionBody::Ordering {
                prompt: stem,
                items,
                explanation,
            } => {
                prompt.push(rich_text(stem));
                prompt.extend(items.iter().map(|item| rich_text(&item.content)));
                add_optional(&mut answer, explanation);
            }
            QuestionBody::ImageOcclusion { questions, .. } => {
                for question in questions {
                    prompt.push(rich_text(&question.prompt));
                    answer.push(rich_text(&question.answer));
                    add_optional(&mut answer, &question.explanation);
                }
            }
        }
        Ok(QuestionSearchText {
            prompt: bounded_join(prompt)?,
            answer: bounded_join(answer)?,
        })
    }

    /// Versioned exact-content identity ignores generated item IDs and display
    /// order where order is not part of the answer. Rich formatting, assets,
    /// explanations and the answer relation remain significant.
    pub fn duplicate_identity(&self) -> Result<String, QuestionError> {
        let value = match &self.definition().question {
            QuestionBody::SingleChoice {
                prompt,
                options,
                correct_option_id,
                explanation,
            } => json!({
                "prompt": prompt,
                "options": sorted(options.iter().map(|option| json!({
                    "content": option.content, "rationale": option.rationale,
                    "correct": option.id == *correct_option_id
                })).collect()),
                "explanation": explanation
            }),
            QuestionBody::SelectAll {
                prompt,
                options,
                correct_option_ids,
                explanation,
            } => json!({
                "prompt": prompt,
                "options": sorted(options.iter().map(|option| json!({
                    "content": option.content, "rationale": option.rationale,
                    "correct": correct_option_ids.contains(&option.id)
                })).collect()),
                "explanation": explanation
            }),
            QuestionBody::TrueFalse {
                statement,
                correct,
                explanation,
                corrected_statement,
            } => json!({
                "statement": statement, "correct": correct,
                "explanation": explanation, "correctedStatement": corrected_statement
            }),
            QuestionBody::Matching {
                prompt,
                pairs,
                explanation,
            } => json!({
                "prompt": prompt,
                "pairs": sorted(pairs.iter().map(|pair| json!({
                    "left": pair.left.content, "right": pair.right.content,
                    "rationale": pair.rationale
                })).collect()),
                "explanation": explanation
            }),
            QuestionBody::Grouping {
                prompt,
                categories,
                items,
                explanation,
            } => {
                let mut memberships = Vec::new();
                for item in items {
                    let category = categories
                        .iter()
                        .find(|category| category.id == item.category_id)
                        .ok_or_else(|| invalid("A question category is unavailable."))?;
                    memberships.push(json!({
                        "content": item.item.content, "category": category.content,
                        "rationale": item.rationale
                    }));
                }
                json!({
                    "prompt": prompt,
                    "categories": sorted(categories.iter().map(|item| item.content.clone()).collect()),
                    "items": sorted(memberships),
                    "explanation": explanation
                })
            }
            QuestionBody::Ordering {
                prompt,
                items,
                explanation,
            } => json!({
                "prompt": prompt,
                "items": items.iter().map(|item| &item.content).collect::<Vec<_>>(),
                "explanation": explanation
            }),
            QuestionBody::ImageOcclusion {
                image_asset_id,
                masks,
                questions,
            } => {
                let mut targets = Vec::new();
                for question in questions {
                    let mut rectangles = Vec::new();
                    for id in &question.mask_ids {
                        let mask = masks
                            .iter()
                            .find(|mask| &mask.id == id)
                            .ok_or_else(|| invalid("A question mask is unavailable."))?;
                        rectangles.push(json!(mask.rect));
                    }
                    targets.push(json!({
                        "prompt": question.prompt, "answer": question.answer,
                        "explanation": question.explanation, "mode": question.mode,
                        "rectangles": sorted(rectangles)
                    }));
                }
                json!({
                    "imageAssetId": image_asset_id,
                    "masks": sorted(masks.iter().map(|mask| json!(mask.rect)).collect()),
                    "questions": sorted(targets)
                })
            }
        };
        serde_json::to_string(&json!({
            "identityVersion": 1, "kind": self.kind(), "content": value
        }))
        .map_err(|_| invalid("The question identity could not be encoded."))
    }

    pub fn duplicate_hash(&self) -> Result<String, QuestionError> {
        Ok(format!(
            "{:x}",
            Sha256::digest(self.duplicate_identity()?.as_bytes())
        ))
    }
}

fn sorted(mut values: Vec<Value>) -> Vec<Value> {
    values.sort_by_cached_key(Value::to_string);
    values
}

fn add_optional(output: &mut Vec<String>, value: &Option<Value>) {
    if let Some(document) = value {
        output.push(rich_text(document));
    }
}

fn bounded_join(parts: Vec<String>) -> Result<String, QuestionError> {
    let value = parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if value.chars().count() > MAX_SEARCH_CHARS {
        return Err(invalid("The question's searchable text is too large."));
    }
    Ok(value)
}

/// All callers have admitted the rich fields, including depth and node limits.
/// Only content and supported atoms contribute; arbitrary attributes never do.
fn rich_text(document: &Value) -> String {
    fn collect(node: &Value, output: &mut String) {
        let kind = node.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "text" => output.push_str(node.get("text").and_then(Value::as_str).unwrap_or("")),
            "inlineMath" | "blockMath" => {
                output.push(' ');
                output.push_str(
                    node.get("attrs")
                        .and_then(|attrs| attrs.get("latex"))
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                );
                output.push(' ');
            }
            "image" => output.push_str(" [Image] "),
            "audio" => output.push_str(" [Audio] "),
            "video" => output.push_str(" [Video] "),
            "hardBreak" | "horizontalRule" => output.push(' '),
            _ => {}
        }
        if let Some(children) = node.get("content").and_then(Value::as_array) {
            for child in children {
                collect(child, output);
            }
        }
        if matches!(
            kind,
            "paragraph"
                | "heading"
                | "listItem"
                | "taskItem"
                | "tableCell"
                | "tableHeader"
                | "codeBlock"
        ) {
            output.push(' ');
        }
    }
    let mut output = String::new();
    collect(document, &mut output);
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}
