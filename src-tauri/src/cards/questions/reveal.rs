//! Self-assessed reveal evidence. There is deliberately no objective Evaluation
//! or synthetic learner response; scheduling is a separate repository operation.

use super::*;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevealAnswerRequest {
    pub reveal_id: String,
    pub expected_revision: i64,
    pub response_time_ms: i64,
}

#[derive(Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RevealedAnswer {
    request: RevealAnswerRequest,
    revealed_at_ms: i64,
}

impl RevealedAnswer {
    pub fn view(&self, instance: &QuestionInstance) -> Result<AttemptProgress, QuestionError> {
        let target = instance.occlusion_target()?;
        Ok(AttemptProgress::Revealed {
            reveal_id: self.request.reveal_id.clone(),
            response_time_ms: self.request.response_time_ms,
            revealed_at_ms: self.revealed_at_ms,
            answer: target.answer.clone(),
            explanation: target.explanation.clone(),
        })
    }

    pub fn validate(
        &self,
        instance: &QuestionInstance,
        revision: i64,
        assisted: bool,
    ) -> Result<(), QuestionError> {
        instance.occlusion_target()?;
        validate_entity_id(&self.request.reveal_id, "answer reveal")?;
        validate_elapsed(self.request.response_time_ms)?;
        let expected = if assisted { 2 } else { 1 };
        if self.revealed_at_ms < 0
            || self.request.expected_revision != expected
            || next_revision(expected)? != revision
        {
            return Err(invalid(
                "The self-assessed reveal does not match its captured evidence.",
            ));
        }
        Ok(())
    }
}

impl QuestionAttempt {
    pub fn reveal_answer(
        &mut self,
        request: RevealAnswerRequest,
        now_ms: i64,
    ) -> Result<AttemptView, QuestionError> {
        self.instance.occlusion_target()?;
        validate_entity_id(&request.reveal_id, "answer reveal")?;
        validate_elapsed(request.response_time_ms)?;
        if let AttemptState::Revealed { record } = &self.state {
            if record.request.reveal_id != request.reveal_id {
                return Err(QuestionError::new(
                    "question_already_revealed",
                    "The image answer has already been revealed. Reload its saved state.",
                ));
            }
            if record.request != request {
                return Err(identity_conflict());
            }
            return self.view();
        }
        if !matches!(self.state, AttemptState::Answering { saved: None }) {
            return Err(invalid("Only an unanswered image question can be revealed."));
        }
        self.require_revision(request.expected_revision)?;
        if now_ms < 0 {
            return Err(invalid("The native reveal time is invalid."));
        }
        let revision = next_revision(self.revision)?;
        self.state = AttemptState::Revealed {
            record: RevealedAnswer {
                request,
                revealed_at_ms: now_ms,
            },
        };
        self.revision = revision;
        self.view()
    }
}

#[cfg(test)]
#[path = "reveal_tests.rs"]
mod tests;
