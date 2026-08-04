//! Transactional manual scheduling actions for exact generated-card IDs.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    practice::refresh_session_counters,
    scheduler::{load_schedule, new_schedule, update_schedule},
    util::{generate_id, unix_time_millis, validate_entity_id},
    workload::next_local_day_start_millis,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleActionInput {
    pub mutation_id: String,
    pub card_ids: Vec<String>,
    pub action: String,
    pub due_at: Option<i64>,
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleActionResultDto {
    pub affected_count: i64,
    pub action: String,
}

#[tauri::command]
pub fn apply_schedule_action(
    database: State<'_, Database>,
    input: ScheduleActionInput,
) -> Result<ScheduleActionResultDto, CommandError> {
    validate_input(&input)?;
    database.with_connection(|connection| apply_in_connection(connection, input))
}

fn apply_in_connection(
    connection: &mut Connection,
    input: ScheduleActionInput,
) -> Result<ScheduleActionResultDto, CommandError> {
    if let Some(existing) = existing_result(connection, &input.mutation_id)? {
        return Ok(existing);
    }
    let card_id = input.card_ids.first().ok_or_else(|| {
        CommandError::new("invalid_schedule_selection", "Select exactly one card.")
    })?;
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let control_state: Option<(Option<i64>, Option<i64>)> = transaction
        .query_row(
            "SELECT suspended_at, buried_until FROM cards
             WHERE id = ?1 AND deleted_at IS NULL",
            [card_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((suspended_before, buried_before)) = control_state else {
        return Err(CommandError::new(
            "schedule_card_not_found",
            "The selected card no longer exists.",
        ));
    };
    let before = load_schedule(&transaction, card_id)?;
    let mut after = before.clone();
    let (suspended_after, buried_after) = match input.action.as_str() {
        "suspend" => (Some(now), buried_before),
        "resume" => (None, buried_before),
        "bury" => (suspended_before, Some(next_local_day_start_millis()?)),
        "unbury" => (suspended_before, None),
        "reset" => {
            after = new_schedule(card_id.clone(), now);
            after.revision = before.revision + 1;
            (suspended_before, buried_before)
        }
        "reschedule" => {
            after.due_at = input.due_at.ok_or_else(|| {
                CommandError::new("missing_reschedule_date", "Choose a new due date.")
            })?;
            after.updated_at = now;
            after.revision = before.revision + 1;
            (suspended_before, buried_before)
        }
        _ => unreachable!("validated action"),
    };

    if matches!(
        input.action.as_str(),
        "suspend" | "resume" | "bury" | "unbury"
    ) {
        transaction
            .execute(
                "UPDATE cards SET suspended_at = ?1, buried_until = ?2, updated_at = ?3
                 WHERE id = ?4 AND deleted_at IS NULL",
                params![suspended_after, buried_after, now, card_id],
            )
            .map_err(CommandError::database)?;
        after.updated_at = now;
        after.revision = before.revision + 1;
    }
    update_schedule(&transaction, &after)?;

    let event_id = generate_id(&transaction)?;
    transaction
        .execute(
            "INSERT INTO schedule_action_events (
                 id, mutation_id, session_id, card_id, action,
                 due_before, due_after, state_before, state_after,
                 suspended_before, suspended_after,
                 buried_until_before, buried_until_after,
                 schedule_revision_before, schedule_revision_after,
                 occurred_at, created_at
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                 ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16
             )",
            params![
                event_id,
                input.mutation_id,
                input.session_id,
                card_id,
                input.action,
                before.due_at,
                after.due_at,
                before.state,
                after.state,
                suspended_before,
                suspended_after,
                buried_before,
                buried_after,
                before.revision,
                after.revision,
                now,
            ],
        )
        .map_err(CommandError::database)?;

    if let Some(session_id) = &input.session_id {
        if matches!(input.action.as_str(), "suspend" | "bury") {
            transaction
                .execute(
                    "UPDATE practice_session_cards
                     SET dismissed_at = COALESCE(dismissed_at, ?1), disposition = ?2
                     WHERE session_id = ?3 AND card_id = ?4 AND rated_at IS NULL",
                    params![
                        now,
                        if input.action == "suspend" {
                            "suspended"
                        } else {
                            "buried"
                        },
                        session_id,
                        card_id,
                    ],
                )
                .map_err(CommandError::database)?;
            refresh_session_counters(&transaction, session_id, now)?;
        }
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(ScheduleActionResultDto {
        affected_count: 1,
        action: input.action,
    })
}

fn existing_result(
    connection: &Connection,
    mutation_id: &str,
) -> Result<Option<ScheduleActionResultDto>, CommandError> {
    connection
        .query_row(
            "SELECT action FROM schedule_action_events WHERE mutation_id = ?1",
            [mutation_id],
            |row| {
                Ok(ScheduleActionResultDto {
                    action: row.get(0)?,
                    affected_count: 1,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

fn validate_input(input: &ScheduleActionInput) -> Result<(), CommandError> {
    validate_entity_id(&input.mutation_id, "schedule mutation")?;
    if let Some(session_id) = &input.session_id {
        validate_entity_id(session_id, "practice session")?;
    }
    if input.card_ids.len() != 1 {
        return Err(CommandError::new(
            "invalid_schedule_selection",
            "Schedule mutations operate on exactly one card so every retry remains independently idempotent.",
        ));
    }
    validate_entity_id(&input.card_ids[0], "card")?;
    if !matches!(
        input.action.as_str(),
        "suspend" | "resume" | "bury" | "unbury" | "reset" | "reschedule"
    ) {
        return Err(CommandError::new(
            "invalid_schedule_action",
            "Choose a supported scheduling action.",
        ));
    }
    if input.action == "reschedule" {
        let due_at = input.due_at.ok_or_else(|| {
            CommandError::new("missing_reschedule_date", "Choose a new due date.")
        })?;
        if due_at < 0 {
            return Err(CommandError::new(
                "invalid_reschedule_date",
                "The new due date is invalid.",
            ));
        }
    } else if input.due_at.is_some() {
        return Err(CommandError::new(
            "unexpected_reschedule_date",
            "A due date is only valid when rescheduling.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_multi_card_mutations() {
        let input = ScheduleActionInput {
            mutation_id: "a".repeat(32),
            card_ids: vec!["b".repeat(32), "c".repeat(32)],
            action: "suspend".to_string(),
            due_at: None,
            session_id: None,
        };
        assert_eq!(
            validate_input(&input)
                .expect_err("selection should fail")
                .code,
            "invalid_schedule_selection"
        );
    }
}

#[cfg(test)]
include!("schedule_controls/extended_tests.rs");
