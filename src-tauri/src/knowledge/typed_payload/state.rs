//! A failed optional annotation is durable debt, not a failed core claim and not
//! usable structured evidence. Provider proposals cannot select the native state.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{annotation_error, rejection_rule, ValidatedPayload};
use crate::{
    error::CommandError,
    knowledge::contracts::{hash, is_hash, KnowledgeKind},
};

mod repair;

const MAX_RETAINED_DRAFT_BYTES: usize = 12_000;

/// The validated variant has the exact historical serialization. A deferred
/// record has disjoint fields, so damaged v1 structures never decode as debt.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub(crate) enum TypedPayload {
    Validated(ValidatedPayload),
    Deferred(Box<DeferredAnnotation>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DeferredAnnotation {
    deferred_version: u8,
    kind: KnowledgeKind,
    statement_hash: String,
    rule: String,
    draft_hash: String,
    draft_bytes: usize,
    draft: Option<Value>,
}

impl TypedPayload {
    /// Strict admission remains available wherever a validated shape is required.
    pub(crate) fn admit(
        value: Value,
        statement: &str,
        kind: KnowledgeKind,
    ) -> Result<Self, CommandError> {
        ValidatedPayload::admit(value, statement, kind).map(Self::Validated)
    }

    /// Call only after independently admitting the core statement and grounding.
    /// No rewriting of statements, quantities, signs or qualifications occurs.
    pub(crate) fn admit_or_defer(
        value: Value,
        statement: &str,
        kind: KnowledgeKind,
    ) -> Result<Self, CommandError> {
        match Self::admit(value.clone(), statement, kind) {
            Ok(payload) => Ok(payload),
            Err(error) => {
                let rule = rejection_rule(&error);
                if !known_rule(rule) {
                    return Err(error);
                }
                let encoded = value.to_string();
                let deferred = Self::Deferred(Box::new(DeferredAnnotation {
                    deferred_version: 1,
                    kind,
                    statement_hash: hash(statement),
                    rule: rule.to_string(),
                    draft_hash: hash(&encoded),
                    draft_bytes: encoded.len(),
                    draft: (encoded.len() <= MAX_RETAINED_DRAFT_BYTES).then_some(value),
                }));
                Ok(deferred.repair_locally(statement, kind)?.unwrap_or(deferred))
            }
        }
    }

    /// Validate the stored state, not a claim of annotation validity. Consumers
    /// that need structured/graded support must also reject deferred state.
    pub(crate) fn validate(
        &self,
        statement: &str,
        kind: KnowledgeKind,
    ) -> Result<(), CommandError> {
        match self {
            Self::Validated(payload) => payload.validate(statement, kind),
            Self::Deferred(deferred) => deferred.validate(statement, kind),
        }
    }

    /// Resolve only an unambiguous representation defect in a retained draft.
    /// Stored bindings are checked first; strict admission checks the entire
    /// candidate afterwards. Neither failure nor ambiguity mutates the debt.
    pub(crate) fn repair_locally(
        &self,
        statement: &str,
        kind: KnowledgeKind,
    ) -> Result<Option<Self>, CommandError> {
        self.validate(statement, kind)?;
        let Self::Deferred(deferred) = self else {
            return Ok(None);
        };
        if !matches!(
            deferred.rule.as_str(),
            "knowledge_annotation_malformed"
                | "knowledge_annotation_occurrence_out_of_range"
        ) {
            return Ok(None);
        }
        Ok(deferred
            .draft
            .as_ref()
            .and_then(|draft| repair::draft(draft, statement, kind))
            .map(Self::Validated))
    }

    pub(crate) fn deferred_rule(&self) -> Option<&str> {
        match self {
            Self::Validated(_) => None,
            Self::Deferred(deferred) => Some(&deferred.rule),
        }
    }

    pub(crate) fn prompt(&self, statement: &str) -> Result<Value, CommandError> {
        match self {
            Self::Validated(payload) => payload.prompt(statement),
            Self::Deferred(deferred) => {
                deferred.validate(statement, deferred.kind)?;
                // Never supply the failed draft as evidence or ask the author to
                // repair it implicitly while generating an answer key.
                Ok(json!({
                    "annotationState": "deferred",
                    "gradedBindingsAllowed": false,
                    "use": "Core statement and qualifications remain available for recall and explanation. Do not use this unit for a graded interactive answer component."
                }))
            }
        }
    }

    pub(crate) fn diagnostic(&self) -> Option<Value> {
        match self {
            Self::Validated(_) => None,
            Self::Deferred(deferred) => Some(json!({
                "rule": deferred.rule,
                "draftFingerprint": deferred.draft_hash,
                "draftRetained": deferred.draft.is_some(),
                "draftBytes": deferred.draft_bytes,
            })),
        }
    }

    pub(crate) fn schema() -> Value {
        // The wire contract describes only proposed shapes, never native debt.
        ValidatedPayload::schema()
    }
}

impl DeferredAnnotation {
    fn validate(&self, statement: &str, kind: KnowledgeKind) -> Result<(), CommandError> {
        let invalid = || {
            annotation_error(
                "deferred_record_invalid",
                "The deferred annotation does not match its admitted core or retained draft.",
            )
        };
        if self.deferred_version != 1
            || self.kind != kind
            || self.statement_hash != hash(statement)
            || !is_hash(&self.draft_hash)
            || !known_rule(&self.rule)
        {
            return Err(invalid());
        }
        match &self.draft {
            Some(draft) => {
                let encoded = draft.to_string();
                if encoded.len() > MAX_RETAINED_DRAFT_BYTES
                    || encoded.len() != self.draft_bytes
                    || hash(&encoded) != self.draft_hash
                    || !ValidatedPayload::admit(draft.clone(), statement, kind)
                        .is_err_and(|error| rejection_rule(&error) == self.rule)
                {
                    return Err(invalid());
                }
            }
            None if self.draft_bytes <= MAX_RETAINED_DRAFT_BYTES => return Err(invalid()),
            None => {}
        }
        Ok(())
    }
}

fn known_rule(rule: &str) -> bool {
    matches!(
        rule,
        "knowledge_annotation_malformed"
            | "knowledge_annotation_structural_bounds"
            | "knowledge_annotation_selection_invalid"
            | "knowledge_annotation_selection_absent"
            | "knowledge_annotation_occurrence_out_of_range"
            | "knowledge_annotation_kind_mismatch"
            | "knowledge_annotation_operator_token_split"
            | "knowledge_annotation_numeric_token_split"
            | "knowledge_annotation_quantity_order"
            | "knowledge_annotation_quantity_operator"
            | "knowledge_annotation_quantity_literal"
            | "knowledge_annotation_dependency_invalid"
            | "knowledge_annotation_dependency_cycle"
    )
}

#[cfg(test)]
mod tests;
