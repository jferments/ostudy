//! Retry-safe objective and self-assessed attempt state. The session repository must persist each
//! accepted transition in its transaction before acknowledging it. This value
//! does not open a database, update a schedule, or count a review.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{
    bounded_size, invalid, ComponentFeedback, Evaluation, LearnerAnswer, PublicQuestion,
    QuestionError, QuestionInstance, QuestionResponse, MAX_DEFINITION_BYTES, MAX_RESPONSE_BYTES,
};
use crate::{cards::collect_asset_ids, util::validate_entity_id};

#[path = "reveal.rs"]
mod reveal;
pub use reveal::RevealAnswerRequest;
use reveal::RevealedAnswer;

const ATTEMPT_VERSION: u32 = 1;
const MAX_ATTEMPT_BYTES: usize = 4 * MAX_DEFINITION_BYTES;
const MAX_RESPONSE_TIME_MS: i64 = 86_400_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAnswerRequest {
    pub mutation_id: String,
    pub expected_revision: i64,
    pub answer: LearnerAnswer,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckAnswerRequest {
    pub check_id: String,
    pub expected_revision: i64,
    pub answer: LearnerAnswer,
    pub response_time_ms: i64,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CheckedAnswer {
    request: CheckAnswerRequest,
    checked_at_ms: i64,
    evaluation: Evaluation,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
enum AttemptState {
    Answering { saved: Option<SaveAnswerRequest> },
    Checked { record: Box<CheckedAnswer> },
    Revealed { record: RevealedAnswer },
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredAttempt {
    attempt_version: u32,
    instance: String,
    revision: i64,
    assisted: bool,
    state: AttemptState,
}

/// Only this public projection may cross the pre-answer UI boundary. The
/// private serialized attempt always includes the captured answer key.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptView {
    pub instance_id: String,
    pub revision: i64,
    pub assisted: bool,
    pub question: PublicQuestion,
    pub progress: AttemptProgress,
}

impl AttemptView {
    pub fn asset_ids(&self) -> Result<Vec<String>, QuestionError> {
        let mut ids: BTreeSet<_> = self.question.asset_ids()?.into_iter().collect();
        if let AttemptProgress::Checked { evaluation, .. } = &self.progress {
            let mut fields = Vec::new();
            if let Some(explanation) = &evaluation.explanation {
                fields.push(explanation);
            }
            for component in &evaluation.components {
                let document = match component {
                    ComponentFeedback::Choice { rationale, .. }
                    | ComponentFeedback::Placement { rationale, .. } => rationale,
                    ComponentFeedback::Boolean {
                        corrected_statement,
                        ..
                    } => corrected_statement,
                    ComponentFeedback::Position { .. } => continue,
                };
                if let Some(document) = document {
                    fields.push(document);
                }
            }
            for document in fields {
                ids.extend(collect_asset_ids(document)?);
            }
        }
        if let AttemptProgress::Revealed {
            answer, explanation, ..
        } = &self.progress
        {
            ids.extend(collect_asset_ids(answer)?);
            if let Some(explanation) = explanation {
                ids.extend(collect_asset_ids(explanation)?);
            }
        }
        Ok(ids.into_iter().collect())
    }
}

#[derive(Clone, Serialize)]
#[serde(
    tag = "phase",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AttemptProgress {
    Answering {
        saved_answer: Option<LearnerAnswer>,
    },
    Checked {
        check_id: String,
        first_answer: LearnerAnswer,
        response_time_ms: i64,
        checked_at_ms: i64,
        evaluation: Evaluation,
    },
    Revealed {
        reveal_id: String,
        response_time_ms: i64,
        revealed_at_ms: i64,
        answer: serde_json::Value,
        explanation: Option<serde_json::Value>,
    },
}

#[derive(Clone)]
pub struct QuestionAttempt {
    instance: QuestionInstance,
    revision: i64,
    assisted: bool,
    state: AttemptState,
}

impl QuestionAttempt {
    pub fn start(instance: QuestionInstance) -> Result<Self, QuestionError> {
        // Image questions expose metadata only; the raster service authorizes pixels.
        instance.question_for_attempt()?;
        Ok(Self {
            instance,
            revision: 1,
            assisted: false,
            state: AttemptState::Answering { saved: None },
        })
    }

    pub fn revision(&self) -> i64 {
        self.revision
    }

    pub fn instance(&self) -> &QuestionInstance {
        &self.instance
    }

    pub fn view(&self) -> Result<AttemptView, QuestionError> {
        let progress = match &self.state {
            AttemptState::Answering { saved } => AttemptProgress::Answering {
                saved_answer: saved.as_ref().map(|request| request.answer.clone()),
            },
            AttemptState::Checked { record } => AttemptProgress::Checked {
                check_id: record.request.check_id.clone(),
                first_answer: record.request.answer.clone(),
                response_time_ms: record.request.response_time_ms,
                checked_at_ms: record.checked_at_ms,
                evaluation: record.evaluation.clone(),
            },
            AttemptState::Revealed { record } => record.view(&self.instance)?,
        };
        Ok(AttemptView {
            instance_id: self.instance.identity().instance_id.clone(),
            revision: self.revision,
            assisted: self.assisted,
            question: self.instance.question_for_attempt()?,
            progress,
        })
    }

    /// Replays the latest acknowledged save by identity, but never permits an
    /// older autosave to overwrite a newer response or an already checked one.
    pub fn save_answer(&mut self, mut request: SaveAnswerRequest) -> Result<i64, QuestionError> {
        validate_entity_id(&request.mutation_id, "answer mutation")?;
        request.answer = self.admit_answer(request.answer)?;
        let AttemptState::Answering { saved } = &self.state else {
            return Err(already_checked());
        };
        if let Some(previous) = saved {
            if previous.mutation_id == request.mutation_id {
                if previous != &request {
                    return Err(identity_conflict());
                }
                return next_revision(previous.expected_revision);
            }
        }
        self.require_revision(request.expected_revision)?;
        let revision = next_revision(self.revision)?;
        self.state = AttemptState::Answering {
            saved: Some(request),
        };
        self.revision = revision;
        Ok(revision)
    }

    /// Call only when assistance was actually exposed. Merely opening a Tutor
    /// or starting a failed hint request is not evidence of assisted study.
    pub fn mark_assisted(&mut self) -> Result<i64, QuestionError> {
        if !matches!(self.state, AttemptState::Answering { .. }) {
            return Err(already_checked());
        }
        if !self.assisted {
            self.revision = next_revision(self.revision)?;
            self.assisted = true;
        }
        Ok(self.revision)
    }

    /// The native caller supplies its observed time, not a client-authored
    /// timestamp. A retry returns the original time and immutable evaluation.
    pub fn check_answer(
        &mut self,
        mut request: CheckAnswerRequest,
        now_ms: i64,
    ) -> Result<AttemptView, QuestionError> {
        validate_entity_id(&request.check_id, "answer check")?;
        request.answer = self.admit_answer(request.answer)?;
        validate_elapsed(request.response_time_ms)?;
        if let AttemptState::Checked { record } = &self.state {
            if record.request.check_id != request.check_id {
                return Err(already_checked());
            }
            if record.request != request {
                return Err(identity_conflict());
            }
            return self.view();
        }
        self.require_revision(request.expected_revision)?;
        if now_ms < 0 {
            return Err(invalid("The native check time is invalid."));
        }
        let evaluation = self.instance.evaluate(&request.answer)?;
        let revision = next_revision(self.revision)?;
        let record = CheckedAnswer {
            request,
            checked_at_ms: now_ms,
            evaluation,
        };
        self.state = AttemptState::Checked {
            record: Box::new(record),
        };
        self.revision = revision;
        self.view()
    }

    /// Private checkpoint. Repository ownership, atomic writes and cleanup are
    /// separate responsibilities; obtaining this string is not a durable save.
    pub fn to_json(&self) -> Result<String, QuestionError> {
        let stored = StoredAttempt {
            attempt_version: ATTEMPT_VERSION,
            instance: self.instance.to_json()?,
            revision: self.revision,
            assisted: self.assisted,
            state: self.state.clone(),
        };
        bounded_size(&stored, MAX_ATTEMPT_BYTES)?;
        serde_json::to_string(&stored).map_err(|_| invalid("The attempt could not be encoded."))
    }

    pub fn restore(json: &str) -> Result<Self, QuestionError> {
        if json.len() > MAX_ATTEMPT_BYTES {
            return Err(invalid("The attempt exceeds its byte budget."));
        }
        let stored: StoredAttempt = serde_json::from_str(json)
            .map_err(|_| invalid("The attempt has an unsupported or malformed structure."))?;
        if stored.attempt_version != ATTEMPT_VERSION {
            return Err(QuestionError::new(
                "unsupported_question_attempt",
                "This question-attempt version is not supported.",
            ));
        }
        let mut attempt = Self::start(QuestionInstance::restore(&stored.instance)?)?;
        attempt.revision = stored.revision;
        attempt.assisted = stored.assisted;
        attempt.state = stored.state;
        attempt.validate_restored()?;
        Ok(attempt)
    }

    /// Validate and normalize a response without exposing feedback or changing the first attempt.
    pub fn admit_answer(&self, answer: LearnerAnswer) -> Result<LearnerAnswer, QuestionError> {
        bounded_size(&answer, MAX_RESPONSE_BYTES)?;
        // Validation does not expose feedback. Duplicate/unknown members fail
        // BEFORE normalization; set ordering is not a new logical answer.
        self.instance.evaluate(&answer)?;
        Ok(normalize_answer(answer))
    }

    fn require_revision(&self, revision: i64) -> Result<(), QuestionError> {
        if revision != self.revision {
            return Err(QuestionError::new(
                "question_attempt_conflict",
                "The attempt changed. Reload it before submitting another change.",
            ));
        }
        Ok(())
    }

    fn validate_restored(&self) -> Result<(), QuestionError> {
        if self.revision < 1 {
            return Err(invalid("The attempt revision is invalid."));
        }
        match &self.state {
            AttemptState::Answering { saved: None } => {
                let expected = if self.assisted { 2 } else { 1 };
                if self.revision != expected {
                    return Err(invalid("The unanswered attempt has an invalid revision."));
                }
            }
            AttemptState::Answering { saved: Some(saved) } => {
                validate_entity_id(&saved.mutation_id, "answer mutation")?;
                let normalized = self.admit_answer(saved.answer.clone())?;
                let accepted = next_revision(saved.expected_revision)?;
                if normalized != saved.answer
                    || accepted > self.revision
                    || (accepted != self.revision
                        && (!self.assisted || next_revision(accepted)? != self.revision))
                {
                    return Err(invalid("The saved answer does not match its receipt."));
                }
            }
            AttemptState::Checked { record } => {
                validate_entity_id(&record.request.check_id, "answer check")?;
                validate_elapsed(record.request.response_time_ms)?;
                let normalized = self.admit_answer(record.request.answer.clone())?;
                let evaluation = self.instance.evaluate(&normalized)?;
                if normalized != record.request.answer
                    || record.evaluation != evaluation
                    || record.checked_at_ms < 0
                    || next_revision(record.request.expected_revision)? != self.revision
                {
                    return Err(invalid(
                        "The checked answer does not match its frozen evidence.",
                    ));
                }
            }
            AttemptState::Revealed { record } => {
                record.validate(&self.instance, self.revision, self.assisted)?;
            }
        }
        Ok(())
    }
}

fn normalize_answer(mut answer: LearnerAnswer) -> LearnerAnswer {
    match &mut answer.response {
        QuestionResponse::SelectAll {
            selected_option_ids,
        } => selected_option_ids.sort(),
        QuestionResponse::Matching { assignments } | QuestionResponse::Grouping { assignments } => {
            assignments.sort_by(|left, right| left.item_id.cmp(&right.item_id));
        }
        _ => {}
    }
    answer
}

fn next_revision(revision: i64) -> Result<i64, QuestionError> {
    if revision < 1 {
        return Err(invalid("The attempt revision is invalid."));
    }
    revision
        .checked_add(1)
        .ok_or_else(|| invalid("The attempt revision limit was reached."))
}

fn validate_elapsed(milliseconds: i64) -> Result<(), QuestionError> {
    if !(0..=MAX_RESPONSE_TIME_MS).contains(&milliseconds) {
        return Err(invalid("The response time is outside the supported range."));
    }
    Ok(())
}

fn already_checked() -> QuestionError {
    QuestionError::new(
        "question_already_checked",
        "The first answer is already checked and cannot be replaced.",
    )
}

fn identity_conflict() -> QuestionError {
    QuestionError::new(
        "question_operation_conflict",
        "This operation identity was already used for a different answer.",
    )
}

#[cfg(test)]
#[path = "attempt_tests.rs"]
mod tests;
