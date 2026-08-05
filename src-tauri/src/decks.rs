use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    practice::refresh_session_counters,
    study_calendar::StudyTimeContext,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_DECK_NAME_CHARS: usize = 200;
const MAX_DECK_DESCRIPTION_CHARS: usize = 2_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateDeckInput {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateDeckInput {
    pub deck_id: String,
    pub name: String,
    pub description: Option<String>,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_due_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
    pub revision: i64,
}

#[tauri::command]
pub fn list_decks(database: State<'_, Database>) -> Result<Vec<DeckDto>, CommandError> {
    database.with_connection(|connection| list_decks_from_connection(connection, false))
}

#[tauri::command]
pub fn list_archived_decks(database: State<'_, Database>) -> Result<Vec<DeckDto>, CommandError> {
    database.with_connection(|connection| list_decks_from_connection(connection, true))
}

#[tauri::command]
pub fn get_deck(database: State<'_, Database>, deck_id: String) -> Result<DeckDto, CommandError> {
    validate_entity_id(&deck_id, "deck")?;
    database.with_connection(|connection| get_deck_from_connection(connection, &deck_id))
}

#[tauri::command]
pub fn create_deck(
    database: State<'_, Database>,
    input: CreateDeckInput,
) -> Result<DeckDto, CommandError> {
    database.with_connection(|connection| create_deck_in_connection(connection, input))
}

#[tauri::command]
pub fn update_deck(
    database: State<'_, Database>,
    input: UpdateDeckInput,
) -> Result<DeckDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    database.with_connection(|connection| update_deck_in_connection(connection, input))
}

#[tauri::command]
pub fn archive_deck(
    database: State<'_, Database>,
    deck_id: String,
) -> Result<DeckDto, CommandError> {
    validate_entity_id(&deck_id, "deck")?;
    database.with_connection(|connection| archive_deck_in_connection(connection, &deck_id))
}

#[tauri::command]
pub fn unarchive_deck(
    database: State<'_, Database>,
    deck_id: String,
) -> Result<DeckDto, CommandError> {
    validate_entity_id(&deck_id, "deck")?;
    database.with_connection(|connection| unarchive_deck_in_connection(connection, &deck_id))
}

pub(crate) fn list_decks_from_connection(
    connection: &mut Connection,
    archived: bool,
) -> Result<Vec<DeckDto>, CommandError> {
    let context = StudyTimeContext::capture()?;
    let archive_clause = if archived {
        "d.archived_at IS NOT NULL"
    } else {
        "d.archived_at IS NULL"
    };
    let sql = deck_projection_sql(&format!("d.deleted_at IS NULL AND {archive_clause}"));
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map([context.now_utc_millis], deck_from_row)
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub(crate) fn get_deck_from_connection(
    connection: &Connection,
    deck_id: &str,
) -> Result<DeckDto, CommandError> {
    let context = StudyTimeContext::capture()?;
    let sql = deck_projection_sql("d.id = ?2 AND d.deleted_at IS NULL");
    connection
        .query_row(
            &sql,
            params![context.now_utc_millis, deck_id],
            deck_from_row,
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("deck_not_found", "That deck no longer exists."))
}

pub(crate) fn create_deck_in_connection(
    connection: &mut Connection,
    input: CreateDeckInput,
) -> Result<DeckDto, CommandError> {
    let name = validate_name(input.name)?;
    let description = validate_description(input.description.unwrap_or_default())?;
    let now = unix_time_millis()?;

    let transaction = connection.transaction().map_err(CommandError::database)?;
    let id = generate_id(&transaction)?;
    let sort_order: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1
             FROM decks
             WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    transaction
        .execute(
            "INSERT INTO decks (
                 id, name, description, sort_order, created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)",
            params![id, name, description, sort_order, now],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;

    get_deck_from_connection(connection, &id)
}

fn update_deck_in_connection(
    connection: &mut Connection,
    input: UpdateDeckInput,
) -> Result<DeckDto, CommandError> {
    let name = validate_name(input.name)?;
    let description = validate_description(input.description.unwrap_or_default())?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The deck revision is invalid.",
        ));
    }
    let now = unix_time_millis()?;

    let changed = connection
        .execute(
            "UPDATE decks
             SET name = ?1, description = ?2, updated_at = ?3, revision = revision + 1
             WHERE id = ?4
               AND revision = ?5
               AND deleted_at IS NULL",
            params![name, description, now, input.deck_id, input.revision],
        )
        .map_err(CommandError::database)?;

    if changed == 0 {
        return Err(CommandError::new(
            "deck_changed",
            "This deck changed in another window. Reload it and try again.",
        ));
    }

    get_deck_from_connection(connection, &input.deck_id)
}

