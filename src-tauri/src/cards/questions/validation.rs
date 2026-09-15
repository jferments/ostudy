use std::collections::BTreeSet;

use serde_json::Value;

use super::{
    bounded_size, invalid, model::*, QuestionError, MAX_DEFINITION_BYTES, MAX_ITEMS,
    QUESTION_SCHEMA_VERSION,
};
use crate::{
    cards::{collect_asset_ids, validate_document},
    util::validate_entity_id,
};

/// The private fields prevent unvalidated definitions from reaching projection,
/// capture, and evaluation. Asset existence/ownership is a separate store check.
#[derive(Clone)]
pub struct ValidatedQuestion {
    pub(super) definition: QuestionDefinition,
    asset_ids: Vec<String>,
}

impl ValidatedQuestion {
    pub fn parse(json: &str) -> Result<Self, QuestionError> {
        if json.len() > MAX_DEFINITION_BYTES {
            return Err(invalid("The question exceeds its byte budget."));
        }
        let definition = serde_json::from_str(json)
            .map_err(|_| invalid("The question has an unsupported or malformed structure."))?;
        Self::validate(definition)
    }

    pub fn validate(definition: QuestionDefinition) -> Result<Self, QuestionError> {
        if definition.schema_version != QUESTION_SCHEMA_VERSION {
            return Err(QuestionError::new(
                "unsupported_question_version",
                "This question version is not supported.",
            ));
        }
        let mut fields = Fields::default();
        match &definition.question {
            QuestionBody::SingleChoice {
                prompt,
                options,
                correct_option_id,
                explanation,
            } => {
                fields.choices(prompt, options, explanation)?;
                member(
                    correct_option_id,
                    options.iter().map(|option| option.id.as_str()),
                )?;
            }
            QuestionBody::SelectAll {
                prompt,
                options,
                correct_option_ids,
                explanation,
            } => {
                fields.choices(prompt, options, explanation)?;
                subset(
                    correct_option_ids,
                    options.iter().map(|option| option.id.as_str()),
                )?;
            }
            QuestionBody::TrueFalse {
                statement,
                explanation,
                corrected_statement,
                ..
            } => {
                fields.rich(statement, true)?;
                fields.optional(explanation)?;
                fields.optional(corrected_statement)?;
            }
            QuestionBody::Matching {
                prompt,
                pairs,
                explanation,
            } => {
                cardinality(pairs.len(), 2, 16)?;
                fields.rich(prompt, true)?;
                for pair in pairs {
                    fields.item(&pair.left)?;
                    fields.item(&pair.right)?;
                    fields.optional(&pair.rationale)?;
                }
                distinct_content(pairs.iter().map(|pair| &pair.left.content))?;
                distinct_content(pairs.iter().map(|pair| &pair.right.content))?;
                fields.optional(explanation)?;
            }
            QuestionBody::Grouping {
                prompt,
                categories,
                items,
                explanation,
            } => {
                cardinality(categories.len(), 2, 8)?;
                cardinality(items.len(), 2, MAX_ITEMS)?;
                fields.rich(prompt, true)?;
                for category in categories {
                    fields.item(category)?;
                }
                for grouped in items {
                    fields.item(&grouped.item)?;
                    member(
                        &grouped.category_id,
                        categories.iter().map(|category| category.id.as_str()),
                    )?;
                    fields.optional(&grouped.rationale)?;
                }
                distinct_content(categories.iter().map(|category| &category.content))?;
                distinct_content(items.iter().map(|grouped| &grouped.item.content))?;
                fields.optional(explanation)?;
            }
            QuestionBody::Ordering {
                prompt,
                items,
                explanation,
            } => {
                cardinality(items.len(), 2, MAX_ITEMS)?;
                fields.rich(prompt, true)?;
                for item in items {
                    fields.item(item)?;
                }
                distinct_content(items.iter().map(|item| &item.content))?;
                fields.optional(explanation)?;
            }
            QuestionBody::ImageOcclusion {
                image_asset_id,
                masks,
                questions,
            } => {
                cardinality(masks.len(), 1, 128)?;
                cardinality(questions.len(), 1, 64)?;
                validate_entity_id(image_asset_id, "question image")?;
                fields.assets.insert(image_asset_id.clone());
                for mask in masks {
                    fields.id(&mask.id)?;
                    let rect = mask.rect;
                    if ![rect.x, rect.y, rect.width, rect.height]
                        .into_iter()
                        .all(f64::is_finite)
                        || rect.x < 0.0
                        || rect.y < 0.0
                        || rect.width <= 0.0
                        || rect.height <= 0.0
                        || rect.x + rect.width > 1.0
                        || rect.y + rect.height > 1.0
                    {
                        return Err(invalid(
                            "An image mask must be a finite, positive rectangle inside the image.",
                        ));
                    }
                }
                for question in questions {
                    fields.id(&question.id)?;
                    cardinality(question.mask_ids.len(), 1, masks.len())?;
                    subset(&question.mask_ids, masks.iter().map(|mask| mask.id.as_str()))?;
                    fields.rich(&question.prompt, true)?;
                    fields.rich(&question.answer, true)?;
                    fields.optional(&question.explanation)?;
                }
            }
        }
        bounded_size(&definition, MAX_DEFINITION_BYTES)?;
        Ok(Self {
            definition,
            asset_ids: fields.assets.into_iter().collect(),
        })
    }

    /// Authoring/storage only. Never use this as a pre-answer Practice DTO.
    pub fn definition(&self) -> &QuestionDefinition {
        &self.definition
    }

    pub fn kind(&self) -> QuestionKind {
        self.definition.question.kind()
    }

    pub fn asset_ids(&self) -> &[String] {
        &self.asset_ids
    }

