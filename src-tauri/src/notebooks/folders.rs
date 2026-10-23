//! Notes folders: how the learner organizes notebooks in Notes. Folders may
//! hold folders; a notebook sits in one folder or at the top, and may be
//! pinned. This is separate from the subject a notebook is filed in.
//! Deleting a folder deletes the folders inside it and sends their notebooks
//! back to the top.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::require_notebook;
use crate::{
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_NAME_CHARS: usize = 120;
/// Folders inside folders, at most this deep, so the list stays readable.
pub(crate) const MAX_FOLDER_DEPTH: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteFolderDto {
    pub id: String,
    /// The folder it is inside; None is the top of Notes.
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateNoteFolderInput {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenameNoteFolderInput {
    pub folder_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NoteFolderInput {
    pub folder_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PutNotebookInFolderInput {
    pub notebook_id: String,
    /// None puts it back at the top.
    pub folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PinNotebookInput {
    pub notebook_id: String,
    pub pinned: bool,
}

/// Where a notebook sits in Notes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Shelf {
    pub(crate) folder_id: Option<String>,
    pub(crate) pinned_at: Option<i64>,
}

#[tauri::command]
pub fn list_note_folders(
    database: State<'_, Database>,
) -> Result<Vec<NoteFolderDto>, CommandError> {
    database.with_connection(|connection| list(connection))
}

#[tauri::command]
pub fn create_note_folder(
    database: State<'_, Database>,
    input: CreateNoteFolderInput,
) -> Result<NoteFolderDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| create(connection, input, now))
}

#[tauri::command]
pub fn rename_note_folder(
    database: State<'_, Database>,
    input: RenameNoteFolderInput,
) -> Result<NoteFolderDto, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| rename(connection, input, now))
}

#[tauri::command]
pub fn delete_note_folder(
    database: State<'_, Database>,
    input: NoteFolderInput,
) -> Result<(), CommandError> {
    database.with_connection(|connection| delete(connection, &input.folder_id))
}

#[tauri::command]
pub fn put_notebook_in_folder(
    database: State<'_, Database>,
    input: PutNotebookInFolderInput,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| put_in_folder(connection, input, now))
}

#[tauri::command]
pub fn pin_notebook(
    database: State<'_, Database>,
    input: PinNotebookInput,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| pin(connection, input, now))
}

