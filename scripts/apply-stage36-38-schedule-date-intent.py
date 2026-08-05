#!/usr/bin/env python3
"""Move manual rescheduling from frontend timestamps to backend local-date intent."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
path = ROOT / "src-tauri/src/schedule_controls.rs"
content = path.read_text()

content = content.replace(
    "    util::{generate_id, unix_time_millis, validate_entity_id},\n"
    "    workload::next_local_day_start_millis,\n",
    "    study_calendar::StudyTimeContext,\n"
    "    util::{generate_id, validate_entity_id},\n",
)
content = content.replace("    pub due_at: Option<i64>,\n", "    pub due_date: Option<String>,\n", 1)
content = content.replace("    due_at: Option<i64>,\n", "    due_date: Option<&'a str>,\n", 1)
content = content.replace(
    "    due_at: Option<i64>,\n    session_id: Option<String>,\n    request_fingerprint: String,\n",
    "    due_date: Option<String>,\n"
    "    due_at: Option<i64>,\n"
    "    session_id: Option<String>,\n"
    "    request_fingerprint: String,\n",
    1,
)
content = content.replace(
    "    due_at: Option<i64>,\n    result: ScheduleActionResultDto,\n",
    "    due_date: Option<String>,\n"
    "    due_at: Option<i64>,\n"
    "    result: ScheduleActionResultDto,\n",
    1,
)
content = content.replace(
    "    let input = validate_input(input)?;\n"
    "    let now = unix_time_millis()?;\n"
    "    let bury_until = if input.action == ScheduleActionKind::Bury {\n"
    "        Some(next_local_day_start_millis(now)?)\n",
    "    let context = StudyTimeContext::capture()?;\n"
    "    let input = validate_input(input, &context)?;\n"
    "    let now = context.now_utc_millis;\n"
    "    let bury_until = if input.action == ScheduleActionKind::Bury {\n"
    "        Some(context.tomorrow_start_utc_millis)\n",
    1,
)
content = content.replace(
    '            "INSERT INTO schedule_action_operations (\n'
    '                 id, request_fingerprint, session_id, action, due_at,\n'
    '                 requested_count, changed_count, unchanged_count, rejected_count,\n'
    '                 occurred_at, created_at\n'
    '             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",',
    '            "INSERT INTO schedule_action_operations (\n'
    '                 id, request_fingerprint, session_id, action, due_at,\n'
    '                 reschedule_intent_json,\n'
    '                 requested_count, changed_count, unchanged_count, rejected_count,\n'
    '                 occurred_at, created_at\n'
    '             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",',
    1,
)
content = content.replace(
    "                input.due_at,\n"
    "                result.requested_count,\n"
    "                result.changed_count,\n"
    "                result.unchanged_count,\n"
    "                result.rejected_count,\n"
    "                now,\n",
    "                input.due_at,\n"
    "                input.due_date.as_ref().map(|date|\n"
    "                    serde_json::json!({\"localDate\": date}).to_string()\n"
    "                ),\n"
    "                result.requested_count,\n"
    "                result.changed_count,\n"
    "                result.unchanged_count,\n"
    "                result.rejected_count,\n"
    "                now,\n",
    1,
)
content = content.replace(
    '            "SELECT request_fingerprint, session_id, due_at, action,\n'
    '                    requested_count, changed_count, unchanged_count, rejected_count\n',
    '            "SELECT request_fingerprint, session_id, due_at,\n'
    '                    reschedule_intent_json, action,\n'
    '                    requested_count, changed_count, unchanged_count, rejected_count\n',
    1,
)
content = content.replace(
    "                let action: String = row.get(3)?;\n"
    "                let changed_count: i64 = row.get(5)?;\n",
    "                let intent_json: Option<String> = row.get(3)?;\n"
    "                let due_date = intent_json.as_deref().and_then(|value| {\n"
    "                    serde_json::from_str::<serde_json::Value>(value).ok()?\n"
    "                        .get(\"localDate\")?.as_str().map(str::to_string)\n"
    "                });\n"
    "                let action: String = row.get(4)?;\n"
    "                let changed_count: i64 = row.get(6)?;\n",
    1,
)
content = content.replace(
    "                    session_id: row.get(1)?,\n"
    "                    due_at: row.get(2)?,\n"
    "                    result: ScheduleActionResultDto {\n"
    "                        action,\n"
    "                        requested_count: row.get(4)?,\n"
    "                        changed_count,\n"
    "                        unchanged_count: row.get(6)?,\n"
    "                        rejected_count: row.get(7)?,\n",
    "                    session_id: row.get(1)?,\n"
    "                    due_date,\n"
    "                    due_at: row.get(2)?,\n"
    "                    result: ScheduleActionResultDto {\n"
    "                        action,\n"
    "                        requested_count: row.get(5)?,\n"
    "                        changed_count,\n"
    "                        unchanged_count: row.get(7)?,\n"
    "                        rejected_count: row.get(8)?,\n",
    1,
)
content = content.replace(
    "                && existing.due_at == input.due_at\n"
    "                && card_ids == input.card_ids\n",
    "                && existing.due_date == input.due_date\n"
    "                && existing.due_at == input.due_at\n"
    "                && card_ids == input.card_ids\n",
    1,
)
start = content.index("fn validate_input(input: ScheduleActionInput)")
end = content.index("\n#[cfg(test)]\nmod tests", start)
replacement = r'''fn validate_input(
    input: ScheduleActionInput,
    context: &StudyTimeContext,
) -> Result<ValidatedScheduleAction, CommandError> {
    validate_entity_id(&input.mutation_id, "schedule mutation")?;
    if let Some(session_id) = &input.session_id {
        validate_entity_id(session_id, "practice session")?;
    }
    let action = ScheduleActionKind::parse(&input.action)?;
    let (due_date, due_at) = if action == ScheduleActionKind::Reschedule {
        let due_date = input.due_date.ok_or_else(|| {
            CommandError::new("missing_reschedule_date", "Choose a new due date.")
        })?;
        let parsed = context.parse_local_date(&due_date)?;
        let due_at = context.resolve_date_or_now(parsed)?;
        (Some(due_date), Some(due_at))
    } else {
        if input.due_date.is_some() {
            return Err(CommandError::new(
                "unexpected_reschedule_date",
                "A due date is only valid when rescheduling.",
            ));
        }
        (None, None)
    };

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
        due_date: due_date.as_deref(),
        session_id: input.session_id.as_deref(),
    };
    let json = serde_json::to_string(&identity).map_err(CommandError::serialization)?;

    Ok(ValidatedScheduleAction {
        mutation_id: input.mutation_id,
        card_ids,
        action,
        due_date,
        due_at,
        session_id: input.session_id,
        request_fingerprint: format!("schedule-action:v2:{json}"),
    })
}
'''
content = content[:start] + replacement + content[end:]
content = content.replace(
    "        let validated = validate_input(ScheduleActionInput {",
    "        let context = StudyTimeContext::capture().expect(\"context\");\n"
    "        let validated = validate_input(ScheduleActionInput {",
    1,
)
content = content.replace("            due_at: None,", "            due_date: None,", 1)
content = content.replace(
    "        })\n        .expect(\"a bounded batch should validate\");",
    "        }, &context)\n        .expect(\"a bounded batch should validate\");",
    1,
)
path.write_text(content)

# Update transactional scheduling tests to use canonical local-date intent.
tests = ROOT / "src-tauri/src/schedule_controls/extended_tests.rs"
content = tests.read_text().replace("due_at: None", "due_date: None")
old = '''                let current_due = load_schedule(connection, card_id)?.due_at;
                let mut same_due = action_input(&card_ids, 8, "reschedule");
                same_due.due_at = Some(current_due);
                let reschedule = apply_in_connection(connection, same_due)?;'''
new = '''                let context = StudyTimeContext::capture()?;
                let due_date = context.date_after_days(1)?;
                let target = context.resolve_date_start(due_date)?;
                connection
                    .execute(
                        "UPDATE card_schedules SET due_at = ?1 WHERE card_id = ?2",
                        params![target, card_id],
                    )
                    .map_err(CommandError::database)?;
                let mut same_due = action_input(&card_ids, 8, "reschedule");
                same_due.due_date = Some(due_date.format("%Y-%m-%d").to_string());
                let reschedule = apply_in_connection(connection, same_due)?;'''
if old not in content:
    raise SystemExit("reschedule test marker missing")
content = content.replace(old, new, 1)
content = content.replace(".due_at = Some(", ".due_date = Some(")
tests.write_text(content)

print("Applied backend local-date scheduling intent.")
