use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_DECK_NAME_CHARS: usize = 200;
const MAX_DECK_DESCRIPTION_CHARS: usize = 2_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeckInput {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub card_count: i64,
    pub due_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[tauri::command]
pub fn list_decks(database: State<'_, Database>) -> Result<Vec<DeckDto>, CommandError> {
    database.with_connection(list_decks_from_connection)
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

pub(crate) fn list_decks_from_connection(
    connection: &mut Connection,
) -> Result<Vec<DeckDto>, CommandError> {
    let now = unix_time_millis()?;
    let mut statement = connection
        .prepare(
            "SELECT d.id, d.name, d.description, d.created_at, d.updated_at, d.revision,\n\
                    (SELECT COUNT(*)\n\
                     FROM cards c\n\
                     JOIN notes n ON n.id = c.note_id\n\
                     WHERE c.deck_id = d.id\n\
                       AND c.deleted_at IS NULL\n\
                       AND n.deleted_at IS NULL) AS card_count,\n\
                    (SELECT COUNT(*)\n\
                     FROM cards c\n\
                     JOIN notes n ON n.id = c.note_id\n\
                     JOIN card_schedules s ON s.card_id = c.id\n\
                     WHERE c.deck_id = d.id\n\
                       AND c.deleted_at IS NULL\n\
                       AND c.suspended_at IS NULL\n\
                       AND n.deleted_at IS NULL\n\
                       AND s.due_at <= ?1) AS due_count\n\
             FROM decks d\n\
             WHERE d.deleted_at IS NULL AND d.archived_at IS NULL\n\
             ORDER BY d.sort_order ASC, d.name COLLATE NOCASE ASC, d.created_at ASC",
        )
        .map_err(CommandError::database)?;

    let rows = statement
        .query_map([now], deck_from_row)
        .map_err(CommandError::database)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub(crate) fn get_deck_from_connection(
    connection: &Connection,
    deck_id: &str,
) -> Result<DeckDto, CommandError> {
    let now = unix_time_millis()?;
    connection
        .query_row(
            "SELECT d.id, d.name, d.description, d.created_at, d.updated_at, d.revision,\n\
                    (SELECT COUNT(*)\n\
                     FROM cards c\n\
                     JOIN notes n ON n.id = c.note_id\n\
                     WHERE c.deck_id = d.id\n\
                       AND c.deleted_at IS NULL\n\
                       AND n.deleted_at IS NULL) AS card_count,\n\
                    (SELECT COUNT(*)\n\
                     FROM cards c\n\
                     JOIN notes n ON n.id = c.note_id\n\
                     JOIN card_schedules s ON s.card_id = c.id\n\
                     WHERE c.deck_id = d.id\n\
                       AND c.deleted_at IS NULL\n\
                       AND c.suspended_at IS NULL\n\
                       AND n.deleted_at IS NULL\n\
                       AND s.due_at <= ?1) AS due_count\n\
             FROM decks d\n\
             WHERE d.id = ?2\n\
               AND d.deleted_at IS NULL\n\
               AND d.archived_at IS NULL",
            params![now, deck_id],
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
            "SELECT COALESCE(MAX(sort_order), -1) + 1\n\
             FROM decks\n\
             WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    transaction
        .execute(
            "INSERT INTO decks (\n\
                 id, name, description, sort_order, created_at, updated_at, revision\n\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)",
            params![id, name, description, sort_order, now],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)?;

    Ok(DeckDto {
        id,
        name,
        description,
        card_count: 0,
        due_count: 0,
        created_at: now,
        updated_at: now,
        revision: 1,
    })
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
            "UPDATE decks\n\
             SET name = ?1, description = ?2, updated_at = ?3, revision = revision + 1\n\
             WHERE id = ?4\n\
               AND revision = ?5\n\
               AND deleted_at IS NULL\n\
               AND archived_at IS NULL",
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
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        revision: row.get(5)?,
        card_count: row.get(6)?,
        due_count: row.get(7)?,
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
            .with_connection(list_decks_from_connection)
            .expect("decks should load");
        assert_eq!(decks.len(), 1);
        assert_eq!(decks[0].id, created.id);
    }

    #[test]
    fn rejects_blank_deck_names() {
        let error = validate_name("   \n\t".to_string()).expect_err("name should fail");
        assert_eq!(error.code, "invalid_deck_name");
    }
}
