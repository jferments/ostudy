#!/usr/bin/env python3
"""Migrate regression tests to the scoped projection APIs."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")

def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")

regression = read("src-tauri/src/regression_tests.rs")
regression = regression.replace(
    """    error::CommandError,
    learning_signals::needs_more_practice,
    scheduler::insert_new_schedule,
    settings::load_settings,
    util::{generate_id, unix_time_millis},
    workload::{planned_scheduled_cards, workload_overview},""",
    """    error::CommandError,
    scheduler::insert_new_schedule,
    settings::load_settings,
    study_calendar::StudyTimeContext,
    studyability::learning_signal_cte,
    util::{generate_id, unix_time_millis},
    workload::{planned_scheduled_cards_scoped, workload_overview_scoped},""",
    1,
)
marker = "fn fts_count(connection: &Connection, query: &str) -> Result<i64, CommandError> {"
helper = r'''fn projected_needs_more_practice(
    connection: &Connection,
    card_id: &str,
) -> Result<bool, CommandError> {
    let sql = format!(
        "WITH {signals}
         SELECT COALESCE((
             SELECT needs_more_practice FROM learning_signals WHERE card_id = ?1
         ), 0)",
        signals = learning_signal_cte(),
    );
    connection
        .query_row(&sql, [card_id], |row| row.get::<_, i64>(0))
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

'''
if helper not in regression:
    regression = regression.replace(marker, helper + marker, 1)
regression = regression.replace(
    """            let overview =
                workload_overview(connection, std::slice::from_ref(&deck_id), &settings)?;""",
    """            let context = StudyTimeContext::capture()?;
            let overview = workload_overview_scoped(
                connection,
                std::slice::from_ref(&deck_id),
                &[],
                &settings,
                &context,
            )?;""",
    1,
)
regression = regression.replace(
    """            let (planned, _) =
                planned_scheduled_cards(connection, std::slice::from_ref(&deck_id), &settings)?;""",
    """            let (planned, _) = planned_scheduled_cards_scoped(
                connection,
                std::slice::from_ref(&deck_id),
                &[],
                &settings,
                &context,
            )?;""",
    1,
)
regression = regression.replace(
    """            let protected = workload_overview(connection, &[deck_id], &settings)?;""",
    """            let protected = workload_overview_scoped(
                connection,
                &[deck_id],
                &[],
                &settings,
                &context,
            )?;""",
    1,
)
regression = regression.replace("needs_more_practice(connection, &card_id)?", "projected_needs_more_practice(connection, &card_id)?")
write("src-tauri/src/regression_tests.rs", regression)

cards = read("src-tauri/src/cards.rs")
cards = cards.replace("use tauri::State;\n", "")
cards = cards.replace("    database::Database,\n", "")
if "#[cfg(test)]\nuse crate::database::Database;" not in cards:
    cards = cards.replace(
        "#[cfg(test)]\nuse crate::util::unix_time_millis;",
        "#[cfg(test)]\nuse crate::{database::Database, util::unix_time_millis};",
        1,
    )
old_first = """        let cards = database
            .with_connection(|connection| list_cards_from_connection(connection, &paths, &deck.id))
            .expect("cards should list");
        assert_eq!(cards.len(), 1);"""
new_first = """        let card_count: i64 = database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM cards c
                         JOIN notes n ON n.id = c.note_id
                         WHERE c.deck_id = ?1
                           AND c.deleted_at IS NULL
                           AND n.deleted_at IS NULL",
                        [deck.id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)
            })
            .expect("active card count should load");
        assert_eq!(card_count, 1);"""
if old_first not in cards:
    raise SystemExit("first card-list test anchor changed")
cards = cards.replace(old_first, new_first, 1)
old_second = """        let cards = database
            .with_connection(|connection| list_cards_from_connection(connection, &paths, &deck.id))
            .expect("cards should list");
        assert!(cards.is_empty());"""
new_second = """        let card_count: i64 = database
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM cards c
                         JOIN notes n ON n.id = c.note_id
                         WHERE c.deck_id = ?1
                           AND c.deleted_at IS NULL
                           AND n.deleted_at IS NULL",
                        [deck.id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)
            })
            .expect("active card count should load");
        assert_eq!(card_count, 0);"""
if old_second not in cards:
    raise SystemExit("second card-list test anchor changed")
cards = cards.replace(old_second, new_second, 1)
write("src-tauri/src/cards.rs", cards)

print("Migrated native regression tests to bounded projections.")