    pub fn to_json(&self) -> Result<String, QuestionError> {
        serde_json::to_string(&self.definition)
            .map_err(|_| invalid("The validated question could not be encoded."))
    }

    pub fn presentation_keys(&self) -> Vec<String> {
        match &self.definition.question {
            QuestionBody::ImageOcclusion { questions, .. } => {
                questions.iter().map(|question| question.id.clone()).collect()
            }
            _ => vec!["main".to_string()],
        }
    }
}

#[derive(Default)]
struct Fields {
    ids: BTreeSet<String>,
    assets: BTreeSet<String>,
    bytes: usize,
}

impl Fields {
    fn id(&mut self, id: &str) -> Result<(), QuestionError> {
        validate_entity_id(id, "question item")?;
        if !self.ids.insert(id.to_string()) {
            return Err(invalid("Question item identities must be unique."));
        }
        Ok(())
    }

    fn rich(&mut self, document: &Value, required: bool) -> Result<(), QuestionError> {
        // Leave headroom for the definition and snapshot envelopes under Serde's
        // default recursion limit. Every admitted field must parse on reopening.
        let mut pending = vec![(document, 0_usize)];
        let mut visited = 0_usize;
        while let Some((value, depth)) = pending.pop() {
            visited += 1;
            if depth > 64 {
                return Err(invalid("A question field is nested too deeply."));
            }
            match value {
                Value::Array(values) => {
                    if values.len() > 100_000_usize.saturating_sub(visited + pending.len()) {
                        return Err(invalid("A question field contains too many values."));
                    }
                    pending.extend(values.iter().map(|value| (value, depth + 1)));
                }
                Value::Object(values) => {
                    if values.len() > 100_000_usize.saturating_sub(visited + pending.len()) {
                        return Err(invalid("A question field contains too many values."));
                    }
                    pending.extend(values.values().map(|value| (value, depth + 1)));
                }
                _ => {}
            }
        }
        validate_document(document)?;
        self.bytes += bounded_size(
            document,
            128_000.min(MAX_DEFINITION_BYTES.saturating_sub(self.bytes)),
        )?;
        let assets = collect_asset_ids(document)?;
        if required && assets.is_empty() && !has_meaningful_text(document) {
            return Err(invalid("A required question field is empty."));
        }
        self.assets.extend(assets);
        Ok(())
    }

    fn optional(&mut self, document: &Option<Value>) -> Result<(), QuestionError> {
        if let Some(document) = document {
            self.rich(document, false)?;
        }
        Ok(())
    }

    fn item(&mut self, item: &RichItem) -> Result<(), QuestionError> {
        self.id(&item.id)?;
        self.rich(&item.content, true)
    }

    fn choices(
        &mut self,
        prompt: &Value,
        options: &[ChoiceOption],
        explanation: &Option<Value>,
    ) -> Result<(), QuestionError> {
        cardinality(options.len(), 2, MAX_ITEMS)?;
        self.rich(prompt, true)?;
        for option in options {
            self.id(&option.id)?;
            self.rich(&option.content, true)?;
            self.optional(&option.rationale)?;
        }
        distinct_content(options.iter().map(|option| &option.content))?;
        self.optional(explanation)
    }
}

// Called only after the existing rich validator has bounded and admitted nodes.
fn has_meaningful_text(document: &Value) -> bool {
    let mut pending = vec![document];
    while let Some(node) = pending.pop() {
        let kind = node.get("type").and_then(Value::as_str);
        if kind == Some("text")
            && node
                .get("text")
                .and_then(Value::as_str)
                .is_some_and(|text| !text.trim().is_empty())
        {
            return true;
        }
        if matches!(kind, Some("inlineMath" | "blockMath")) {
            return true;
        }
        if let Some(children) = node.get("content").and_then(Value::as_array) {
            pending.extend(children);
        }
    }
    false
}

fn distinct_content<'a>(documents: impl Iterator<Item = &'a Value>) -> Result<(), QuestionError> {
    let mut seen = Vec::new();
    for document in documents {
        if seen.contains(&document) {
            return Err(invalid(
                "Items on the same side must not have identical content.",
            ));
        }
        seen.push(document);
    }
    Ok(())
}

pub(super) fn cardinality(count: usize, minimum: usize, maximum: usize) -> Result<(), QuestionError> {
    if count < minimum || count > maximum {
        return Err(invalid("The question has an unsupported number of items."));
    }
    Ok(())
}

pub(super) fn member<'a>(
    id: &str,
    mut allowed: impl Iterator<Item = &'a str>,
) -> Result<(), QuestionError> {
    if !allowed.any(|candidate| candidate == id) {
        return Err(invalid(
            "A question reference does not identify an available item.",
        ));
    }
    Ok(())
}

pub(super) fn subset<'a>(
    ids: &[String],
    allowed: impl Iterator<Item = &'a str>,
) -> Result<(), QuestionError> {
    let allowed: BTreeSet<_> = allowed.collect();
    cardinality(ids.len(), 0, allowed.len())?;
    let mut seen = BTreeSet::new();
    for id in ids {
        if !allowed.contains(id.as_str()) || !seen.insert(id.as_str()) {
            return Err(invalid(
                "Question references must be unique and identify available items.",
            ));
        }
    }
    Ok(())
}

pub(super) fn permutation<'a>(
    ids: &[String],
    allowed: impl Iterator<Item = &'a str>,
) -> Result<(), QuestionError> {
    let allowed: Vec<_> = allowed.collect();
    if ids.len() != allowed.len() {
        return Err(invalid(
            "A displayed order must contain every item exactly once.",
        ));
    }
    subset(ids, allowed.into_iter())
}
