//! Transactional manual scheduling actions for exact generated-card IDs.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    practice::refresh_session_counters,
    scheduler::{
        load_schedule, new_schedule, update_schedule, ScheduleRecord, SCHEDULER_NAME,
        SCHEDULER_VERSION,
    },
    util::{generate_id, unix_time_millis, validate_entity_id},
    workload::next_local_day_start_millis,
};

const MAX_SCHEDULE_ACTION_CARDS: usize = 10_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleActionInput {
    pub mutation_id: String,
    pub card_ids: Vec<String>,
    pub action: String,
    pub due_at: Option<i64>,
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleActionResultDto {
    pub requested_count: i64,
    pub changed_count: i64,
    pub unchanged_count: i64,
    pub rejected_count: i64,
    pub action: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScheduleActionKind {
    Suspend,
    Resume,
    Bury,
    Unbury,
    Reset,
    Reschedule,
}

impl ScheduleActionKind {
    fn parse(value: &str) -> Result<Self, CommandError> {
        match value {
            "suspend" => Ok(Self::Suspend),
            "resume" => Ok(Self::Resume),
            "bury" => Ok(Self::Bury),
            "unbury" => Ok(Self::Unbury),
            "reset" => Ok(Self::Reset),
            "reschedule" => Ok(Self::Reschedule),
            _ => Err(CommandError::new(
                "invalid_schedule_action",
                "Choose a supported scheduling action.",
            )),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Suspend => "suspend",
            Self::Resume => "resume",
            Self::Bury => "bury",
            Self::Unbury => "unbury",
            Self::Reset => "reset",
            Self::Reschedule => "reschedule",
        }
    }

    const fn dismisses_from_session(self) -> bool {
        matches!(self, Self::Suspend | Self::Bury)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScheduleActionRequestIdentity<'a> {
    card_ids: &'a [String],
    action: &'a str,
    due_at: Option<i64>,
    session_id: Option<&'a str>,
}

struct ValidatedScheduleAction {
    mutation_id: String,
    card_ids: Vec<String>,
    action: ScheduleActionKind,
    due_at: Option<i64>,
    session_id: Option<String>,
    request_fingerprint: String,
}

struct ExistingScheduleOperation {
    request_fingerprint: Option<String>,
    session_id: Option<String>,
    due_at: Option<i64>,
    result: ScheduleActionResultDto,
}

struct CardActionPlan {
    card_id: String,
    before: ScheduleRecord,
    after: ScheduleRecord,
    suspended_before: Option<i64>,
    suspended_after: Option<i64>,
    buried_before: Option<i64>,
    buried_after: Option<i64>,
    schedule_changed: bool,
    session_dismissal_needed: bool,
}

impl CardActionPlan {
    fn changed(&self) -> bool {
        self.schedule_changed || self.session_dismissal_needed
    }

    fn control_changed(&self) -> bool {
        self.suspended_before != self.suspended_after
            || self.buried_before != self.buried_after
    }
}

#[tauri::command]
pub fn apply_schedule_action(
    database: State<'_, Database>,
    input: ScheduleActionInput,
) -> Result<ScheduleActionResultDto, CommandError> {
    database.with_connection(|connection| apply_in_connection(connection, input))
}

fn apply_in_connection(
    connection: &mut Connection,
    input: ScheduleActionInput,
) -> Result<ScheduleActionResultDto, CommandError> {
    let input = validate_input(input)?;
    let now = unix_time_millis()?;
    let bury_until = if input.action == ScheduleActionKind::Bury {
        Some(next_local_day_start_millis()?)
    } else {
        None
    };
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;

    if let Some(existing) = existing_operation(&transaction, &input.mutation_id)? {
        ensure_equivalent_retry(&transaction, &existing, &input)?;
        transaction.commit().map_err(CommandError::database)?;
        return Ok(existing.result);
    }

    validate_session_relationships(&transaction, &input)?;

    // Every selected card, schedule, and relevant session-card state is loaded
    // before the first write. Any invalid member therefore rejects the entire
    // batch and leaves all selected cards unchanged.
    let mut plans = Vec::with_capacity(input.card_ids.len());
    for card_id in &input.card_ids {
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
                "At least one selected card no longer exists. No schedules were changed.",
            ));
        };
        let before = load_schedule(&transaction, card_id)?;
        let session_dismissal_needed = session_dismissal_needed(
            &transaction,
            input.session_id.as_deref(),
            card_id,
            input.action,
        )?;
        plans.push(plan_card_action(
            card_id,
            before,
            suspended_before,
            buried_before,
            input.action,
            input.due_at,
            bury_until,
            now,
            session_dismissal_needed,
        )?);
    }

    let changed_count = i64::try_from(plans.iter().filter(|plan| plan.changed()).count())
        .unwrap_or(i64::MAX);
    let requested_count = i64::try_from(plans.len()).unwrap_or(i64::MAX);
    let result = ScheduleActionResultDto {
        requested_count,
        changed_count,
        unchanged_count: requested_count.saturating_sub(changed_count),
        rejected_count: 0,
        action: input.action.as_str().to_string(),
    };

    transaction
        .execute(
            "INSERT INTO schedule_action_operations (
                 id, request_fingerprint, session_id, action, due_at,
                 requested_count, changed_count, unchanged_count, rejected_count,
                 occurred_at, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                input.mutation_id.as_str(),
                input.request_fingerprint.as_str(),
                input.session_id.as_deref(),
                input.action.as_str(),
                input.due_at,
                result.requested_count,
                result.changed_count,
                result.unchanged_count,
                result.rejected_count,
                now,
            ],
        )
        .map_err(CommandError::database)?;

    let mut session_counters_may_have_changed = false;
    for plan in plans.iter().filter(|plan| plan.changed()) {
        if plan.schedule_changed {
            if plan.control_changed() {
                let updated = transaction
                    .execute(
                        "UPDATE cards
                         SET suspended_at = ?1, buried_until = ?2, updated_at = ?3
                         WHERE id = ?4 AND deleted_at IS NULL",
                        params![
                            plan.suspended_after,
                            plan.buried_after,
                            now,
                            plan.card_id.as_str(),
                        ],
                    )
                    .map_err(CommandError::database)?;
                if updated != 1 {
                    return Err(CommandError::new(
                        "schedule_card_not_found",
                        "A selected card changed while the schedule operation was running. No schedules were changed.",
                    ));
                }
            }
            update_schedule(&transaction, &plan.after)?;
        }

        if plan.session_dismissal_needed {
            let session_id = input.session_id.as_deref().ok_or_else(|| {
                CommandError::new(
                    "practice_session_not_found",
                    "The practice session context was lost. No schedules were changed.",
                )
            })?;
            let disposition = if input.action == ScheduleActionKind::Suspend {
                "suspended"
            } else {
                "buried"
            };
            let dismissed = transaction
                .execute(
                    "UPDATE practice_session_cards
                     SET dismissed_at = ?1, disposition = ?2
                     WHERE session_id = ?3 AND card_id = ?4
                       AND rated_at IS NULL AND dismissed_at IS NULL",
                    params![now, disposition, session_id, plan.card_id.as_str()],
                )
                .map_err(CommandError::database)?;
            if dismissed != 1 {
                return Err(CommandError::new(
                    "schedule_session_card_changed",
                    "A practice-session card changed while the schedule operation was running. No schedules were changed.",
                ));
            }
            session_counters_may_have_changed = true;
        }

        insert_card_event(&transaction, &input, plan, now)?;
    }

    if session_counters_may_have_changed {
        if let Some(session_id) = input.session_id.as_deref() {
            refresh_session_counters(&transaction, session_id, now)?;
        }
    }

    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn plan_card_action(
    card_id: &str,
    before: ScheduleRecord,
    suspended_before: Option<i64>,
    buried_before: Option<i64>,
    action: ScheduleActionKind,
    due_at: Option<i64>,
    bury_until: Option<i64>,
    now: i64,
    session_dismissal_needed: bool,
) -> Result<CardActionPlan, CommandError> {
    let mut after = before.clone();
    let mut suspended_after = suspended_before;
    let mut buried_after = buried_before;
    let schedule_changed = match action {
        ScheduleActionKind::Suspend => {
            if suspended_before.is_none() {
                suspended_after = Some(now);
                true
            } else {
                false
            }
        }
        ScheduleActionKind::Resume => {
            if suspended_before.is_some() {
                suspended_after = None;
                true
            } else {
                false
            }
        }
        ScheduleActionKind::Bury => {
            let target = bury_until.ok_or_else(|| {
                CommandError::new(
                    "invalid_bury_deadline",
                    "The next local study day could not be calculated.",
                )
            })?;
            if buried_before.is_some_and(|existing| existing >= target) {
                false
            } else {
                buried_after = Some(target);
                true
            }
        }
        ScheduleActionKind::Unbury => {
            if buried_before.is_some() {
                buried_after = None;
                true
            } else {
                false
            }
        }
        ScheduleActionKind::Reset => {
            if is_canonical_new_schedule(&before, now) {
                false
            } else {
                after = new_schedule(card_id.to_string(), now);
                after.revision = before.revision + 1;
                true
            }
        }
        ScheduleActionKind::Reschedule => {
            let target = due_at.ok_or_else(|| {
                CommandError::new("missing_reschedule_date", "Choose a new due date.")
            })?;
            if before.due_at == target {
                false
            } else {
                after.due_at = target;
                after.updated_at = now;
                after.revision = before.revision + 1;
                true
            }
        }
    };

    if schedule_changed
        && matches!(
            action,
            ScheduleActionKind::Suspend
                | ScheduleActionKind::Resume
                | ScheduleActionKind::Bury
                | ScheduleActionKind::Unbury
        )
    {
        after.updated_at = now;
        after.revision = before.revision + 1;
    }

    Ok(CardActionPlan {
        card_id: card_id.to_string(),
        before,
        after,
        suspended_before,
        suspended_after,
        buried_before,
        buried_after,
        schedule_changed,
        session_dismissal_needed,
    })
}

