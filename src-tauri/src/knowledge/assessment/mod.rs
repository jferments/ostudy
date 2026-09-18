//! Component-level assessment evidence over canonical knowledge. Providers select
//! references, never native hashes or verdicts of factual truth. Generation and
//! restored checkpoints use the same admission path; publishing stays atomic in
//! the existing authoring transaction.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    authoring::card_types_and_templates::generated_import::question::GeneratedQuestion,
    error::CommandError,
};

use super::{
    authoring::{KnowledgeCatalog, UnitSupport, CARD_SUPPORT_KEY},
    contracts::hash,
};

mod components;
pub(crate) mod policy;
mod schema;
pub(crate) mod settings;
pub(crate) mod store;
pub(crate) use schema::generation_schema;

pub(crate) const ASSESSMENT_KEY: &str = "_ostudyAssessment";
pub(crate) const CARD_POLICY_KEY: &str = "_ostudyGenerationPolicy";
pub(crate) const ASSESSMENT_VERSION: i64 = 1;
pub(crate) const MAX_COMPONENTS: usize = 65;
pub(crate) const MAX_ASSESSMENT_UNITS: usize = 128;
const MAX_COMPONENT_UNITS: usize = 2;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceStance {
    Supports,
    Contradicts,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ComponentProposal {
    pub key: String,
    pub stance: EvidenceStance,
    pub knowledge_unit_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AssessmentProposal {
    pub criterion: String,
    pub ambiguous: bool,
    pub components: Vec<ComponentProposal>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ComponentSupport {
    pub key: String,
    pub stance: EvidenceStance,
    pub units: Vec<UnitSupport>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AssessmentRecord {
    pub schema_version: i64,
    pub question_hash: String,
    pub opportunity_id: String,
    pub criterion: String,
    pub policy_hash: String,
    pub components: Vec<ComponentSupport>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TypedCard {
    #[serde(rename = "type")]
    kind: String,
    question: GeneratedQuestion,
    #[serde(default)]
    tags: Vec<String>,
}

pub(crate) fn admit_card(
    raw: Value,
    proposal: AssessmentProposal,
    catalog: &KnowledgeCatalog,
    policy: &policy::GenerationPolicy,
) -> Result<Value, CommandError> {
    policy.validate()?;
    let input: TypedCard = serde_json::from_value(raw)
        .map_err(|_| invalid("The generated interactive card has an invalid shape."))?;
    if input.kind != "interactive" || proposal.ambiguous {
        return Err(invalid(
            "Ambiguous assessment opportunities must be omitted.",
        ));
    }
    validate_criterion(&proposal.criterion)?;
    validate_tags(&input.tags)?;
    let question = input.question.admit()?;
    let family = policy::QuestionFamily::from_kind(question.kind().as_str())?;
    if !policy.allowed_types.contains(&family) {
        return Err(invalid("The model returned a disabled question type."));
    }
    let required = components::requirements(&input.question, policy.max_compound_items)?;
    let allowed = components::allowed_requirements(&input.question, policy.max_compound_items)?;
    if required.len() > MAX_COMPONENTS || proposal.components.len() > MAX_COMPONENTS {
        return Err(invalid("Assessment component evidence exceeds its bound."));
    }
    let mut components = BTreeMap::new();
    for component in proposal.components {
        if allowed.get(&component.key) != Some(&component.stance) {
            return Err(invalid(
                "A component's evidence role does not match the answer key.",
            ));
        }
        let mut units = catalog.select(&component.knowledge_unit_ids, MAX_COMPONENT_UNITS)?;
        units.sort_by(|a, b| a.unit_id.cmp(&b.unit_id));
        let key = component.key;
        let binding = ComponentSupport {
            key: key.clone(),
            stance: component.stance,
            units,
        };
        if components.insert(key, binding).is_some() {
            return Err(invalid(
                "An assessment repeats a component evidence binding.",
            ));
        }
    }
    let missing_required = required
        .keys()
        .filter(|key| !components.contains_key(*key))
        .count();
    if missing_required != 0 {
        crate::logging::warn_target(
            "ai.knowledge.assessment",
            "assessment_required_evidence_missing",
            "Generated assessment evidence omitted one or more graded answer components.",
            json!({
                "questionKind": question.kind().as_str(),
                "requiredComponents": required.len(),
                "providedComponents": components.len(),
                "missingRequiredComponents": missing_required,
            }),
        );
        return Err(invalid(
            "Every graded answer component requires its own evidence binding; prompt/category/context bindings are optional.",
        ));
    }
    let question_hash = hash(&question.to_json()?);
    let criterion = proposal.criterion.trim().to_string();
    let components = components.into_values().collect::<Vec<_>>();
    let opportunity_id = opportunity_id(&question_hash, &criterion, &components)?;
    let record = AssessmentRecord {
        schema_version: ASSESSMENT_VERSION,
        question_hash,
        opportunity_id,
        criterion,
        policy_hash: policy.fingerprint()?,
        components,
    };
    let support = record.support()?;
    let tags = input
        .tags
        .into_iter()
        .filter(|tag| {
            !matches!(
                tag.trim().to_ascii_lowercase().as_str(),
                "ai generated" | "ai-generated" | "ai_generated" | "generated by ai" | "pdf"
            )
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"type":"interactive", "question":input.question, "tags":tags,
        CARD_SUPPORT_KEY:support, CARD_POLICY_KEY:record.policy_hash, ASSESSMENT_KEY:record}),
    )
}

impl AssessmentRecord {
    pub(crate) fn support(&self) -> Result<Vec<UnitSupport>, CommandError> {
        if self.schema_version != ASSESSMENT_VERSION
            || self.components.is_empty()
            || self.components.len() > MAX_COMPONENTS
        {
            return Err(invalid("Unsupported or oversized assessment evidence."));
        }
        let mut keys = BTreeSet::new();
        let mut units = BTreeMap::new();
        for component in &self.components {
            if component.key.len() > 160
                || !keys.insert(&component.key)
                || component.units.is_empty()
                || component.units.len() > MAX_COMPONENT_UNITS
            {
                return Err(invalid("Invalid assessment component evidence."));
            }
            let mut selected = BTreeSet::new();
            for unit in &component.units {
                if !selected.insert(&unit.unit_id) {
                    return Err(invalid("A component repeats the same evidence unit."));
                }
                if let Some(previous) = units.insert(unit.unit_id.clone(), unit.clone()) {
                    if previous != *unit {
                        return Err(invalid(
                            "Assessment components disagree about a source revision.",
                        ));
                    }
                }
            }
        }
        if units.len() > MAX_ASSESSMENT_UNITS {
            return Err(invalid("Assessment evidence exceeds its source bound."));
        }
        Ok(units.into_values().collect())
    }

    pub(crate) fn validate_card(&self, card: &Value) -> Result<(), CommandError> {
        let content = super::authoring::without_card_support(card.clone());
        let input: TypedCard = serde_json::from_value(content)
            .map_err(|_| invalid("Stored assessment card content is invalid."))?;
        let question = input.question.admit()?;
        let required = components::requirements(&input.question, policy::MAX_COMPOUND_ITEMS)?;
        let allowed =
            components::allowed_requirements(&input.question, policy::MAX_COMPOUND_ITEMS)?;
        self.support()?;
        validate_criterion(&self.criterion)?;
        let opportunity = opportunity_id(&self.question_hash, &self.criterion, &self.components)?;
        let wrong_role = self
            .components
            .iter()
            .any(|component| allowed.get(&component.key) != Some(&component.stance));
        let provided = self
            .components
            .iter()
            .map(|component| component.key.as_str())
            .collect::<BTreeSet<_>>();
        let missing_required = required.keys().any(|key| !provided.contains(key.as_str()));
        if input.kind != "interactive"
            || self.question_hash != hash(&question.to_json()?)
            || missing_required
            || wrong_role
            || self.opportunity_id != opportunity
            || self.policy_hash.len() != 64
            || !self
                .policy_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid(
                "Assessment evidence does not match the captured question.",
            ));
        }
        Ok(())
    }
}

pub(crate) fn record_from_card(card: &Value) -> Result<Option<AssessmentRecord>, CommandError> {
    let Some(value) = card.get(ASSESSMENT_KEY) else {
        if card.get("type").and_then(Value::as_str) == Some("interactive") {
            return Err(invalid(
                "A generated interactive card is missing component evidence.",
            ));
        }
        return Ok(None);
    };
    let record: AssessmentRecord = serde_json::from_value(value.clone())
        .map_err(|_| invalid("The stored component evidence has an invalid shape."))?;
    record.validate_card(card)?;
    Ok(Some(record))
}

pub(crate) fn validates_checkpoint(
    card: &Value,
    catalog: &KnowledgeCatalog,
    policy: &policy::GenerationPolicy,
) -> bool {
    let Ok(policy_hash) = policy.fingerprint() else {
        return false;
    };
    if card.get(CARD_POLICY_KEY).and_then(Value::as_str) != Some(policy_hash.as_str()) {
        return false;
    }
    // Cached recall content must pass the same native preparation as new output.
    // Valid source IDs alone do not make malformed Markdown or cloze importable.
    use crate::authoring::card_types_and_templates::generated_import::validate_generated_card;
    if validate_generated_card(super::authoring::without_card_support(card.clone())).is_err() {
        return false;
    }
    let Ok(record) = record_from_card(card) else {
        return false;
    };
    let Ok(support) = super::authoring::card_support(card) else {
        return false;
    };
    match record {
        Some(record) => {
            record.policy_hash == policy_hash
                && record
                    .components
                    .iter()
                    .all(|component| catalog.validates(&component.units, MAX_COMPONENT_UNITS))
                && catalog.validates(&support, MAX_ASSESSMENT_UNITS)
        }
        None => catalog.validates(&support, 2),
    }
}

fn opportunity_id(
    question_hash: &str,
    criterion: &str,
    components: &[ComponentSupport],
) -> Result<String, CommandError> {
    let encoded =
        serde_json::to_string(&(ASSESSMENT_VERSION, question_hash, criterion, components))
            .map_err(CommandError::serialization)?;
    Ok(hash(&encoded))
}

fn validate_criterion(value: &str) -> Result<(), CommandError> {
    if value.trim().is_empty() || value.chars().count() > 500 || value.chars().any(char::is_control)
    {
        return Err(invalid(
            "An assessment needs one explicit, bounded relation or ordering criterion.",
        ));
    }
    Ok(())
}

fn validate_tags(tags: &[String]) -> Result<(), CommandError> {
    if tags.len() > 32
        || tags.iter().any(|tag| {
            tag.trim().is_empty()
                || tag.chars().count() > 64
                || tag.starts_with('#')
                || tag.chars().any(char::is_control)
        })
    {
        return Err(invalid("The generated question tags are invalid."));
    }
    Ok(())
}

pub(crate) fn invalid(message: &str) -> CommandError {
    CommandError::new("ai_document_output_invalid", message)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod publication_tests;