/// Every folder, parents before their folders, each level in its order.
pub(crate) fn list(connection: &Connection) -> Result<Vec<NoteFolderDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, parent_id, name, sort_order FROM note_folders
             ORDER BY sort_order, name COLLATE NOCASE, id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok(NoteFolderDto {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                name: row.get(2)?,
                sort_order: row.get(3)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    // Parents first, so a reader can build the tree in one pass.
    let mut ordered = Vec::with_capacity(rows.len());
    let mut level: Vec<Option<String>> = vec![None];
    while !level.is_empty() {
        let mut next = Vec::new();
        for parent in &level {
            for folder in rows.iter().filter(|folder| &folder.parent_id == parent) {
                next.push(Some(folder.id.clone()));
                ordered.push(folder.clone());
            }
        }
        level = next;
    }
    Ok(ordered)
}

pub(crate) fn create(
    connection: &Connection,
    input: CreateNoteFolderInput,
    now: i64,
) -> Result<NoteFolderDto, CommandError> {
    let name = clean_name(&input.name)?;
    if let Some(parent) = &input.parent_id {
        if depth(connection, parent)? >= MAX_FOLDER_DEPTH {
            return Err(invalid(format!(
                "Folders go at most {MAX_FOLDER_DEPTH} deep."
            )));
        }
    }
    let sort_order: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM note_folders WHERE parent_id IS ?1",
            [&input.parent_id],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let id = generate_id(connection)?;
    connection
        .execute(
            "INSERT INTO note_folders(id, parent_id, name, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, input.parent_id, name, sort_order, now],
        )
        .map_err(CommandError::database)?;
    Ok(NoteFolderDto {
        id,
        parent_id: input.parent_id,
        name,
        sort_order,
    })
}

fn rename(
    connection: &Connection,
    input: RenameNoteFolderInput,
    now: i64,
) -> Result<NoteFolderDto, CommandError> {
    require_folder(connection, &input.folder_id)?;
    let name = clean_name(&input.name)?;
    connection
        .execute(
            "UPDATE note_folders SET name = ?2, updated_at = MAX(?3, created_at) WHERE id = ?1",
            params![input.folder_id, name, now],
        )
        .map_err(CommandError::database)?;
    require_folder(connection, &input.folder_id)
}

/// Deleting a folder that is already gone succeeds, so a request can be repeated.
fn delete(connection: &Connection, folder_id: &str) -> Result<(), CommandError> {
    validate_entity_id(folder_id, "folder")?;
    connection
        .execute("DELETE FROM note_folders WHERE id = ?1", [folder_id])
        .map_err(CommandError::database)?;
    Ok(())
}

fn put_in_folder(
    connection: &Connection,
    input: PutNotebookInFolderInput,
    now: i64,
) -> Result<(), CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    require_notebook(connection, &input.notebook_id)?;
    if let Some(folder) = &input.folder_id {
        require_folder(connection, folder)?;
    }
    connection
        .execute(
            "INSERT INTO notebook_shelf(notebook_id, folder_id, pinned_at, updated_at)
             VALUES (?1, ?2, NULL, ?3)
             ON CONFLICT(notebook_id) DO UPDATE SET folder_id = excluded.folder_id,
                 updated_at = excluded.updated_at",
            params![input.notebook_id, input.folder_id, now],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn pin(connection: &Connection, input: PinNotebookInput, now: i64) -> Result<(), CommandError> {
    validate_entity_id(&input.notebook_id, "notebook")?;
    require_notebook(connection, &input.notebook_id)?;
    // A notebook already pinned keeps the time it was pinned.
    connection
        .execute(
            "INSERT INTO notebook_shelf(notebook_id, folder_id, pinned_at, updated_at)
             VALUES (?1, NULL, CASE WHEN ?2 THEN ?3 END, ?3)
             ON CONFLICT(notebook_id) DO UPDATE SET
                 pinned_at = CASE WHEN ?2 THEN COALESCE(pinned_at, ?3) END,
                 updated_at = ?3",
            params![input.notebook_id, input.pinned, now],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

/// Where a notebook sits; the top and unpinned when it has never been moved.
pub(crate) fn shelf(connection: &Connection, notebook_id: &str) -> Result<Shelf, CommandError> {
    Ok(connection
        .query_row(
            "SELECT folder_id, pinned_at FROM notebook_shelf WHERE notebook_id = ?1",
            [notebook_id],
            |row| {
                Ok(Shelf {
                    folder_id: row.get(0)?,
                    pinned_at: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .unwrap_or_default())
}

fn require_folder(connection: &Connection, folder_id: &str) -> Result<NoteFolderDto, CommandError> {
    validate_entity_id(folder_id, "folder")?;
    connection
        .query_row(
            "SELECT id, parent_id, name, sort_order FROM note_folders WHERE id = ?1",
            [folder_id],
            |row| {
                Ok(NoteFolderDto {
                    id: row.get(0)?,
                    parent_id: row.get(1)?,
                    name: row.get(2)?,
                    sort_order: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "note_folder_not_found",
                "That folder is no longer in Notes.",
            )
        })
}

/// How many folders deep `folder_id` is, counting itself.
fn depth(connection: &Connection, folder_id: &str) -> Result<usize, CommandError> {
    let mut current = Some(require_folder(connection, folder_id)?);
    let mut depth = 0;
    while let Some(folder) = current {
        depth += 1;
        current = match folder.parent_id {
            Some(parent) if depth <= MAX_FOLDER_DEPTH => Some(require_folder(connection, &parent)?),
            _ => None,
        };
    }
    Ok(depth)
}

fn clean_name(name: &str) -> Result<String, CommandError> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(invalid(format!(
            "Name the folder in 1 to {MAX_NAME_CHARS} characters."
        )));
    }
    Ok(name)
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new("note_folder_invalid", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notebooks::{create as create_notebook, CreateNotebookInput, PageSize, Paper};

    const NOW: i64 = 1_700_000_000_000;

    fn notebook(connection: &mut Connection, title: &str) -> String {
        create_notebook(
            connection,
            CreateNotebookInput {
                title: title.into(),
                subject_id: None,
                paper: Paper::Lined,
                page_size: PageSize::Letter,
            },
            NOW,
        )
        .unwrap()
        .notebook
        .id
    }

    fn folder(connection: &Connection, name: &str, parent: Option<&str>) -> NoteFolderDto {
        create(
            connection,
            CreateNoteFolderInput {
                name: name.into(),
                parent_id: parent.map(str::to_string),
            },
            NOW,
        )
        .unwrap()
    }

    #[test]
    fn folders_list_parents_first_in_their_order() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let physiology = folder(connection, "  Physiology ", None);
                let clinical = folder(connection, "Clinical", None);
                let gi = folder(connection, "GI", Some(&physiology.id));
                assert_eq!(physiology.name, "Physiology");
                assert_eq!(
                    (physiology.sort_order, clinical.sort_order, gi.sort_order),
                    (0, 1, 0)
                );
                let names = list(connection)?
                    .into_iter()
                    .map(|folder| folder.name)
                    .collect::<Vec<_>>();
                assert_eq!(names, ["Physiology", "Clinical", "GI"]);
                let renamed = rename(
                    connection,
                    RenameNoteFolderInput {
                        folder_id: gi.id.clone(),
                        name: "Gut".into(),
                    },
                    NOW + 1,
                )?;
                assert_eq!(renamed.name, "Gut");
                assert!(create(
                    connection,
                    CreateNoteFolderInput {
                        name: " ".into(),
                        parent_id: None
                    },
                    NOW
                )
                .is_err());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn folders_nest_only_so_deep() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let mut parent: Option<String> = None;
                for level in 0..MAX_FOLDER_DEPTH {
                    parent =
                        Some(folder(connection, &format!("Level {level}"), parent.as_deref()).id);
                }
                let error = create(
                    connection,
                    CreateNoteFolderInput {
                        name: "Too deep".into(),
                        parent_id: parent,
                    },
                    NOW,
                )
                .unwrap_err();
                assert_eq!(error.code, "note_folder_invalid");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn notebooks_go_in_folders_get_pinned_and_return_to_the_top_when_their_folder_goes() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let id = notebook(connection, "Questions");
                assert_eq!(shelf(connection, &id)?, Shelf::default());
                let physiology = folder(connection, "Physiology", None);
                let gi = folder(connection, "GI", Some(&physiology.id));
                put_in_folder(
                    connection,
                    PutNotebookInFolderInput {
                        notebook_id: id.clone(),
                        folder_id: Some(gi.id.clone()),
                    },
                    NOW,
                )?;
                pin(
                    connection,
                    PinNotebookInput {
                        notebook_id: id.clone(),
                        pinned: true,
                    },
                    NOW + 5,
                )?;
                // Pinning again keeps the first time.
                pin(
                    connection,
                    PinNotebookInput {
                        notebook_id: id.clone(),
                        pinned: true,
                    },
                    NOW + 9,
                )?;
                assert_eq!(
                    shelf(connection, &id)?,
                    Shelf {
                        folder_id: Some(gi.id.clone()),
                        pinned_at: Some(NOW + 5)
                    }
                );
                delete(connection, &physiology.id)?;
                delete(connection, &physiology.id)?;
                assert_eq!(
                    shelf(connection, &id)?,
                    Shelf {
                        folder_id: None,
                        pinned_at: Some(NOW + 5)
                    }
                );
                pin(
                    connection,
                    PinNotebookInput {
                        notebook_id: id.clone(),
                        pinned: false,
                    },
                    NOW + 10,
                )?;
                assert_eq!(shelf(connection, &id)?.pinned_at, None);
                let missing = put_in_folder(
                    connection,
                    PutNotebookInFolderInput {
                        notebook_id: id,
                        folder_id: Some(gi.id),
                    },
                    NOW,
                )
                .unwrap_err();
                assert_eq!(missing.code, "note_folder_not_found");
                Ok(())
            })
            .unwrap();
    }
}
