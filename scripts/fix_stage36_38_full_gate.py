#!/usr/bin/env python3
"""Fix the final Stage 3.6–3.8 full-gate regressions.

Temporary development applicator; remove it and its workflow after validation.
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    if text.count(old) != 1:
        raise SystemExit(f"{path}: expected one replacement anchor, found {text.count(old)}")
    write(path, text.replace(old, new, 1))


# Review operation tests now prepare the same immutable presentation snapshots as
# production session creation. This keeps idempotency tests focused on replay
# behavior without bypassing the historical-attribution boundary.
replace_once(
    "src-tauri/src/practice/operation_tests.rs",
    """    decks::{create_deck_in_connection, CreateDeckInput},
    scheduler::{insert_new_schedule, load_schedule},""",
    """    decks::{create_deck_in_connection, CreateDeckInput},
    historical_attribution::{insert_session_snapshots, prepare_attributions},
    scheduler::{insert_new_schedule, load_schedule},""",
)
replace_once(
    "src-tauri/src/practice/operation_tests.rs",
    """    for (position, card_id) in card_ids.iter().enumerate() {
        connection
            .execute(
                \"INSERT INTO practice_session_cards (
                     session_id, card_id, position, presentation_variant
                 ) VALUES (?1, ?2, ?3, '')\",
                params![
                    session_id,
                    card_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                ],
            )
            .map_err(CommandError::database)?;
    }
    Ok(session_id)""",
    """    for (position, card_id) in card_ids.iter().enumerate() {
        connection
            .execute(
                \"INSERT INTO practice_session_cards (
                     session_id, card_id, position, presentation_variant
                 ) VALUES (?1, ?2, ?3, '')\",
                params![
                    session_id,
                    card_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                ],
            )
            .map_err(CommandError::database)?;
    }
    let requested = card_ids
        .iter()
        .cloned()
        .map(|card_id| (card_id, String::new()))
        .collect::<Vec<_>>();
    let attributions = prepare_attributions(connection, &requested)?;
    insert_session_snapshots(connection, &session_id, attributions.into_values(), 10)?;
    Ok(session_id)""",
)

# Strict request validation rejects impossible calendar intents before any clock
# capture, transaction, or idempotency fingerprint is produced.
replace_once(
    "src-tauri/src/schedule_controls.rs",
    """    if action == ScheduleActionKind::Reschedule {
        if input.reschedule.is_none() {
            return Err(CommandError::new(
                \"missing_reschedule_date\",
                \"Choose a new due date.\",
            ));
        }
    } else if input.reschedule.is_some() {""",
    """    if action == ScheduleActionKind::Reschedule {
        match input.reschedule.as_ref() {
            None => {
                return Err(CommandError::new(
                    \"missing_reschedule_date\",
                    \"Choose a new due date.\",
                ));
            }
            Some(RescheduleIntentInput::CalendarDays { days })
                if !(1..=3650).contains(days) =>
            {
                return Err(CommandError::new(
                    \"invalid_reschedule_date\",
                    \"Choose between 1 and 3650 local calendar days.\",
                ));
            }
            Some(RescheduleIntentInput::CalendarDate { date }) => {
                parse_local_date(date)?;
            }
            Some(
                RescheduleIntentInput::DueNow
                | RescheduleIntentInput::Tomorrow
                | RescheduleIntentInput::CalendarDays { .. },
            ) => {}
        }
    } else if input.reschedule.is_some() {""",
)

# Both bounded workload ranking CTEs expose an explicit column name consumed by
# their outer projection.
workload = read("src-tauri/src/workload.rs")
needle = "COALESCE(ls.needs_more_practice, 0),\n                    n.note_type"
if workload.count(needle) != 2:
    raise SystemExit(f"workload.rs: expected two ranking aliases, found {workload.count(needle)}")
workload = workload.replace(
    needle,
    "COALESCE(ls.needs_more_practice, 0) AS needs_more_practice,\n                    n.note_type",
)
write("src-tauri/src/workload.rs", workload)

# The handcrafted schema-12 regression fixture must advertise the same
# user_version that the real migration runner records. Schema-14 canonical
# databases legitimately contain the new attribution repair as ledger row 3.
replace_once(
    "src-tauri/src/stage34_35_regression_tests.rs",
    """    for migration in PRE_CANONICAL_MIGRATIONS {
        connection
            .execute_batch(migration)
            .expect(\"historical migration should apply\");
    }
    connection
}""",
    """    for migration in PRE_CANONICAL_MIGRATIONS {
        connection
            .execute_batch(migration)
            .expect(\"historical migration should apply\");
    }
    connection
        .execute_batch(\"PRAGMA user_version = 12;\")
        .expect(\"historical schema version should be recorded\");
    connection
}""",
)
replace_once(
    "src-tauri/src/stage34_35_regression_tests.rs",
    """            assert_eq!(repair_rows, 2);
            Ok(())
        })
        .expect(\"canonical startup should be read-only\");""",
    """            assert_eq!(repair_rows, 3);
            Ok(())
        })
        .expect(\"canonical startup should be read-only\");""",
)

print("Applied final Stage 3.6–3.8 full-gate corrections.")