fn archive_deck_in_connection(
    connection: &mut Connection,
    deck_id: &str,
) -> Result<DeckDto, CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let changed = transaction
        .execute(
            "UPDATE decks
             SET archived_at = ?1, updated_at = ?1, revision = revision + 1
             WHERE id = ?2 AND deleted_at IS NULL AND archived_at IS NULL",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "deck_not_found",
            "That deck is already archived or no longer exists.",
        ));
    }

    transaction
        .execute(
            "INSERT INTO deck_archive_cards (deck_id, card_id, archived_at)
             SELECT ?1, c.id, ?2
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             WHERE c.deck_id = ?1
               AND c.deleted_at IS NULL
               AND n.deleted_at IS NULL
               AND c.suspended_at IS NULL",
            params![deck_id, now],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "UPDATE cards
             SET suspended_at = ?1, updated_at = ?1
             WHERE id IN (
                 SELECT card_id FROM deck_archive_cards
                 WHERE deck_id = ?2 AND archived_at = ?1
             )",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "UPDATE card_schedules
             SET updated_at = ?1, revision = revision + 1
             WHERE card_id IN (
                 SELECT card_id FROM deck_archive_cards
                 WHERE deck_id = ?2 AND archived_at = ?1
             )",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;
    let archive_session_ids = {
        let mut statement = transaction
            .prepare(
                "SELECT DISTINCT psc.session_id
                 FROM practice_session_cards psc
                 JOIN cards c ON c.id = psc.card_id
                 JOIN practice_sessions ps ON ps.id = psc.session_id
                 WHERE c.deck_id = ?1 AND ps.completed_at IS NULL
                   AND psc.rated_at IS NULL AND psc.dismissed_at IS NULL",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([deck_id], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        rows
    };
    transaction
        .execute(
            "UPDATE practice_session_cards
             SET dismissed_at = ?1, disposition = 'suspended'
             WHERE rated_at IS NULL AND dismissed_at IS NULL
               AND card_id IN (SELECT id FROM cards WHERE deck_id = ?2)",
            params![now, deck_id],
        )
        .map_err(CommandError::database)?;
    for session_id in archive_session_ids {
        refresh_session_counters(&transaction, &session_id, now)?;
    }
    transaction.commit().map_err(CommandError::database)?;
    get_deck_from_connection(connection, deck_id)
}

fn unarchive_deck_in_connection(
    connection: &mut Connection,
    deck_id: &str,
) -> Result<DeckDto, CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let archived_at: Option<i64> = transaction
        .query_row(
            "SELECT archived_at FROM decks
             WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NOT NULL",
            [deck_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .flatten();
    let Some(archived_at) = archived_at else {
        return Err(CommandError::new(
            "deck_not_found",
            "That deck is not archived or no longer exists.",
        ));
    };

    transaction
        .execute(
            "UPDATE decks
             SET archived_at = NULL, updated_at = ?1, revision = revision + 1
             WHERE id = ?2 AND deleted_at IS NULL AND archived_at = ?3",
            params![now, deck_id, archived_at],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "UPDATE cards
             SET suspended_at = NULL, updated_at = ?1
             WHERE id IN (
                 SELECT card_id FROM deck_archive_cards
                 WHERE deck_id = ?2 AND archived_at = ?3
             )
               AND deleted_at IS NULL
               AND suspended_at = ?3",
            params![now, deck_id, archived_at],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "UPDATE card_schedules
             SET updated_at = ?1, revision = revision + 1
             WHERE card_id IN (
                 SELECT c.id
                 FROM cards c
                 JOIN deck_archive_cards dac ON dac.card_id = c.id
                 WHERE dac.deck_id = ?2 AND dac.archived_at = ?3
                   AND c.deleted_at IS NULL AND c.suspended_at IS NULL
             )",
            params![now, deck_id, archived_at],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "DELETE FROM deck_archive_cards WHERE deck_id = ?1",
            [deck_id],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;
    get_deck_from_connection(connection, deck_id)
}

fn deck_projection_sql(where_sql: &str) -> String {
    format!(
        "WITH note_counts AS (
             SELECT n.deck_id, COUNT(*) AS source_note_count
             FROM notes n
             WHERE n.deleted_at IS NULL
             GROUP BY n.deck_id
         ),
         generated_counts AS (
             SELECT c.deck_id, COUNT(*) AS generated_card_count
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL
             GROUP BY c.deck_id
         ),
         due_counts AS (
             SELECT c.deck_id, COUNT(*) AS studyable_due_count
             FROM cards c
             JOIN notes n ON n.id = c.note_id
             JOIN decks eligible_deck ON eligible_deck.id = c.deck_id
             JOIN card_schedules s ON s.card_id = c.id
             WHERE c.deleted_at IS NULL
               AND n.deleted_at IS NULL
               AND eligible_deck.deleted_at IS NULL
               AND eligible_deck.archived_at IS NULL
               AND c.suspended_at IS NULL
               AND (c.buried_until IS NULL OR c.buried_until <= ?1)
               AND s.due_at <= ?1
             GROUP BY c.deck_id
         )
         SELECT d.id, d.name, d.description,
                COALESCE(nc.source_note_count, 0),
                COALESCE(gc.generated_card_count, 0),
                CASE WHEN d.archived_at IS NULL
                     THEN COALESCE(dc.studyable_due_count, 0)
                     ELSE 0 END,
                d.created_at, d.updated_at, d.archived_at, d.revision
         FROM decks d
         LEFT JOIN note_counts nc ON nc.deck_id = d.id
         LEFT JOIN generated_counts gc ON gc.deck_id = d.id
         LEFT JOIN due_counts dc ON dc.deck_id = d.id
         WHERE {where_sql}
         ORDER BY d.sort_order ASC, d.name COLLATE NOCASE ASC, d.created_at ASC"
    )
}

pub(crate) fn validate_name(value: String) -> Result<String, CommandError> {
    let trimmed = value.trim();
    let character_count = trimmed.chars().count();

    if character_count == 0 {
        return Err(CommandError::new(
            "invalid_deck_name",
            "Enter a name for the deck.",
        ));
    }
    if character_count > MAX_DECK_NAME_CHARS {
        return Err(CommandError::new(
            "invalid_deck_name",
            format!("Deck names cannot exceed {MAX_DECK_NAME_CHARS} characters."),
        ));
    }

    Ok(trimmed.to_string())
}

pub(crate) fn validate_description(value: String) -> Result<String, CommandError> {
    let trimmed = value.trim();

    if trimmed.chars().count() > MAX_DECK_DESCRIPTION_CHARS {
        return Err(CommandError::new(
            "invalid_deck_description",
            format!("Deck descriptions cannot exceed {MAX_DECK_DESCRIPTION_CHARS} characters."),
        ));
    }

    Ok(trimmed.to_string())
}

fn deck_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeckDto> {
    Ok(DeckDto {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        source_note_count: row.get(3)?,
        generated_card_count: row.get(4)?,
        studyable_due_count: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        archived_at: row.get(8)?,
        revision: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_updates_and_lists_persistent_deck_records() {
        let database = Database::open_in_memory().expect("database should open");

        let created = database
            .with_connection(|connection| {
                create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "  Pharmacology  ".to_string(),
                        description: Some("  NCLEX medications  ".to_string()),
                    },
                )
            })
            .expect("deck should be created");

        assert_eq!(created.name, "Pharmacology");
        assert_eq!(created.description, "NCLEX medications");
        assert_eq!(created.id.len(), 32);
        assert_eq!(created.source_note_count, 0);
        assert_eq!(created.generated_card_count, 0);

        let updated = database
            .with_connection(|connection| {
                update_deck_in_connection(
                    connection,
                    UpdateDeckInput {
                        deck_id: created.id.clone(),
                        name: "Advanced Pharmacology".to_string(),
                        description: Some("Updated".to_string()),
                        revision: created.revision,
                    },
                )
            })
            .expect("deck should update");
        assert_eq!(updated.name, "Advanced Pharmacology");
        assert_eq!(updated.revision, 2);

        let decks = database
            .with_connection(|connection| list_decks_from_connection(connection, false))
            .expect("decks should load");
        assert_eq!(decks.len(), 1);
        assert_eq!(decks[0].id, created.id);
    }

    #[test]
    fn archive_round_trip_preserves_individual_suspensions() {
        let database = Database::open_in_memory().expect("database should open");
        let deck = database
            .with_connection(|connection| {
                create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Archive".to_string(),
                        description: None,
                    },
                )
            })
            .expect("deck");
        let archived = database
            .with_connection(|connection| archive_deck_in_connection(connection, &deck.id))
            .expect("archive");
        assert!(archived.archived_at.is_some());
        let restored = database
            .with_connection(|connection| unarchive_deck_in_connection(connection, &deck.id))
            .expect("unarchive");
        assert_eq!(restored.archived_at, None);
    }

    #[test]
    fn rejects_blank_deck_names() {
        let error = validate_name("   \n\t".to_string()).expect_err("name should fail");
        assert_eq!(error.code, "invalid_deck_name");
    }
}

#[cfg(test)]
mod stage36_tests;
