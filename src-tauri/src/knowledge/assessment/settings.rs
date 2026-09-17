use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::policy::GenerationPolicy;
use crate::{database::Database, error::CommandError};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationPolicySnapshot {
    pub revision: i64,
    pub policy: GenerationPolicy,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveGenerationPolicyInput {
    pub revision: i64,
    pub policy: GenerationPolicy,
}

pub(crate) fn load(connection: &Connection) -> Result<GenerationPolicySnapshot, CommandError> {
    let stored: Option<(i64, String)> = connection
        .query_row(
            "SELECT revision, policy_json FROM question_generation_settings WHERE singleton = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let (revision, policy) = match stored {
        Some((revision, encoded)) => (
            revision,
            serde_json::from_str::<GenerationPolicy>(&encoded)
                .map_err(CommandError::serialization)?,
        ),
        None => (0, GenerationPolicy::default()),
    };
    policy.validate()?;
    Ok(GenerationPolicySnapshot { revision, policy })
}

#[tauri::command]
pub fn get_question_generation_policy(
    database: State<'_, Database>,
) -> Result<GenerationPolicySnapshot, CommandError> {
    database.with_connection(|connection| load(connection))
}

#[tauri::command]
pub fn save_question_generation_policy(
    database: State<'_, Database>,
    input: SaveGenerationPolicyInput,
) -> Result<GenerationPolicySnapshot, CommandError> {
    save(&database, input)
}

pub(crate) fn save(
    database: &Database,
    input: SaveGenerationPolicyInput,
) -> Result<GenerationPolicySnapshot, CommandError> {
    input.policy.validate()?;
    if input.revision < 0 || input.revision == i64::MAX {
        return Err(CommandError::new(
            "invalid_generation_policy",
            "The policy revision is invalid.",
        ));
    }
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let current = load(&transaction)?;
        if current.revision != input.revision {
            return Err(CommandError::new(
                "generation_policy_changed",
                "Generation preferences changed in another window. Reload them before saving.",
            ));
        }
        let revision = current.revision + 1;
        let encoded = serde_json::to_string(&input.policy).map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO question_generation_settings (singleton, revision, policy_json)
             VALUES (1, ?1, ?2) ON CONFLICT(singleton) DO UPDATE
             SET revision = excluded.revision, policy_json = excluded.policy_json",
                params![revision, encoded],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)?;
        Ok(GenerationPolicySnapshot {
            revision,
            policy: input.policy,
        })
    })
}
