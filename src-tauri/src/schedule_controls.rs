//! Transactional manual scheduling actions for exact generated-card IDs.
//!
//! Date-only intentions are resolved once in Rust from a captured local-calendar
//! context. Idempotency fingerprints contain the canonical intent, while the
//! operation ledger stores both that intent and its originally resolved instant.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{
    params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension,
    TransactionBehavior,
};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    practice::refresh_session_counters,
    projections::scope::{chunks, push_value},
    scheduler::{new_schedule, update_schedule, ScheduleRecord, SCHEDULER_NAME, SCHEDULER_VERSION},
    study_calendar::{parse_local_date, StudyTimeContext},
    util::{generate_id, validate_entity_id},
};

const MAX_SCHEDULE_ACTION_CARDS: usize = 10_000;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RescheduleIntent {
    DueNow,
    Tomorrow,
    CalendarDays { days: i64 },
    CalendarDate { date: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleActionInput {
    pub mutation_id: String,
    pub card_ids: Vec<String>,
    pub action: String,
    #[serde(default)]
    pub reschedule_intent: Option<RescheduleIntent>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleActionResultDto {
    pub requested_count: i64,
    pub changed_count: i64,
    pub unchanged_count: i64,
    pub rejected_count: i64,
    pub action: String,
    pub resolved_due_at: Option<i64>,
}

impl Serialize for ScheduleActionResultDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ScheduleActionResultDto", 7)?;
        state.serialize_field("affectedCount", &self.changed_count)?;
        state.serialize_field("requestedCount", &self.requested_count)?;
        state.serialize_field("changedCount", &self.changed_count)?;
        state.serialize_field("unchangedCount", &self.unchanged_count)?;
        state.serialize_field("rejectedCount", &self.rejected_count)?;
        state.serialize_field("action", &self.action)?;
        state.serialize_field("resolvedDueAt", &self.resolved_due_at)?;
        state.end()
    }
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
    reschedule_intent: Option<&'a RescheduleIntent>,
    session_id: Option<&'a str>,
}

#[derive(Debug)]
struct ValidatedScheduleAction {
    mutation_id: String,
    card_ids: Vec<String>,
    action: ScheduleActionKind,
    reschedule_intent: Option<RescheduleIntent>,
    reschedule_intent_json: Option<String>,
    session_id: Option<String>,
    request_fingerprint: String,
}

struct ExistingScheduleOperation {
    request_fingerprint: Option<String>,
    session_id: Option<String>,
    due_at: Option<i64>,
    result: ScheduleActionResultDto,
}

struct StoredCardState {
    suspended_at: Option<i64>,
    archive_suspended_at: Option<i64>,
    buried_until: Option<i64>,
    schedule: ScheduleRecord,
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
        self.suspended_before != self.suspended_after || self.buried_before != self.buried_after
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
    let time = StudyTimeContext::capture()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;

    if let Some(existing) = existing_operation(&transaction, &input.mutation_id)? {
        ensure_equivalent_retry(&transaction, &existing, &input)?;
        transaction.commit().map_err(CommandError::database)?;
        return Ok(existing.result);
    }

    let resolved_due_at = resolve_due_at(input.reschedule_intent.as_ref(), &time)?;
    validate_session(&transaction, input.session_id.as_deref())?;
    let stored_cards = load_active_card_states(&transaction, &input.card_ids)?;
    let dismissals = load_session_dismissals(
        &transaction,
        input.session_id.as_deref(),
        &input.card_ids,
        input.action,
    )?;

    let bury_until =
        (input.action == ScheduleActionKind::Bury).then_some(time.tomorrow_start_utc_millis);
    let mut plans = Vec::with_capacity(input.card_ids.len());
    for card_id in &input.card_ids {
        let stored = stored_cards
            .get(card_id)
            .ok_or_else(schedule_card_not_found)?;
        if stored.archive_suspended_at.is_some() {
            return Err(CommandError::new(
                "deck_archived",
                "A selected card belongs to an archived deck. Restore the deck before changing its schedule.",
            ));
        }
        plans.push(plan_card_action(
            card_id,
            stored.schedule.clone(),
            stored.suspended_at,
            stored.buried_until,
            input.action,
            resolved_due_at,
            bury_until,
            time.now_utc_millis,
            dismissals.get(card_id).copied().unwrap_or(false),
        )?);
    }

    let requested_count = i64::try_from(plans.len()).unwrap_or(i64::MAX);
    let changed_count =
        i64::try_from(plans.iter().filter(|plan| plan.changed()).count()).unwrap_or(i64::MAX);
    let result = ScheduleActionResultDto {
        requested_count,
        changed_count,
        unchanged_count: requested_count.saturating_sub(changed_count),
        rejected_count: 0,
        action: input.action.as_str().to_string(),
        resolved_due_at,
    };
    insert_operation(&transaction, &input, &result, time.now_utc_millis)?;

    let mut refresh_session = false;
    for plan in plans.iter().filter(|plan| plan.changed()) {
        if plan.schedule_changed {
            apply_schedule_plan(&transaction, plan, time.now_utc_millis)?;
        }
        if plan.session_dismissal_needed {
            dismiss_session_card(&transaction, &input, plan, time.now_utc_millis)?;
            refresh_session = true;
        }
        insert_card_event(&transaction, &input, plan, time.now_utc_millis)?;
    }
    if refresh_session {
        if let Some(session_id) = input.session_id.as_deref() {
            refresh_session_counters(&transaction, session_id, time.now_utc_millis)?;
        }
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(result)
}

fn resolve_due_at(
    intent: Option<&RescheduleIntent>,
    time: &StudyTimeContext,
) -> Result<Option<i64>, CommandError> {
    let Some(intent) = intent else {
        return Ok(None);
    };
    let resolved = match intent {
        RescheduleIntent::DueNow => time.now_utc_millis,
        RescheduleIntent::Tomorrow => time.tomorrow_start_utc_millis,
        RescheduleIntent::CalendarDays { days } => {
            if !matches!(days, 3 | 7) {
                return Err(CommandError::new(
                    "invalid_reschedule_days",
                    "Choose three or seven local calendar days.",
                ));
            }
            time.start_after_local_days(*days)?
        }
        RescheduleIntent::CalendarDate { date } => {
            time.resolve_calendar_date(parse_local_date(date)?)?
        }
    };
    Ok(Some(resolved))
}

fn load_active_card_states(
    connection: &Connection,
    card_ids: &[String],
) -> Result<BTreeMap<String, StoredCardState>, CommandError> {
    let mut result = BTreeMap::new();
    for chunk in chunks(card_ids) {
        let mut values = Vec::<SqlValue>::new();
        let placeholders = chunk
            .iter()
            .map(|card_id| format!("?{}", push_value(&mut values, card_id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT
                 c.id, c.suspended_at, c.archive_suspended_at, c.buried_until,
                 s.state, s.due_at, s.stability, s.difficulty, s.elapsed_days,
                 s.scheduled_days, s.reps, s.lapses, s.last_review_at,
                 s.scheduler_name, s.scheduler_version, s.updated_at, s.revision
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             WHERE c.id IN ({placeholders})
               AND c.deleted_at IS NULL
               AND n.deleted_at IS NULL
               AND d.deleted_at IS NULL
               AND d.archived_at IS NULL"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                let card_id: String = row.get(0)?;
                Ok((
                    card_id.clone(),
                    StoredCardState {
                        suspended_at: row.get(1)?,
                        archive_suspended_at: row.get(2)?,
                        buried_until: row.get(3)?,
                        schedule: ScheduleRecord {
                            card_id,
                            state: row.get(4)?,
                            due_at: row.get(5)?,
                            stability: row.get(6)?,
                            difficulty: row.get(7)?,
                            elapsed_days: row.get(8)?,
                            scheduled_days: row.get(9)?,
                            reps: row.get(10)?,
                            lapses: row.get(11)?,
                            last_review_at: row.get(12)?,
                            scheduler_name: row.get(13)?,
                            scheduler_version: row.get(14)?,
                            updated_at: row.get(15)?,
                            revision: row.get(16)?,
                        },
                    },
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        result.extend(rows);
    }
    if result.len() != card_ids.len() {
        return Err(schedule_card_not_found());
    }
    Ok(result)
}

fn validate_session(connection: &Connection, session_id: Option<&str>) -> Result<(), CommandError> {
    let Some(session_id) = session_id else {
        return Ok(());
    };
    let completed_at: Option<Option<i64>> = connection
        .query_row(
            "SELECT completed_at FROM practice_sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    match completed_at {
        None => Err(CommandError::new(
            "practice_session_not_found",
            "That practice session no longer exists. No schedules were changed.",
        )),
        Some(Some(_)) => Err(CommandError::new(
            "practice_session_complete",
            "That practice session has already ended. No schedules were changed.",
        )),
        Some(None) => Ok(()),
    }
}

fn load_session_dismissals(
    connection: &Connection,
    session_id: Option<&str>,
    card_ids: &[String],
    action: ScheduleActionKind,
) -> Result<BTreeMap<String, bool>, CommandError> {
    let Some(session_id) = session_id else {
        return Ok(BTreeMap::new());
    };
    let mut result = BTreeMap::new();
    for chunk in chunks(card_ids) {
        let mut values = vec![SqlValue::Text(session_id.to_string())];
        let placeholders = chunk
            .iter()
            .map(|card_id| format!("?{}", push_value(&mut values, card_id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT card_id, rated_at, dismissed_at
             FROM practice_session_cards
             WHERE session_id = ?1 AND card_id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(params_from_iter(values.iter()), |row| {
                let card_id: String = row.get(0)?;
                let rated_at: Option<i64> = row.get(1)?;
                let dismissed_at: Option<i64> = row.get(2)?;
                Ok((
                    card_id,
                    action.dismisses_from_session() && rated_at.is_none() && dismissed_at.is_none(),
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        result.extend(rows);
    }
    if result.len() != card_ids.len() {
        return Err(CommandError::new(
            "schedule_session_card_mismatch",
            "At least one selected card is not part of that practice session. No schedules were changed.",
        ));
    }
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
    let changed = match action {
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

    if changed
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
        schedule_changed: changed,
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

fn insert_operation(
    connection: &Connection,
    input: &ValidatedScheduleAction,
    result: &ScheduleActionResultDto,
    now: i64,
) -> Result<(), CommandError> {
    connection
        .execute(
            "INSERT INTO schedule_action_operations (
                 id, request_fingerprint, session_id, action, due_at,
                 requested_count, changed_count, unchanged_count, rejected_count,
                 occurred_at, created_at, reschedule_intent_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, ?11)",
            params![
                input.mutation_id,
                input.request_fingerprint,
                input.session_id,
                input.action.as_str(),
                result.resolved_due_at,
                result.requested_count,
                result.changed_count,
                result.unchanged_count,
                result.rejected_count,
                now,
                input.reschedule_intent_json,
            ],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn apply_schedule_plan(
    connection: &Connection,
    plan: &CardActionPlan,
    now: i64,
) -> Result<(), CommandError> {
    if plan.control_changed() {
        let updated = connection
            .execute(
                "UPDATE cards
                 SET suspended_at = ?1, buried_until = ?2, updated_at = ?3
                 WHERE id = ?4 AND deleted_at IS NULL",
                params![plan.suspended_after, plan.buried_after, now, plan.card_id,],
            )
            .map_err(CommandError::database)?;
        if updated != 1 {
            return Err(schedule_card_not_found());
        }
    }
    update_schedule(connection, &plan.after)
}

fn dismiss_session_card(
    connection: &Connection,
    input: &ValidatedScheduleAction,
    plan: &CardActionPlan,
    now: i64,
) -> Result<(), CommandError> {
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
    let changed = connection
        .execute(
            "UPDATE practice_session_cards
             SET dismissed_at = ?1, disposition = ?2
             WHERE session_id = ?3 AND card_id = ?4
               AND rated_at IS NULL AND dismissed_at IS NULL",
            params![now, disposition, session_id, plan.card_id],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "schedule_session_card_changed",
            "A practice-session card changed while the schedule operation was running. No schedules were changed.",
        ));
    }
    Ok(())
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
                input.mutation_id,
                input.session_id,
                plan.card_id,
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
                let due_at: Option<i64> = row.get(2)?;
                let action: String = row.get(3)?;
                Ok(ExistingScheduleOperation {
                    request_fingerprint: row.get(0)?,
                    session_id: row.get(1)?,
                    due_at,
                    result: ScheduleActionResultDto {
                        requested_count: row.get(4)?,
                        changed_count: row.get(5)?,
                        unchanged_count: row.get(6)?,
                        rejected_count: row.get(7)?,
                        action,
                        resolved_due_at: due_at,
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
                && card_ids == input.card_ids
                && (input.action != ScheduleActionKind::Reschedule || existing.due_at.is_some())
        }
    };
    if equivalent {
        Ok(())
    } else {
        Err(CommandError::new(
            "idempotency_conflict",
            "That schedule retry identifier was already used for a different request. Refresh the cards before trying again.",
        ))
    }
}

fn validate_input(input: ScheduleActionInput) -> Result<ValidatedScheduleAction, CommandError> {
    validate_entity_id(&input.mutation_id, "schedule mutation")?;
    if let Some(session_id) = &input.session_id {
        validate_entity_id(session_id, "practice session")?;
    }
    let action = ScheduleActionKind::parse(&input.action)?;
    match (action, input.reschedule_intent.as_ref()) {
        (ScheduleActionKind::Reschedule, None) => {
            return Err(CommandError::new(
                "missing_reschedule_intent",
                "Choose when the selected cards should next be due.",
            ));
        }
        (ScheduleActionKind::Reschedule, Some(RescheduleIntent::CalendarDays { days }))
            if !matches!(days, 3 | 7) =>
        {
            return Err(CommandError::new(
                "invalid_reschedule_days",
                "Choose three or seven local calendar days.",
            ));
        }
        (ScheduleActionKind::Reschedule, Some(RescheduleIntent::CalendarDate { date })) => {
            parse_local_date(date)?;
        }
        (ScheduleActionKind::Reschedule, Some(_)) => {}
        (_, Some(_)) => {
            return Err(CommandError::new(
                "unexpected_reschedule_intent",
                "A due-date intention is only valid when rescheduling.",
            ));
        }
        (_, None) => {}
    }

    let mut unique_card_ids = BTreeSet::new();
    for card_id in input.card_ids {
        validate_entity_id(&card_id, "card")?;
        unique_card_ids.insert(card_id);
    }
    if unique_card_ids.is_empty() {
        return Err(CommandError::new(
            "invalid_schedule_selection",
            "Select at least one card to change its schedule.",
        ));
    }
    if unique_card_ids.len() > MAX_SCHEDULE_ACTION_CARDS {
        return Err(CommandError::new(
            "too_many_schedule_cards",
            format!("A schedule operation can contain at most {MAX_SCHEDULE_ACTION_CARDS} cards."),
        ));
    }
    let card_ids = unique_card_ids.into_iter().collect::<Vec<_>>();
    let identity = ScheduleActionRequestIdentity {
        card_ids: &card_ids,
        action: action.as_str(),
        reschedule_intent: input.reschedule_intent.as_ref(),
        session_id: input.session_id.as_deref(),
    };
    let request_json = serde_json::to_string(&identity).map_err(CommandError::serialization)?;
    let reschedule_intent_json = input
        .reschedule_intent
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(CommandError::serialization)?;

    Ok(ValidatedScheduleAction {
        mutation_id: input.mutation_id,
        card_ids,
        action,
        reschedule_intent: input.reschedule_intent,
        reschedule_intent_json,
        session_id: input.session_id,
        request_fingerprint: format!("schedule-action:v2:{request_json}"),
    })
}

fn schedule_card_not_found() -> CommandError {
    CommandError::new(
        "schedule_card_not_found",
        "At least one selected card no longer exists or belongs to an archived deck. No schedules were changed.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_id(value: u8) -> String {
        format!("{value:032x}")
    }

    #[test]
    fn fingerprint_uses_canonical_date_intent_not_resolved_timestamp() {
        let validated = validate_input(ScheduleActionInput {
            mutation_id: test_id(1),
            card_ids: vec![test_id(2)],
            action: "reschedule".to_string(),
            reschedule_intent: Some(RescheduleIntent::Tomorrow),
            session_id: None,
        })
        .expect("intent should validate");
        assert!(validated.request_fingerprint.contains("tomorrow"));
        assert!(!validated.request_fingerprint.contains("dueAt"));
    }

    #[test]
    fn rejects_arbitrary_day_counts() {
        let error = validate_input(ScheduleActionInput {
            mutation_id: test_id(1),
            card_ids: vec![test_id(2)],
            action: "reschedule".to_string(),
            reschedule_intent: Some(RescheduleIntent::CalendarDays { days: 4 }),
            session_id: None,
        })
        .expect_err("four days is not a supported shortcut");
        assert_eq!(error.code, "invalid_reschedule_days");
    }
}
