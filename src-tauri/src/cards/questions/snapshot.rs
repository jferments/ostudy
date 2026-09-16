use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    invalid, model::*, QuestionError, ValidatedQuestion, GRADING_POLICY_VERSION,
    MAX_DEFINITION_BYTES, MAX_RESPONSE_BYTES, QUESTION_SCHEMA_VERSION,
};
use crate::util::validate_entity_id;

#[path = "occlusion_snapshot.rs"]
mod occlusion_snapshot;

const SNAPSHOT_VERSION: u32 = 1;
const MAX_SNAPSHOT_BYTES: usize = MAX_DEFINITION_BYTES + 16_384;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceIdentity {
    pub instance_id: String,
    pub note_id: String,
    pub card_id: String,
    pub note_revision: i64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredInstance {
    snapshot_version: u32,
    grading_policy_version: u32,
    identity: InstanceIdentity,
    definition: QuestionDefinition,
    presentation_key: String,
    display_order: DisplayOrder,
}

/// An immutable in-memory instance, ready for the existing transactional session
/// owner to persist. Restoring never reads a current note or chooses a new order.
/// This is not itself a database commit, review receipt, or durable attempt.
#[derive(Clone)]
pub struct QuestionInstance {
    identity: InstanceIdentity,
    question: ValidatedQuestion,
    presentation_key: String,
    display_order: DisplayOrder,
    fingerprint: String,
}

impl QuestionInstance {
    pub fn capture(
        identity: InstanceIdentity,
        question: ValidatedQuestion,
        presentation_key: String,
        display_order: DisplayOrder,
    ) -> Result<Self, QuestionError> {
        validate_entity_id(&identity.instance_id, "question instance")?;
        validate_entity_id(&identity.note_id, "question note")?;
        validate_entity_id(&identity.card_id, "question card")?;
        if identity.note_revision < 1 {
            return Err(invalid(
                "A question instance requires a positive note revision.",
            ));
        }
        if !question.presentation_keys().contains(&presentation_key) {
            return Err(invalid("The question presentation does not exist."));
        }
        question.validate_order(&display_order)?;
        let mut instance = Self {
            identity,
            question,
            presentation_key,
            display_order,
            fingerprint: String::new(),
        };
        instance.fingerprint = format!("{:x}", Sha256::digest(instance.to_json()?.as_bytes()));
        Ok(instance)
    }

    pub fn restore(json: &str) -> Result<Self, QuestionError> {
        if json.len() > MAX_SNAPSHOT_BYTES {
            return Err(invalid("The captured question exceeds its byte budget."));
        }
        let stored: StoredInstance = serde_json::from_str(json).map_err(|_| {
            invalid("The captured question has an unsupported or malformed structure.")
        })?;
        if stored.snapshot_version != SNAPSHOT_VERSION
            || stored.grading_policy_version != GRADING_POLICY_VERSION
        {
            return Err(QuestionError::new(
                "unsupported_question_snapshot",
                "This captured question or grading policy is not supported.",
            ));
        }
        Self::capture(
            stored.identity,
            ValidatedQuestion::validate(stored.definition)?,
            stored.presentation_key,
            stored.display_order,
        )
    }

    /// Private storage output includes the frozen key. Never send this JSON to
    /// Practice, a Tutor hint, diagnostics, or an untrusted provider.
    pub fn to_json(&self) -> Result<String, QuestionError> {
        let stored = StoredInstance {
            snapshot_version: SNAPSHOT_VERSION,
            grading_policy_version: GRADING_POLICY_VERSION,
            identity: self.identity.clone(),
            definition: self.question.definition().clone(),
            presentation_key: self.presentation_key.clone(),
            display_order: self.display_order.clone(),
        };
        super::bounded_size(&stored, MAX_SNAPSHOT_BYTES)?;
        serde_json::to_string(&stored)
            .map_err(|_| invalid("The captured question could not be encoded."))
    }

    pub fn identity(&self) -> &InstanceIdentity {
        &self.identity
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn asset_ids(&self) -> &[String] {
        self.question.asset_ids()
    }

    pub fn public_question(&self) -> Result<PublicQuestion, QuestionError> {
        self.question.project(&self.display_order)
    }

    pub fn occlusion_mask_plan(&self) -> Result<super::OcclusionMaskPlan, QuestionError> {
        self.question.occlusion_mask_plan(&self.presentation_key)
    }

    /// Local objective evaluation only; the caller owns check-id idempotency,
    /// first-attempt persistence, preview revisions, and the one scheduling write.
    pub fn evaluate(&self, answer: &LearnerAnswer) -> Result<Evaluation, QuestionError> {
        if answer.schema_version != QUESTION_SCHEMA_VERSION {
            return Err(QuestionError::new(
                "unsupported_question_response",
                "This response version is not supported.",
            ));
        }
        if answer.instance_id != self.identity.instance_id {
            return Err(invalid(
                "The response belongs to a different question instance.",
            ));
        }
        self.question.evaluate(&answer.response)
    }

    pub fn evaluate_json(&self, json: &str) -> Result<Evaluation, QuestionError> {
        if json.len() > MAX_RESPONSE_BYTES {
            return Err(invalid("The response exceeds its byte budget."));
        }
        let answer: LearnerAnswer = serde_json::from_str(json)
            .map_err(|_| invalid("The response has an unsupported or malformed structure."))?;
        self.evaluate(&answer)
    }
}