fn is_canonical_new_schedule(schedule: &ScheduleRecord, now: i64) -> bool {
    schedule.state == 0
        && schedule.due_at <= now
        && schedule.stability == 0.0
        && schedule.difficulty == 0.0
        && schedule.elapsed_days == 0
        && schedule.scheduled_days == 0
        && schedule.reps == 0
        && schedule.lapses == 0
        && schedule.scheduler_name == SCHEDULER_NAME
        && schedule.scheduler_version == SCHEDULER_VERSION
}

fn session_dismissal_needed(
    connection: &Connection,
    session_id: Option<&str>,
    card_id: &str,
    action: ScheduleActionKind,
) -> Result<bool, CommandError> {
    let Some(session_id) = session_id else {
        return Ok(false);
    };
    if !action.dismisses_from_session() {
        return Ok(false);
    }
    let state: (Option<i64>, Option<i64>) = connection
        .query_row(
            "SELECT rated_at, dismissed_at
             FROM practice_session_cards
             WHERE session_id = ?1 AND card_id = ?2",
            params![session_id, card_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    Ok(state.0.is_none() && state.1.is_none())
}

fn insert_card_event(
    connection: &Connection,
    input: &ValidatedScheduleAction,
    plan: &CardActionPlan,
    now: i64,
) -> Result<(), CommandError> {
    let event_id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO schedule_action_events (
                 id, operation_id, session_id, card_id, action,
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
                input.mutation_id.as_str(),
                input.session_id.as_deref(),
                plan.card_id.as_str(),
                input.action.as_str(),
                plan.before.due_at,
                plan.after.due_at,
                plan.before.state,
                plan.after.state,
                plan.suspended_before,
                plan.suspended_after,
                plan.buried_before,
                plan.buried_after,
                plan.before.revision,
                plan.after.revision,
                now,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn validate_session_relationships(
    connection: &Connection,
    input: &ValidatedScheduleAction,
) -> Result<(), CommandError> {
    let Some(session_id) = input.session_id.as_deref() else {
        return Ok(());
    };
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM practice_sessions WHERE id = ?1)",
            [session_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists == 0 {
        return Err(CommandError::new(
            "practice_session_not_found",
            "That practice session no longer exists. No schedules were changed.",
        ));
    }
    for card_id in &input.card_ids {
        let belongs: i64 = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM practice_session_cards
                     WHERE session_id = ?1 AND card_id = ?2
                 )",
                params![session_id, card_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if belongs == 0 {
            return Err(CommandError::new(
                "schedule_session_card_mismatch",
                "At least one selected card is not part of that practice session. No schedules were changed.",
            ));
        }
    }
    Ok(())
}

fn existing_operation(
    connection: &Connection,
    mutation_id: &str,
) -> Result<Option<ExistingScheduleOperation>, CommandError> {
    connection
        .query_row(
            "SELECT request_fingerprint, session_id, due_at, action,
                    requested_count, changed_count, unchanged_count, rejected_count
             FROM schedule_action_operations WHERE id = ?1",
            [mutation_id],
            |row| {
                let action: String = row.get(3)?;
                Ok(ExistingScheduleOperation {
                    request_fingerprint: row.get(0)?,
                    session_id: row.get(1)?,
                    due_at: row.get(2)?,
                    result: ScheduleActionResultDto {
                        action,
                        requested_count: row.get(4)?,
                        changed_count: row.get(5)?,
                        unchanged_count: row.get(6)?,
                        rejected_count: row.get(7)?,
                    },
                })
            },
        )
        .optional()
        .map_err(CommandError::database)
}

fn ensure_equivalent_retry(
    connection: &Connection,
    existing: &ExistingScheduleOperation,
    input: &ValidatedScheduleAction,
) -> Result<(), CommandError> {
    let equivalent = match existing.request_fingerprint.as_deref() {
        Some(stored) => stored == input.request_fingerprint,
        None => {
            // Schema-8 one-card actions were migrated without a complete request
            // fingerprint. Compare every persisted request field and the exact
            // child-card set when such an operation is replayed.
            let mut statement = connection
                .prepare(
                    "SELECT card_id FROM schedule_action_events
                     WHERE operation_id = ?1 ORDER BY card_id ASC",
                )
                .map_err(CommandError::database)?;
            let card_ids = statement
                .query_map([input.mutation_id.as_str()], |row| row.get::<_, String>(0))
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            existing.result.action == input.action.as_str()
                && existing.session_id == input.session_id
                && existing.due_at == input.due_at
                && card_ids == input.card_ids
        }
    };
    if equivalent {
        return Ok(());
    }
    Err(CommandError::new(
        "idempotency_conflict",
        "That schedule retry identifier was already used for a different request. Refresh the cards before trying again.",
    ))
}

fn validate_input(input: ScheduleActionInput) -> Result<ValidatedScheduleAction, CommandError> {
    validate_entity_id(&input.mutation_id, "schedule mutation")?;
    if let Some(session_id) = &input.session_id {
        validate_entity_id(session_id, "practice session")?;
    }
    let action = ScheduleActionKind::parse(&input.action)?;
    if action == ScheduleActionKind::Reschedule {
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

    let mut unique = BTreeSet::new();
    for card_id in input.card_ids {
        validate_entity_id(&card_id, "card")?;
        unique.insert(card_id);
    }
    if unique.is_empty() {
        return Err(CommandError::new(
            "invalid_schedule_selection",
            "Select at least one card to change its schedule.",
        ));
    }
    if unique.len() > MAX_SCHEDULE_ACTION_CARDS {
        return Err(CommandError::new(
            "too_many_schedule_cards",
            format!(
                "A schedule operation can contain at most {MAX_SCHEDULE_ACTION_CARDS} cards."
            ),
        ));
    }
    let card_ids = unique.into_iter().collect::<Vec<_>>();
    let identity = ScheduleActionRequestIdentity {
        card_ids: &card_ids,
        action: action.as_str(),
        due_at: input.due_at,
        session_id: input.session_id.as_deref(),
    };
    let serialized = serde_json::to_string(&identity).map_err(CommandError::serialization)?;

    Ok(ValidatedScheduleAction {
        mutation_id: input.mutation_id,
        card_ids,
        action,
        due_at: input.due_at,
        session_id: input.session_id,
        request_fingerprint: format!("schedule-action:v1:{serialized}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_deduplicates_bounded_card_ids() {
        let first = "b".repeat(32);
        let second = "c".repeat(32);
        let validated = validate_input(ScheduleActionInput {
            mutation_id: "a".repeat(32),
            card_ids: vec![second.clone(), first.clone(), second],
            action: "suspend".to_string(),
            due_at: None,
            session_id: None,
        })
        .expect("a bounded batch should validate");
        assert_eq!(validated.card_ids, vec![first, "c".repeat(32)]);
    }
}

#[cfg(test)]
include!("schedule_controls/extended_tests.rs");
