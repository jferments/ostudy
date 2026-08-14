use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_FOLDER_NAME_CHARS: usize = 120;
const ORDER_STEP: i64 = 1_000;

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeckFolderDto {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeckPlacementDto {
    pub deck_id: String,
    pub folder_id: Option<String>,
    pub sort_order: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeckLibraryOrganizationDto {
    pub revision: i64,
    pub folders: Vec<DeckFolderDto>,
    pub placements: Vec<DeckPlacementDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateDeckFolderInput {
    pub name: String,
    pub expected_organization_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateDeckFolderInput {
    pub folder_id: String,
    pub name: String,
    pub revision: i64,
    pub expected_organization_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteDeckFolderInput {
    pub folder_id: String,
    pub revision: i64,
    pub expected_organization_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveDeckInLibraryInput {
    pub deck_id: String,
    pub target_folder_id: Option<String>,
    pub target_index: i64,
    pub expected_organization_revision: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MoveDeckFolderInput {
    pub folder_id: String,
    pub target_index: i64,
    pub expected_organization_revision: i64,
}

#[tauri::command]
pub fn get_deck_library_organization(
    database: State<'_, Database>,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    database.with_connection(load_organization)
}

#[tauri::command]
pub fn create_deck_folder(
    database: State<'_, Database>,
    input: CreateDeckFolderInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    database.with_connection(|connection| create_folder(connection, input))
}

#[tauri::command]
pub fn update_deck_folder(
    database: State<'_, Database>,
    input: UpdateDeckFolderInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    validate_entity_id(&input.folder_id, "deck folder")?;
    database.with_connection(|connection| update_folder(connection, input))
}

#[tauri::command]
pub fn delete_deck_folder(
    database: State<'_, Database>,
    input: DeleteDeckFolderInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    validate_entity_id(&input.folder_id, "deck folder")?;
    database.with_connection(|connection| delete_folder(connection, input))
}

#[tauri::command]
pub fn move_deck_in_library(
    database: State<'_, Database>,
    input: MoveDeckInLibraryInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    validate_entity_id(&input.deck_id, "deck")?;
    if let Some(folder_id) = input.target_folder_id.as_deref() {
        validate_entity_id(folder_id, "deck folder")?;
    }
    database.with_connection(|connection| move_deck(connection, input))
}

#[tauri::command]
pub fn move_deck_folder(
    database: State<'_, Database>,
    input: MoveDeckFolderInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    validate_entity_id(&input.folder_id, "deck folder")?;
    database.with_connection(|connection| move_folder(connection, input))
}

fn load_organization(
    connection: &mut Connection,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    let revision = organization_revision(connection)?;
    let folders = {
        let mut statement = connection
            .prepare(
                "SELECT id, name, sort_order, created_at, updated_at, revision
                 FROM deck_folders
                 ORDER BY sort_order ASC, name COLLATE NOCASE ASC, created_at ASC",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([], |row| {
                Ok(DeckFolderDto {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    sort_order: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    revision: row.get(5)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    let placements = {
        let mut statement = connection
            .prepare(
                "SELECT id, folder_id, sort_order
                 FROM decks
                 WHERE deleted_at IS NULL
                 ORDER BY folder_id IS NOT NULL DESC, folder_id ASC,
                          sort_order ASC, name COLLATE NOCASE ASC, created_at ASC",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([], |row| {
                Ok(DeckPlacementDto {
                    deck_id: row.get(0)?,
                    folder_id: row.get(1)?,
                    sort_order: row.get(2)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?
    };
    Ok(DeckLibraryOrganizationDto {
        revision,
        folders,
        placements,
    })
}

fn create_folder(
    connection: &mut Connection,
    input: CreateDeckFolderInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    let name = validate_folder_name(input.name)?;
    validate_revision(input.expected_organization_revision, "library organization")?;
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&transaction, input.expected_organization_revision)?;
    let id = generate_id(&transaction)?;
    let sort_order: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + ?1 FROM deck_folders",
            [ORDER_STEP],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "INSERT INTO deck_folders (
                 id, name, sort_order, created_at, updated_at, revision
             ) VALUES (?1, ?2, ?3, ?4, ?4, 1)",
            params![id, name, sort_order, now],
        )
        .map_err(CommandError::database)?;
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    load_organization(connection)
}

fn update_folder(
    connection: &mut Connection,
    input: UpdateDeckFolderInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    let name = validate_folder_name(input.name)?;
    validate_revision(input.revision, "deck folder")?;
    validate_revision(input.expected_organization_revision, "library organization")?;
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&transaction, input.expected_organization_revision)?;
    let changed = transaction
        .execute(
            "UPDATE deck_folders
             SET name = ?1, updated_at = ?2, revision = revision + 1
             WHERE id = ?3 AND revision = ?4",
            params![name, now, input.folder_id, input.revision],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(folder_changed());
    }
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    load_organization(connection)
}

fn delete_folder(
    connection: &mut Connection,
    input: DeleteDeckFolderInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    validate_revision(input.revision, "deck folder")?;
    validate_revision(input.expected_organization_revision, "library organization")?;
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&transaction, input.expected_organization_revision)?;
    let current_revision: Option<i64> = transaction
        .query_row(
            "SELECT revision FROM deck_folders WHERE id = ?1",
            [input.folder_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    match current_revision {
        None => return Err(folder_not_found()),
        Some(revision) if revision != input.revision => return Err(folder_changed()),
        Some(_) => {}
    }

    let mut unfiled = ordered_deck_ids(&transaction, None, None)?;
    let moved = ordered_deck_ids(&transaction, Some(input.folder_id.as_str()), None)?;
    unfiled.extend(moved.iter().cloned());
    transaction
        .execute(
            "UPDATE decks
             SET folder_id = NULL, updated_at = ?1
             WHERE folder_id = ?2 AND deleted_at IS NULL",
            params![now, input.folder_id],
        )
        .map_err(CommandError::database)?;
    write_deck_order(&transaction, None, &unfiled, now)?;
    let deleted = transaction
        .execute(
            "DELETE FROM deck_folders WHERE id = ?1 AND revision = ?2",
            params![input.folder_id, input.revision],
        )
        .map_err(CommandError::database)?;
    if deleted != 1 {
        return Err(folder_changed());
    }
    renormalize_folder_order(&transaction, now)?;
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    load_organization(connection)
}

fn move_deck(
    connection: &mut Connection,
    input: MoveDeckInLibraryInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    validate_revision(input.expected_organization_revision, "library organization")?;
    if input.target_index < 0 {
        return Err(invalid_target_index());
    }
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&transaction, input.expected_organization_revision)?;
    assert_folder_exists(&transaction, input.target_folder_id.as_deref())?;
    let previous_folder: Option<Option<String>> = transaction
        .query_row(
            "SELECT folder_id FROM decks WHERE id = ?1 AND deleted_at IS NULL",
            [input.deck_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some(previous_folder) = previous_folder else {
        return Err(CommandError::new(
            "deck_not_found",
            "That deck no longer exists.",
        ));
    };

    let mut target_order = ordered_deck_ids(
        &transaction,
        input.target_folder_id.as_deref(),
        Some(input.deck_id.as_str()),
    )?;
    let target_index = usize::try_from(input.target_index).map_err(|_| invalid_target_index())?;
    if target_index > target_order.len() {
        return Err(invalid_target_index());
    }
    target_order.insert(target_index, input.deck_id.clone());
    transaction
        .execute(
            "UPDATE decks SET folder_id = ?1, updated_at = ?2
             WHERE id = ?3 AND deleted_at IS NULL",
            params![input.target_folder_id, now, input.deck_id],
        )
        .map_err(CommandError::database)?;
    write_deck_order(
        &transaction,
        input.target_folder_id.as_deref(),
        &target_order,
        now,
    )?;
    if previous_folder.as_deref() != input.target_folder_id.as_deref() {
        let previous_order = ordered_deck_ids(&transaction, previous_folder.as_deref(), None)?;
        write_deck_order(
            &transaction,
            previous_folder.as_deref(),
            &previous_order,
            now,
        )?;
    }
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    load_organization(connection)
}

fn move_folder(
    connection: &mut Connection,
    input: MoveDeckFolderInput,
) -> Result<DeckLibraryOrganizationDto, CommandError> {
    validate_revision(input.expected_organization_revision, "library organization")?;
    if input.target_index < 0 {
        return Err(invalid_target_index());
    }
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    assert_organization_revision(&transaction, input.expected_organization_revision)?;
    let exists: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM deck_folders WHERE id = ?1)",
            [input.folder_id.as_str()],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if !exists {
        return Err(folder_not_found());
    }
    let mut order = ordered_folder_ids(&transaction, Some(input.folder_id.as_str()))?;
    let target_index = usize::try_from(input.target_index).map_err(|_| invalid_target_index())?;
    if target_index > order.len() {
        return Err(invalid_target_index());
    }
    order.insert(target_index, input.folder_id);
    write_folder_order(&transaction, &order, now)?;
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    load_organization(connection)
}

fn organization_revision(connection: &Connection) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT revision FROM library_organization WHERE singleton_id = 1",
            [],
            |row| row.get(0),
        )
        .map_err(CommandError::database)
}

fn assert_organization_revision(
    transaction: &Transaction<'_>,
    expected: i64,
) -> Result<(), CommandError> {
    if organization_revision(transaction)? == expected {
        Ok(())
    } else {
        Err(organization_changed())
    }
}

fn bump_organization(
    transaction: &Transaction<'_>,
    expected: i64,
    now: i64,
) -> Result<(), CommandError> {
    let changed = transaction
        .execute(
            "UPDATE library_organization
             SET revision = revision + 1, updated_at = ?1
             WHERE singleton_id = 1 AND revision = ?2",
            params![now, expected],
        )
        .map_err(CommandError::database)?;
    if changed == 1 {
        Ok(())
    } else {
        Err(organization_changed())
    }
}

fn assert_folder_exists(
    transaction: &Transaction<'_>,
    folder_id: Option<&str>,
) -> Result<(), CommandError> {
    let Some(folder_id) = folder_id else {
        return Ok(());
    };
    let exists: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM deck_folders WHERE id = ?1)",
            [folder_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    if exists {
        Ok(())
    } else {
        Err(folder_not_found())
    }
}

fn ordered_deck_ids(
    connection: &Connection,
    folder_id: Option<&str>,
    excluding: Option<&str>,
) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id FROM decks
             WHERE deleted_at IS NULL
               AND folder_id IS ?1
               AND (?2 IS NULL OR id <> ?2)
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC, created_at ASC",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map(params![folder_id, excluding], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn ordered_folder_ids(
    connection: &Connection,
    excluding: Option<&str>,
) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id FROM deck_folders
             WHERE (?1 IS NULL OR id <> ?1)
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC, created_at ASC",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map([excluding], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn write_deck_order(
    transaction: &Transaction<'_>,
    folder_id: Option<&str>,
    ordered_ids: &[String],
    now: i64,
) -> Result<(), CommandError> {
    for (index, deck_id) in ordered_ids.iter().enumerate() {
        let order = order_for_index(index)?;
        let changed = transaction
            .execute(
                "UPDATE decks
                 SET folder_id = ?1, sort_order = ?2, updated_at = ?3
                 WHERE id = ?4 AND deleted_at IS NULL",
                params![folder_id, order, now, deck_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "deck_not_found",
                "A deck changed while its library position was being updated.",
            ));
        }
    }
    Ok(())
}

fn write_folder_order(
    transaction: &Transaction<'_>,
    ordered_ids: &[String],
    now: i64,
) -> Result<(), CommandError> {
    for (index, folder_id) in ordered_ids.iter().enumerate() {
        let order = order_for_index(index)?;
        let changed = transaction
            .execute(
                "UPDATE deck_folders SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                params![order, now, folder_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(folder_not_found());
        }
    }
    Ok(())
}

fn renormalize_folder_order(transaction: &Transaction<'_>, now: i64) -> Result<(), CommandError> {
    let order = ordered_folder_ids(transaction, None)?;
    write_folder_order(transaction, &order, now)
}

fn order_for_index(index: usize) -> Result<i64, CommandError> {
    let position = i64::try_from(index)
        .ok()
        .and_then(|value| value.checked_add(1))
        .and_then(|value| value.checked_mul(ORDER_STEP));
    position.ok_or_else(|| {
        CommandError::new(
            "library_order_too_large",
            "The deck library contains too many ordered items.",
        )
    })
}

fn validate_folder_name(value: String) -> Result<String, CommandError> {
    let trimmed = value.trim();
    let count = trimmed.chars().count();
    if count == 0 {
        return Err(CommandError::new(
            "invalid_deck_folder_name",
            "Enter a name for the folder.",
        ));
    }
    if count > MAX_FOLDER_NAME_CHARS {
        return Err(CommandError::new(
            "invalid_deck_folder_name",
            format!("Folder names cannot exceed {MAX_FOLDER_NAME_CHARS} characters."),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_revision(revision: i64, label: &str) -> Result<(), CommandError> {
    if revision >= 1 {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_revision",
            format!("The {label} revision is invalid."),
        ))
    }
}

fn organization_changed() -> CommandError {
    CommandError::new(
        "library_organization_changed",
        "The deck library organization changed. Reload it and try again.",
    )
}

fn folder_changed() -> CommandError {
    CommandError::new(
        "deck_folder_changed",
        "This folder changed in another window. Reload it and try again.",
    )
}

fn folder_not_found() -> CommandError {
    CommandError::new(
        "deck_folder_not_found",
        "That deck folder no longer exists.",
    )
}

fn invalid_target_index() -> CommandError {
    CommandError::new(
        "invalid_library_target",
        "The requested library position is no longer available.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decks::{create_deck_in_connection, CreateDeckInput};

    #[test]
    fn folder_delete_returns_decks_to_unfiled_without_deleting_them() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let deck = create_deck_in_connection(
                    connection,
                    CreateDeckInput {
                        name: "Pharmacology".to_string(),
                        description: None,
                    },
                )?;
                let current_revision = organization_revision(connection)?;
                let created = create_folder(
                    connection,
                    CreateDeckFolderInput {
                        name: "Nursing".to_string(),
                        expected_organization_revision: current_revision,
                    },
                )?;
                let folder = created.folders[0].clone();
                let moved = move_deck(
                    connection,
                    MoveDeckInLibraryInput {
                        deck_id: deck.id.clone(),
                        target_folder_id: Some(folder.id.clone()),
                        target_index: 0,
                        expected_organization_revision: created.revision,
                    },
                )?;
                assert_eq!(
                    moved.placements[0].folder_id.as_deref(),
                    Some(folder.id.as_str())
                );
                let deleted = delete_folder(
                    connection,
                    DeleteDeckFolderInput {
                        folder_id: folder.id,
                        revision: folder.revision,
                        expected_organization_revision: moved.revision,
                    },
                )?;
                assert!(deleted.folders.is_empty());
                assert_eq!(deleted.placements[0].deck_id, deck.id);
                assert_eq!(deleted.placements[0].folder_id, None);
                Ok(())
            })
            .expect("folder lifecycle should succeed");
    }

    #[test]
    fn stale_organization_revision_rejects_reorder() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let created = create_folder(
                    connection,
                    CreateDeckFolderInput {
                        name: "Current".to_string(),
                        expected_organization_revision: 1,
                    },
                )?;
                let error = move_folder(
                    connection,
                    MoveDeckFolderInput {
                        folder_id: created.folders[0].id.clone(),
                        target_index: 0,
                        expected_organization_revision: 1,
                    },
                )
                .expect_err("stale revision should fail");
                assert_eq!(error.code, "library_organization_changed");
                Ok(())
            })
            .expect("test should complete");
    }
}
