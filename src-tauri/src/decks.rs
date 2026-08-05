use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    study_calendar::StudyTimeContext,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_DECK_NAME_CHARS: usize = 120;
const MAX_DECK_DESCRIPTION_CHARS: usize = 2_000;

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

#[tauri::command]
pub fn list_decks(database: State<'_, Database>) -> Result<Vec<DeckDto>, CommandError> {
    database.with_connection(|connection| {
        list_decks_in_connection(connection, false, &StudyTimeContext::capture()?)
    })
}

#[tauri::command]
pub fn list_archived_decks(database: State<'_, Database>) -> Result<Vec<DeckDto>, CommandError> {
    database.with_connection(|connection| {
        list_decks_in_connection(connection, true, &StudyTimeContext::capture()?)
    })
}

#[tauri::command]
pub fn get_deck(database: State<'_, Database>, deck_id: String) -> Result<DeckDto, CommandError> {
    validate_entity_id(&deck_id, "deck")?;
    database.with_connection(|connection| {
        load_deck_summary(connection, &deck_id, false, &StudyTimeContext::capture()?)
    })
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
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The deck revision is invalid.",
        ));
    }
    let name = validate_name(input.name)?;
    let description = validate_description(input.description.unwrap_or_default())?;
    let normalized = normalize_name(&name);
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        ensure_name_available(&transaction, &normalized, Some(&input.deck_id))?;
        let changed = transaction
            .execute(
                "UPDATE decks
                 SET name = ?1, normalized_name = ?2, description = ?3,
                     updated_at = ?4, revision = revision + 1
                 WHERE id = ?5 AND revision = ?6
                   AND deleted_at IS NULL",
                params![
                    name,
                    normalized,
                    description,
                    now,
                    input.deck_id,
                    input.revision,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "deck_changed",
                "This deck changed in another window. Reload it and try again.",
            ));
        }
        transaction.commit().map_err(CommandError::database)?;
        load_deck_summary(
            connection,
            &input.deck_id,
            true,
            &StudyTimeContext::at(now)?,
        )
    })
}

#[tauri::command]
pub fn archive_deck(
    database: State<'_, Database>,
    deck_id: String,
) -> Result<DeckDto, CommandError> {
    validate_entity_id(&deck_id, "deck")?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let exists: i64 = transaction
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM decks
                     WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NULL
                 )",
                [deck_id.as_str()],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if exists == 0 {
            return Err(CommandError::new(
                "deck_not_found",
                "That active deck no longer exists.",
            ));
        }

        transaction
            .execute(
                "INSERT INTO deck_archive_cards
                     (deck_id, card_id, archived_at, suspended_before)
                 SELECT ?1, c.id, ?2, c.suspended_at
                 FROM cards c
                 JOIN notes n ON n.id = c.note_id
                 WHERE c.deck_id = ?1
                   AND c.deleted_at IS NULL
                   AND n.deleted_at IS NULL",
                params![deck_id, now],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE cards
                 SET suspended_at = COALESCE(suspended_at, ?1), updated_at = ?1
                 WHERE deck_id = ?2 AND deleted_at IS NULL",
                params![now, deck_id],
            )
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
                "deck_changed",
                "The deck changed while it was being archived.",
            ));
        }
        transaction.commit().map_err(CommandError::database)?;
        load_deck_summary(connection, &deck_id, true, &StudyTimeContext::at(now)?)
    })
}

#[tauri::command]
pub fn unarchive_deck(
    database: State<'_, Database>,
    deck_id: String,
) -> Result<DeckDto, CommandError> {
    validate_entity_id(&deck_id, "deck")?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let archived_at: i64 = connection
            .query_row(
                "SELECT archived_at FROM decks
                 WHERE id = ?1 AND deleted_at IS NULL AND archived_at IS NOT NULL",
                [deck_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new("deck_not_found", "That archived deck no longer exists.")
            })?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE cards
                 SET suspended_at = NULL, updated_at = ?1
                 WHERE deck_id = ?2
                   AND deleted_at IS NULL
                   AND suspended_at = ?3
                   AND id IN (
                       SELECT card_id FROM deck_archive_cards
                       WHERE deck_id = ?2 AND suspended_before IS NULL
                   )",
                params![now, deck_id, archived_at],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "DELETE FROM deck_archive_cards WHERE deck_id = ?1",
                [deck_id.as_str()],
            )
            .map_err(CommandError::database)?;
        let changed = transaction
            .execute(
                "UPDATE decks
                 SET archived_at = NULL, updated_at = ?1, revision = revision + 1
                 WHERE id = ?2 AND deleted_at IS NULL AND archived_at = ?3",
                params![now, deck_id, archived_at],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "deck_changed",
                "The deck changed while it was being restored.",
            ));
        }
        transaction.commit().map_err(CommandError::database)?;
        load_deck_summary(connection, &deck_id, false, &StudyTimeContext::at(now)?)
    })
}

