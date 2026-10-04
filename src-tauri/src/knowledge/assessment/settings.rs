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

fn validate_global_preferences(policy: &GenerationPolicy) -> Result<(), CommandError> {
    policy.validate()?;
    if !policy.generation_instructions.is_empty() {
        return Err(CommandError::new(
            "invalid_generation_policy",
            "Custom instructions belong to a document or collection generation request, not global question-format preferences.",
        ));
    }
    Ok(())
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
    validate_global_preferences(&policy)?;
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
    validate_global_preferences(&input.policy)?;
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

#[cfg(test)]
mod instruction_scope_tests {
    use super::super::policy::instructions::GenerationInstructions;
    use super::*;

    #[test]
    fn a_job_can_override_its_own_copy_but_cannot_store_the_prompt_globally() {
        let preferences = GenerationPolicy::default();
        let first = GenerationPolicy {
            generation_instructions: GenerationInstructions::parse(Some("Nursing assessments"))
                .unwrap(),
            ..preferences.clone()
        };
        let second = GenerationPolicy {
            generation_instructions: GenerationInstructions::parse(Some("Arterial anatomy"))
                .unwrap(),
            ..preferences.clone()
        };
        assert!(validate_global_preferences(&preferences).is_ok());
        assert!(preferences.generation_instructions.is_empty());
        assert_eq!(
            validate_global_preferences(&first).unwrap_err().code,
            "invalid_generation_policy"
        );
        assert_eq!(
            validate_global_preferences(&second).unwrap_err().code,
            "invalid_generation_policy"
        );
        assert_ne!(first.fingerprint().unwrap(), second.fingerprint().unwrap());
        assert!(first.validate().is_ok());
        assert!(second.validate().is_ok());
    }
}
