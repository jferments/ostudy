//! The deck library's folder commands, kept working on top of subjects.
//!
//! Since release schema 6 a deck folder is a subject (see `subjects`): the
//! folders are every subject, a deck's folder is its subject, and the
//! organization revision is the library revision. The folder screens keep
//! this wire format until the Library replaces them.

use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    subjects::{self, Outcome, Revision, MAX_SUBJECT_NAME_CHARS as MAX_FOLDER_NAME_CHARS},
    util::{unix_time_millis, validate_entity_id},
};

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
    organization(connection)
}

fn organization(connection: &Connection) -> Result<DeckLibraryOrganizationDto, CommandError> {
    let revision = subjects::revision(connection, Revision::Library)?;
    let folders = subjects::list_subjects(connection)?
        .into_iter()
        .map(|subject| DeckFolderDto {
            id: subject.id,
            name: subject.name,
            sort_order: subject.sort_order,
            created_at: subject.created_at,
            updated_at: subject.updated_at,
            revision: subject.revision,
        })
        .collect();
    let placements = subjects::deck_placements(connection)?
        .into_iter()
        .map(|placement| DeckPlacementDto {
            deck_id: placement.deck_id,
            folder_id: placement.subject_id,
            sort_order: placement.sort_order,
        })
        .collect();
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
    subjects::insert_subject(&transaction, &name, now)?;
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    organization(connection)
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
    let renamed =
        subjects::rename_subject_row(&transaction, &input.folder_id, &name, input.revision, now)?;
    if renamed != Outcome::Done {
        return Err(folder_changed());
    }
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    organization(connection)
}

/// Deletes a folder, returning its decks to the end of the unfiled decks. A
/// folder is a subject, and one that also holds documents or study materials
/// is deleted from Documents, where they are shown, rather than from here.
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
    match subjects::subject_revision(&transaction, &input.folder_id)? {
        None => return Err(folder_not_found()),
        Some(revision) if revision != input.revision => return Err(folder_changed()),
        Some(_) => {}
    }
    if subjects::subject_holds_documents(&transaction, &input.folder_id)? {
        return Err(CommandError::new(
            "deck_folder_holds_documents",
            "This folder also holds documents or study materials. Delete it from Documents instead.",
        ));
    }
    match subjects::delete_subject_row(&transaction, &input.folder_id, input.revision, now)? {
        Outcome::Done => {}
        Outcome::Missing => return Err(folder_not_found()),
        Outcome::Stale | Outcome::OutOfRange => return Err(folder_changed()),
    }
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    organization(connection)
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
    if let Some(folder_id) = input.target_folder_id.as_deref() {
        if !subjects::subject_exists(&transaction, folder_id)? {
            return Err(folder_not_found());
        }
    }
    match subjects::file_deck(
        &transaction,
        &input.deck_id,
        input.target_folder_id.as_deref(),
        Some(input.target_index),
        now,
    )? {
        Outcome::Done => {}
        Outcome::Missing => {
            return Err(CommandError::new(
                "deck_not_found",
                "That deck no longer exists.",
            ))
        }
        Outcome::Stale | Outcome::OutOfRange => return Err(invalid_target_index()),
    }
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    organization(connection)
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
    match subjects::move_subject_row(&transaction, &input.folder_id, input.target_index, now)? {
        Outcome::Done => {}
        Outcome::Missing => return Err(folder_not_found()),
        Outcome::Stale | Outcome::OutOfRange => return Err(invalid_target_index()),
    }
    bump_organization(&transaction, input.expected_organization_revision, now)?;
    transaction.commit().map_err(CommandError::database)?;
    organization(connection)
}

fn assert_organization_revision(
    transaction: &Transaction<'_>,
    expected: i64,
) -> Result<(), CommandError> {
    if subjects::revision(transaction, Revision::Library)? == expected {
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
    if subjects::bump(transaction, Revision::Library, Some(expected), now)? {
        Ok(())
    } else {
        Err(organization_changed())
    }
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
    use rusqlite::params;

    use super::*;
    use crate::{
        collections::test_support::seed_document,
        decks::{create_deck_in_connection, CreateDeckInput},
    };

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
                let current_revision = subjects::revision(connection, Revision::Library)?;
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

    #[test]
    fn folders_are_subjects_and_leave_document_filing_alone() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let filing = subjects::revision(connection, Revision::DocumentFiling)?;
                let created = create_folder(
                    connection,
                    CreateDeckFolderInput {
                        name: " Nursing ".to_string(),
                        expected_organization_revision: subjects::revision(
                            connection,
                            Revision::Library,
                        )?,
                    },
                )?;
                let folder = created.folders[0].clone();
                let renamed = update_folder(
                    connection,
                    UpdateDeckFolderInput {
                        folder_id: folder.id.clone(),
                        name: "Clinical nursing".to_string(),
                        revision: folder.revision,
                        expected_organization_revision: created.revision,
                    },
                )?;
                let library = subjects::load_library(connection)?;
                assert_eq!(library.subjects.len(), 1);
                assert_eq!(library.subjects[0].id, folder.id);
                assert_eq!(library.subjects[0].name, "Clinical nursing");
                assert_eq!(library.library_revision, renamed.revision);
                assert_eq!(library.document_filing_revision, filing);

                // A folder whose subject holds documents is deleted from
                // Documents, where they are shown.
                let document = "d".repeat(32);
                seed_document(connection, &document, 1, 'a')?;
                connection
                    .execute(
                        "UPDATE documents SET subject_id = ?1 WHERE id = ?2",
                        params![folder.id, document],
                    )
                    .map_err(CommandError::database)?;
                let refused = delete_folder(
                    connection,
                    DeleteDeckFolderInput {
                        folder_id: folder.id.clone(),
                        revision: renamed.folders[0].revision,
                        expected_organization_revision: renamed.revision,
                    },
                )
                .expect_err("a folder holding documents should stay");
                assert_eq!(refused.code, "deck_folder_holds_documents");
                assert_eq!(organization(connection)?, renamed);
                Ok(())
            })
            .expect("test should complete");
    }
}
