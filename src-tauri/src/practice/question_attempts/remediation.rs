//! Post-feedback practice is separate evidence, never a replacement first answer
//! and never an additional scheduling review.

use super::*;
use crate::cards::questions::{Evaluation, LearnerAnswer};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemediationInput {
    pub target: AttemptIdentityInput,
    pub operation_id: String,
    pub answer: LearnerAnswer,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemediationReceipt {
    pub operation_id: String,
    pub instance_id: String,
    pub evaluation: Evaluation,
    pub created_at: i64,
}

#[tauri::command]
pub fn remediate_practice_question(
    database: State<'_, Database>,
    input: RemediationInput,
) -> Result<RemediationReceipt, CommandError> {
    database.with_connection(|connection| remediate(connection, input))
}

pub(super) fn remediate(
    connection: &mut Connection,
    input: RemediationInput,
) -> Result<RemediationReceipt, CommandError> {
    validate_target(&input.target)?;
    validate_entity_id(&input.operation_id, "remediation operation")?;
    require_same_instance(&input.target, &input.answer.instance_id)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let stored = storage::load(&transaction, &input.target)?;
    if !matches!(
        stored.attempt.view()?.progress,
        AttemptProgress::Checked { .. }
    ) {
        return Err(CommandError::new(
            "question_not_checked",
            "Check the first answer before trying a remediation answer.",
        ));
    }
    let answer = stored.attempt.admit_answer(input.answer)?;
    let answer_json = serde_json::to_string(&answer).map_err(CommandError::serialization)?;
    let request_hash = format!("{:x}", Sha256::digest(answer_json.as_bytes()));
    let evaluation = stored.attempt.instance().evaluate(&answer)?;
    let prior: Option<(String, String, String, i64)> = transaction
        .query_row(
            "SELECT instance_id, request_hash, evaluation_json, created_at
         FROM question_remediation_attempts WHERE operation_id = ?1",
            [&input.operation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let now = if let Some(prior) = prior {
        if prior.0 != input.target.instance_id || prior.1 != request_hash {
            return Err(CommandError::new(
                "question_operation_conflict",
                "That remediation identity belongs to a different answer.",
            ));
        }
        let saved: Evaluation =
            serde_json::from_str(&prior.2).map_err(CommandError::serialization)?;
        if saved != evaluation {
            return Err(invalid_storage());
        }
        prior.3
    } else {
        stored.require_available()?;
        let now = unix_time_millis()?;
        let evaluation_json =
            serde_json::to_string(&evaluation).map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO question_remediation_attempts (
                 operation_id, instance_id, request_hash, answer_json, evaluation_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    input.operation_id,
                    input.target.instance_id,
                    request_hash,
                    answer_json,
                    evaluation_json,
                    now
                ],
            )
            .map_err(CommandError::database)?;
        now
    };
    transaction.commit().map_err(CommandError::database)?;
    Ok(RemediationReceipt {
        operation_id: input.operation_id,
        instance_id: input.target.instance_id,
        evaluation,
        created_at: now,
    })
}
