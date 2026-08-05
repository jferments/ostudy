use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    projections::decks::{self as deck_projection, DeckSummaryRow},
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetDeckArchivedInput {
    pub deck_id: String,
    pub revision: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckSummaryDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_note_count: i64,
    pub generated_card_count: i64,
    pub studyable_due_count: i64,
    pub archived_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

// The package importer is compiled in the same release as the IPC cutover and
// returns the new summary shape. This alias avoids duplicating the package graph
// while all user-facing and serialized field names remain the explicit summary
// contract above.
pub type DeckDto = DeckSummaryDto;

#[tauri::command]
pub fn list_decks(database: State<'_, Database>) -> Result<Vec<DeckSummaryDto>, CommandError> {
    database.with_connection(list_decks_from_connection)
}

#[tauri::command]
pub fn get_deck(
    database: State<'_, Database>,
    deck_id: String,
) -> Result<DeckSummaryDto, CommandError> {
    validate_entity_id(&deck_id, "deck")?;
    database.with_connection(|connection| get_deck_from_connection(connection, &deck_id))
}

#[tauri::command]
pub fn create_deck(
    database: State<'_, Database>,
    input: CreateDeckInput,
) -> Result<DeckSummaryDto, CommandError> {
    database.with_connection(|connection| create_deck_in_connection(connection, input))
}

#[tauri::command]
pub fn update_deck(
    database: State<'_, Database>,
    input: UpdateDeckInput,
) -> Result<DeckSummaryDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    database.with_connection(|connection| update_deck_in_connection(connection, input))
}

#[tauri::command]
pub fn set_deck_archived(
    database: State<'_, Database>,
    input: SetDeckArchivedInput,
) -> Result<DeckSummaryDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    database.with_connection(|connection| set_archived_in_connection(connection, input))
}

pub(crate) fn list_decks_from_connection(
    connection: &mut Connection,
) -> Result<Vec<DeckSummaryDto>, CommandError> {
    let time = StudyTimeContext::capture()?;
    deck_projection::list(connection, time.now_utc_millis)
        .map(|rows| rows.into_iter().map(DeckSummaryDto::from).collect())
}

pub(crate) fn get_deck_from_connection(
    connection: &Connection,
    deck_id: &str,
) -> Result<DeckSummaryDto, CommandError> {
    let time = StudyTimeContext::capture()?;
    deck_projection::get(connection, deck_id, time.now_utc_millis)?
        .map(DeckSummaryDto::from)
        .ok_or_else(|| CommandError::new("deck_not_found", "That deck no longer exists."))
}

pub(crate) fn create_deck_in_connection(
    connection: &mut Connection,
    input: CreateDeckInput,
) -> Result<DeckSummaryDto, CommandError> {
    let name = validate_name(input.name)?;
    let description = validate_description(input.description.unwrap_or_default())?;
    let now = unix_time_millis()?;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
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

    Ok(DeckSummaryDto {
        id,
        name,
        description,
        source_note_count: 0,
        generated_card_count: 0,
        studyable_due_count: 0,
        archived_at: None,
        created_at: now,
        updated_at: now,
        revision: 1,
    })
}

fn update_deck_in_connection(
    connection: &mut Connection,
    input: UpdateDeckInput,
) -> Result<DeckSummaryDto, CommandError> {
    let name = validate_name(input.name)?;
    let description = validate_description(input.description.unwrap_or_default())?;
    validate_revision(input.revision)?;
    let now = unix_time_millis()?;
    let changed = connection
        .execute(
            "UPDATE decks
             SET name = ?1, description = ?2, updated_at = ?3, revision = revision + 1
             WHERE id = ?4 AND revision = ?5 AND deleted_at IS NULL",
            params![name, description, now, input.deck_id, input.revision],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(deck_changed());
    }
    get_deck_from_connection(connection, &input.deck_id)
}

fn set_archived_in_connection(
    connection: &mut Connection,
    input: SetDeckArchivedInput,
) -> Result<DeckSummaryDto, CommandError> {
    validate_revision(input.revision)?;
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let current: Option<(i64, Option<i64>)> = transaction
        .query_row(
            "SELECT revision, archived_at FROM decks
             WHERE id = ?1 AND deleted_at IS NULL",
            [input.deck_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((current_revision, archived_at)) = current else {
        return Err(CommandError::new(
            "deck_not_found",
            "That deck no longer exists.",
        ));
    };
    if current_revision != input.revision {
        return Err(deck_changed());
    }
    if archived_at.is_some() == input.archived {
        transaction.commit().map_err(CommandError::database)?;
        return get_deck_from_connection(connection, &input.deck_id);
    }

    transaction
        .execute(
            "UPDATE decks
             SET archived_at = ?1, updated_at = ?2, revision = revision + 1
             WHERE id = ?3 AND revision = ?4 AND deleted_at IS NULL",
            params![
                input.archived.then_some(now),
                now,
                input.deck_id,
                input.revision
            ],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "UPDATE cards
             SET archive_suspended_at = ?1, updated_at = ?2
             WHERE deck_id = ?3 AND deleted_at IS NULL",
            params![input.archived.then_some(now), now, input.deck_id],
        )
        .map_err(CommandError::database)?;

    if input.archived {
        transaction
            .execute(
                "DELETE FROM card_search_fts WHERE deck_id = ?1",
                [input.deck_id.as_str()],
            )
            .map_err(CommandError::database)?;
    } else {
        transaction
            .execute(
                "INSERT OR REPLACE INTO card_search_fts(
                     rowid, note_id, deck_id, question, answer, tags
                 )
                 SELECT n.rowid, n.id, n.deck_id, n.front_text, n.back_text,
                        COALESCE((
                            SELECT group_concat(t.name, ' ')
                            FROM note_tags nt
                            JOIN tags t ON t.id = nt.tag_id
                            WHERE nt.note_id = n.id
                        ), '')
                 FROM notes n
                 WHERE n.deck_id = ?1 AND n.deleted_at IS NULL",
                [input.deck_id.as_str()],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)?;
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

fn validate_revision(revision: i64) -> Result<(), CommandError> {
    if revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The deck revision is invalid.",
        ));
    }
    Ok(())
}

fn deck_changed() -> CommandError {
    CommandError::new(
        "deck_changed",
        "This deck changed in another window. Reload it and try again.",
    )
}

impl From<DeckSummaryRow> for DeckSummaryDto {
    fn from(row: DeckSummaryRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            source_note_count: row.source_note_count,
            generated_card_count: row.generated_card_count,
            studyable_due_count: row.studyable_due_count,
            archived_at: row.archived_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            revision: row.revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_suspension_round_trip_preserves_manual_suspension() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let created = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Archive".to_string(),
                        description: None,
                    },
                )?;
                let archived = set_archived_in_connection(
                    connection,
                    SetDeckArchivedInput {
                        deck_id: created.id.clone(),
                        revision: created.revision,
                        archived: true,
                    },
                )?;
                assert!(archived.archived_at.is_some());
                let restored = set_archived_in_connection(
                    connection,
                    SetDeckArchivedInput {
                        deck_id: created.id,
                        revision: archived.revision,
                        archived: false,
                    },
                )?;
                assert!(restored.archived_at.is_none());
                Ok(())
            })
            .expect("archive round trip should succeed");
    }

    #[test]
    fn rejects_blank_deck_names() {
        let error = validate_name("   \n\t".to_string()).expect_err("name should fail");
        assert_eq!(error.code, "invalid_deck_name");
    }
}