pub(crate) fn get_deck_from_connection(
    connection: &Connection,
    deck_id: &str,
) -> Result<DeckDto, CommandError> {
    load_deck_summary(connection, deck_id, false, &StudyTimeContext::capture()?)
}

pub(crate) fn create_deck_in_connection(
    connection: &mut Connection,
    input: CreateDeckInput,
) -> Result<DeckDto, CommandError> {
    let name = validate_name(input.name)?;
    let description = validate_description(input.description.unwrap_or_default())?;
    let normalized = normalize_name(&name);
    ensure_name_available(connection, &normalized, None)?;
    let now = unix_time_millis()?;
    let id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO decks (
                 id, name, normalized_name, description,
                 created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)",
            params![id, name, normalized, description, now],
        )
        .map_err(CommandError::database)?;
    load_deck_summary(connection, &id, false, &StudyTimeContext::at(now)?)
}

pub(crate) fn list_decks_in_connection(
    connection: &Connection,
    archived: bool,
    context: &StudyTimeContext,
) -> Result<Vec<DeckDto>, CommandError> {
    let archived_clause = if archived {
        "d.archived_at IS NOT NULL"
    } else {
        "d.archived_at IS NULL"
    };
    let sql = format!(
        "SELECT d.id, d.name, d.description,
                COUNT(DISTINCT CASE WHEN n.deleted_at IS NULL THEN n.id END),
                COUNT(CASE
                    WHEN n.deleted_at IS NULL AND c.deleted_at IS NULL THEN c.id
                END),
                COALESCE(SUM(CASE
                    WHEN d.archived_at IS NULL
                     AND n.deleted_at IS NULL
                     AND c.deleted_at IS NULL
                     AND c.suspended_at IS NULL
                     AND (c.buried_until IS NULL OR c.buried_until <= ?1)
                     AND s.due_at <= ?1
                    THEN 1 ELSE 0 END), 0),
                d.created_at, d.updated_at, d.archived_at, d.revision
         FROM decks d
         LEFT JOIN notes n ON n.deck_id = d.id
         LEFT JOIN cards c ON c.note_id = n.id AND c.deck_id = d.id
         LEFT JOIN card_schedules s ON s.card_id = c.id
         WHERE d.deleted_at IS NULL AND {archived_clause}
         GROUP BY d.id, d.name, d.description, d.created_at,
                  d.updated_at, d.archived_at, d.revision
         ORDER BY d.name COLLATE NOCASE, d.created_at, d.id"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map([context.now_utc_millis], deck_from_row)
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub(crate) fn load_deck_summary(
    connection: &Connection,
    deck_id: &str,
    include_archived: bool,
    context: &StudyTimeContext,
) -> Result<DeckDto, CommandError> {
    let archived_clause = if include_archived {
        ""
    } else {
        "AND d.archived_at IS NULL"
    };
    let sql = format!(
        "SELECT d.id, d.name, d.description,
                COUNT(DISTINCT CASE WHEN n.deleted_at IS NULL THEN n.id END),
                COUNT(CASE
                    WHEN n.deleted_at IS NULL AND c.deleted_at IS NULL THEN c.id
                END),
                COALESCE(SUM(CASE
                    WHEN d.archived_at IS NULL
                     AND n.deleted_at IS NULL
                     AND c.deleted_at IS NULL
                     AND c.suspended_at IS NULL
                     AND (c.buried_until IS NULL OR c.buried_until <= ?2)
                     AND s.due_at <= ?2
                    THEN 1 ELSE 0 END), 0),
                d.created_at, d.updated_at, d.archived_at, d.revision
         FROM decks d
         LEFT JOIN notes n ON n.deck_id = d.id
         LEFT JOIN cards c ON c.note_id = n.id AND c.deck_id = d.id
         LEFT JOIN card_schedules s ON s.card_id = c.id
         WHERE d.id = ?1 AND d.deleted_at IS NULL {archived_clause}
         GROUP BY d.id, d.name, d.description, d.created_at,
                  d.updated_at, d.archived_at, d.revision"
    );
    connection
        .query_row(
            &sql,
            params![deck_id, context.now_utc_millis],
            deck_from_row,
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| CommandError::new("deck_not_found", "That deck no longer exists."))
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

pub(crate) fn validate_name(value: String) -> Result<String, CommandError> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() || normalized.chars().count() > MAX_DECK_NAME_CHARS {
        return Err(CommandError::new(
            "invalid_deck_name",
            format!("Deck names must contain 1 to {MAX_DECK_NAME_CHARS} characters."),
        ));
    }
    Ok(normalized)
}

pub(crate) fn validate_description(value: String) -> Result<String, CommandError> {
    let value = value.trim().to_string();
    if value.chars().count() > MAX_DECK_DESCRIPTION_CHARS {
        return Err(CommandError::new(
            "invalid_deck_description",
            format!("Deck descriptions cannot exceed {MAX_DECK_DESCRIPTION_CHARS} characters."),
        ));
    }
    Ok(value)
}

fn normalize_name(value: &str) -> String {
    value.to_lowercase()
}

fn ensure_name_available(
    connection: &Connection,
    normalized_name: &str,
    except_id: Option<&str>,
) -> Result<(), CommandError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM decks
                 WHERE normalized_name = ?1 AND deleted_at IS NULL
                   AND (?2 IS NULL OR id <> ?2)
             )",
            params![normalized_name, except_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists != 0 {
        return Err(CommandError::new(
            "duplicate_deck_name",
            "A deck with that name already exists.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_ownership_preserves_manual_suspension() {
        let database = Database::open_in_memory().expect("database");
        database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Archive".to_string(),
                        description: None,
                    },
                )?;
                let now = unix_time_millis()?;
                let note_id = generate_id(connection)?;
                let first = generate_id(connection)?;
                let second = generate_id(connection)?;
                connection
                    .execute(
                        "INSERT INTO notes (
                             id, deck_id, note_type, content_format, content_schema_version,
                             front_doc, back_doc, front_text, back_text, duplicate_hash,
                             created_at, updated_at, revision
                         ) VALUES (
                             ?1, ?2, 'basic', 'tiptap-json', 1,
                             '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}',
                             '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}',
                             'Q', 'A', '', ?3, ?3, 1
                         )",
                        params![note_id, deck.id, now],
                    )
                    .map_err(CommandError::database)?;
                for (card_id, direction) in [(&first, "forward"), (&second, "reverse")] {
                    connection
                        .execute(
                            "INSERT INTO cards (
                                 id, note_id, deck_id, direction, template_ordinal,
                                 created_at, updated_at
                             ) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
                            params![card_id, note_id, deck.id, direction, now],
                        )
                        .map_err(CommandError::database)?;
                    crate::scheduler::insert_new_schedule(connection, card_id, now)?;
                }
                connection
                    .execute(
                        "UPDATE cards SET suspended_at = ?1 WHERE id = ?2",
                        params![now - 1, second],
                    )
                    .map_err(CommandError::database)?;
                archive_deck_for_test(connection, &deck.id, now)?;
                unarchive_deck_for_test(connection, &deck.id, now + 10)?;
                let states: (Option<i64>, Option<i64>) = connection
                    .query_row(
                        "SELECT
                             (SELECT suspended_at FROM cards WHERE id = ?1),
                             (SELECT suspended_at FROM cards WHERE id = ?2)",
                        params![first, second],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert!(states.0.is_none());
                assert_eq!(states.1, Some(now - 1));
                Ok(())
            })
            .expect("archive");
    }

    fn archive_deck_for_test(
        connection: &mut Connection,
        deck_id: &str,
        now: i64,
    ) -> Result<(), CommandError> {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO deck_archive_cards
                     (deck_id, card_id, archived_at, suspended_before)
                 SELECT ?1, id, ?2, suspended_at
                 FROM cards WHERE deck_id = ?1 AND deleted_at IS NULL",
                params![deck_id, now],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE cards SET suspended_at = COALESCE(suspended_at, ?1)
                 WHERE deck_id = ?2 AND deleted_at IS NULL",
                params![now, deck_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE decks SET archived_at = ?1 WHERE id = ?2",
                params![now, deck_id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    }

    fn unarchive_deck_for_test(
        connection: &mut Connection,
        deck_id: &str,
        now: i64,
    ) -> Result<(), CommandError> {
        let archived_at: i64 = connection
            .query_row(
                "SELECT archived_at FROM decks WHERE id = ?1",
                [deck_id],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE cards SET suspended_at = NULL, updated_at = ?1
                 WHERE deck_id = ?2 AND suspended_at = ?3
                   AND id IN (
                       SELECT card_id FROM deck_archive_cards
                       WHERE deck_id = ?2 AND suspended_before IS NULL
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
        transaction
            .execute(
                "UPDATE decks SET archived_at = NULL WHERE id = ?1",
                [deck_id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    }
}
