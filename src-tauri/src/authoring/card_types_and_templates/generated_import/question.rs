//! Version-2 generated text questions. Transport-local keys are not database
//! identities. All rich fields use the same Markdown admission as Basic cards;
//! all answer relations are then checked by the canonical question domain.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    cards::questions::{
        ChoiceOption, GroupedItem, MatchingPair, QuestionBody, QuestionDefinition, RichItem,
        ValidatedQuestion, MAX_ITEMS, QUESTION_SCHEMA_VERSION,
    },
    error::CommandError,
};

use super::markdown::markdown_document;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum GeneratedQuestion {
    SingleChoice {
        prompt: String,
        options: Vec<GeneratedOption>,
        correct_option_id: String,
        explanation: Option<String>,
    },
    SelectAll {
        prompt: String,
        options: Vec<GeneratedOption>,
        correct_option_ids: Vec<String>,
        explanation: Option<String>,
    },
    TrueFalse {
        statement: String,
        correct: bool,
        explanation: Option<String>,
        corrected_statement: Option<String>,
    },
    Matching {
        prompt: String,
        pairs: Vec<GeneratedPair>,
        explanation: Option<String>,
    },
    Grouping {
        prompt: String,
        categories: Vec<GeneratedItem>,
        items: Vec<GeneratedGroupedItem>,
        explanation: Option<String>,
    },
    Ordering {
        prompt: String,
        items: Vec<GeneratedItem>,
        explanation: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GeneratedItem {
    pub id: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GeneratedOption {
    pub id: String,
    pub content: String,
    pub rationale: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GeneratedPair {
    pub id: String,
    pub left: String,
    pub right: String,
    pub rationale: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GeneratedGroupedItem {
    pub id: String,
    pub content: String,
    pub category_id: String,
    pub rationale: Option<String>,
}

impl GeneratedQuestion {
    /// Stable transport components for per-option/relation/step grounding.
    /// Explanations are grounded with their component, not an unrelated global
    /// source list. Only the generation owner assigns epistemic meaning to links.
    pub fn evidence_keys(&self) -> Vec<String> {
        let mut keys = vec!["prompt".to_string()];
        match self {
            Self::SingleChoice { options, .. } | Self::SelectAll { options, .. } => {
                keys.extend(options.iter().map(|option| format!("option:{}", option.id)));
            }
            Self::TrueFalse { .. } => keys.push("statement".to_string()),
            Self::Matching { pairs, .. } => {
                keys.extend(pairs.iter().map(|pair| format!("pair:{}", pair.id)));
            }
            Self::Grouping {
                categories, items, ..
            } => {
                keys.extend(
                    categories
                        .iter()
                        .map(|item| format!("category:{}", item.id)),
                );
                keys.extend(items.iter().map(|item| format!("item:{}", item.id)));
            }
            Self::Ordering { items, .. } => {
                keys.extend(items.iter().map(|item| format!("item:{}", item.id)));
            }
        }
        keys
    }

    pub fn admit(&self) -> Result<ValidatedQuestion, CommandError> {
        let body = match self {
            Self::SingleChoice {
                prompt,
                options,
                correct_option_id,
                explanation,
            } => QuestionBody::SingleChoice {
                prompt: rich(prompt, "question.prompt")?,
                options: choices(options)?,
                correct_option_id: local_id("option", correct_option_id)?,
                explanation: optional(explanation, "question.explanation")?,
            },
            Self::SelectAll {
                prompt,
                options,
                correct_option_ids,
                explanation,
            } => {
                bounded(correct_option_ids.len())?;
                QuestionBody::SelectAll {
                    prompt: rich(prompt, "question.prompt")?,
                    options: choices(options)?,
                    correct_option_ids: correct_option_ids
                        .iter()
                        .map(|id| local_id("option", id))
                        .collect::<Result<_, _>>()?,
                    explanation: optional(explanation, "question.explanation")?,
                }
            }
            Self::TrueFalse {
                statement,
                correct,
                explanation,
                corrected_statement,
            } => QuestionBody::TrueFalse {
                statement: rich(statement, "question.statement")?,
                correct: *correct,
                explanation: optional(explanation, "question.explanation")?,
                corrected_statement: optional(corrected_statement, "question.correctedStatement")?,
            },
            Self::Matching {
                prompt,
                pairs,
                explanation,
            } => {
                bounded(pairs.len())?;
                let pairs = pairs
                    .iter()
                    .map(|pair| {
                        Ok(MatchingPair {
                            left: RichItem {
                                id: local_id("left", &pair.id)?,
                                content: rich(&pair.left, "question.pairs.left")?,
                            },
                            right: RichItem {
                                id: local_id("right", &pair.id)?,
                                content: rich(&pair.right, "question.pairs.right")?,
                            },
                            rationale: optional(&pair.rationale, "question.pairs.rationale")?,
                        })
                    })
                    .collect::<Result<_, CommandError>>()?;
                QuestionBody::Matching {
                    prompt: rich(prompt, "question.prompt")?,
                    pairs,
                    explanation: optional(explanation, "question.explanation")?,
                }
            }
            Self::Grouping {
                prompt,
                categories,
                items,
                explanation,
            } => {
                bounded(categories.len())?;
                bounded(items.len())?;
                let categories = categories
                    .iter()
                    .map(|item| rich_item(item, "category"))
                    .collect::<Result<_, _>>()?;
                let items = items
                    .iter()
                    .map(|item| {
                        Ok(GroupedItem {
                            item: RichItem {
                                id: local_id("item", &item.id)?,
                                content: rich(&item.content, "question.items.content")?,
                            },
                            category_id: local_id("category", &item.category_id)?,
                            rationale: optional(&item.rationale, "question.items.rationale")?,
                        })
                    })
                    .collect::<Result<_, CommandError>>()?;
                QuestionBody::Grouping {
                    prompt: rich(prompt, "question.prompt")?,
                    categories,
                    items,
                    explanation: optional(explanation, "question.explanation")?,
                }
            }
            Self::Ordering {
                prompt,
                items,
                explanation,
            } => {
                bounded(items.len())?;
                QuestionBody::Ordering {
                    prompt: rich(prompt, "question.prompt")?,
                    items: items
                        .iter()
                        .map(|item| rich_item(item, "item"))
                        .collect::<Result<_, _>>()?,
                    explanation: optional(explanation, "question.explanation")?,
                }
            }
        };
        let admitted = ValidatedQuestion::validate(QuestionDefinition {
            schema_version: QUESTION_SCHEMA_VERSION,
            question: body,
        })?;
        // Text generation cannot invent local media ownership or remotely fetch
        // a source. Manual managed-image authoring is a separate capability.
        if !admitted.asset_ids().is_empty() {
            return Err(invalid(
                "Generated text questions cannot introduce managed assets.",
            ));
        }
        let keys = self.evidence_keys();
        if keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
            return Err(invalid("Generated component keys must be unique."));
        }
        Ok(admitted)
    }
}

fn bounded(count: usize) -> Result<(), CommandError> {
    if count > MAX_ITEMS {
        return Err(invalid("The generated question has too many components."));
    }
    Ok(())
}

fn local_id(role: &str, key: &str) -> Result<String, CommandError> {
    if key.is_empty()
        || key.len() > 64
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(invalid(
            "Component keys must be 1–64 ASCII letters, digits, '_' or '-'.",
        ));
    }
    let hash = format!(
        "{:x}",
        Sha256::digest(format!("generated-question:v1:{role}:{key}"))
    );
    Ok(hash[..32].to_string())
}

fn choices(options: &[GeneratedOption]) -> Result<Vec<ChoiceOption>, CommandError> {
    bounded(options.len())?;
    options
        .iter()
        .map(|option| {
            Ok(ChoiceOption {
                id: local_id("option", &option.id)?,
                content: rich(&option.content, "question.options.content")?,
                rationale: optional(&option.rationale, "question.options.rationale")?,
            })
        })
        .collect()
}

fn rich_item(item: &GeneratedItem, role: &str) -> Result<RichItem, CommandError> {
    Ok(RichItem {
        id: local_id(role, &item.id)?,
        content: rich(&item.content, "question.items.content")?,
    })
}

fn rich(text: &str, path: &str) -> Result<Value, CommandError> {
    markdown_document(text, path)
}

fn optional(text: &Option<String>, path: &str) -> Result<Option<Value>, CommandError> {
    text.as_deref()
        .filter(|text| !text.trim().is_empty())
        .map(|text| rich(text, path))
        .transpose()
}

fn invalid(message: &'static str) -> CommandError {
    CommandError::new("invalid_generated_question", message)
}
