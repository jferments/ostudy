//! Typed question-domain foundation. This module has no provider, UI, database,
//! filesystem, or scheduler dependency. Persistence services must admit managed
//! asset ownership and commit captured instances/checks transactionally.

mod attempt;
mod evaluation;
mod model;
mod projection;
mod snapshot;
mod validation;

pub use crate::error::CommandError as QuestionError;
pub use attempt::{
    AttemptProgress, AttemptView, CheckAnswerRequest, QuestionAttempt, SaveAnswerRequest,
};
pub use model::*;
pub use projection::OcclusionMaskPlan;
pub use snapshot::{InstanceIdentity, QuestionInstance};
pub use validation::ValidatedQuestion;

pub const QUESTION_SCHEMA_VERSION: u32 = 1;
pub const GRADING_POLICY_VERSION: u32 = 1;
pub const MAX_DEFINITION_BYTES: usize = 1_000_000;
pub const MAX_RESPONSE_BYTES: usize = 16_384;
pub const MAX_ITEMS: usize = 32;

fn invalid(message: &'static str) -> QuestionError {
    QuestionError::new("invalid_question", message)
}

/// Count without allocating a serialized copy. Rich-document depth is validated
/// BEFORE this writer is used; this is a byte bound, not a recursive parser.
fn bounded_size(value: &impl serde::Serialize, maximum: usize) -> Result<usize, QuestionError> {
    struct Counter {
        count: usize,
        maximum: usize,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.maximum.saturating_sub(self.count) {
                return Err(std::io::Error::other("question byte limit"));
            }
            self.count += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter { count: 0, maximum };
    serde_json::to_writer(&mut counter, value)
        .map_err(|_| invalid("The question data exceeds its byte budget."))?;
    Ok(counter.count)
}

#[cfg(test)]
mod tests;
