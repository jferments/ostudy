use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

use super::ensure_active_note;

const MAX_TAGS_PER_CARD: usize = 50;
const MAX_TAG_CHARACTERS: usize = 80;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagSummaryDto {
    pub id: String,
    pub name: String,
    pub card_count: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetCardTagsInput {
    pub note_id: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct NormalizedTag {
    pub(crate) name: String,
    pub(crate) normalized_name: String,
}

#[tauri::command]
pub fn set_card_tags(
    database: State<'_, Database>,
    input: SetCardTagsInput,
) -> Result<Vec<TagDto>, CommandError> {
    validate_entity_id(&input.note_id, "note")?;
    let tags = normalize_tag_names(input.tags)?;
    database.with_connection(|connection| {
        set_card_tags_in_connection(connection, &input.note_id, &tags)
    })
}

pub(crate) fn set_card_tags_in_connection(
    connection: &mut Connection,
    note_id: &str,
    tags: &[NormalizedTag],
) -> Result<Vec<TagDto>, CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    ensure_active_note(&transaction, note_id)?;
    replace_note_tags(&transaction, note_id, tags, now)?;

    // Tags are card metadata. They affect updated ordering, but they do not
    // change the rich-document revision used for optimistic editor writes.
    transaction
        .execute(
            "UPDATE notes SET updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![now, note_id],
        )
        .map_err(CommandError::database)?;
    transaction
        .execute(
            "UPDATE cards SET updated_at = ?1 WHERE note_id = ?2 AND deleted_at IS NULL",
            params![now, note_id],
        )
        .map_err(CommandError::database)?;
    remove_orphan_tags(&transaction)?;
    transaction.commit().map_err(CommandError::database)?;
    load_tags_for_note(connection, note_id)
}

pub(crate) fn normalize_tag_names(values: Vec<String>) -> Result<Vec<NormalizedTag>, CommandError> {
    if values.len() > MAX_TAGS_PER_CARD {
        return Err(CommandError::new(
            "too_many_tags",
            format!("A flashcard can have at most {MAX_TAGS_PER_CARD} tags."),
        ));
    }

    let mut normalized_names = BTreeSet::new();
    let mut tags = Vec::new();
    for value in values {
        let name = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            continue;
        }
        if name.chars().count() > MAX_TAG_CHARACTERS {
            return Err(CommandError::new(
                "tag_too_long",
                format!("Tags cannot exceed {MAX_TAG_CHARACTERS} characters."),
            ));
        }
        if name
            .chars()
            .any(|character| character.is_control() || character == ',')
        {
            return Err(CommandError::new(
                "invalid_tag",
                "Tags cannot contain commas or control characters.",
            ));
        }
        let normalized_name = name.to_lowercase();
        if normalized_names.insert(normalized_name.clone()) {
            tags.push(NormalizedTag {
                name,
                normalized_name,
            });
        }
    }

    if tags.len() > MAX_TAGS_PER_CARD {
        return Err(CommandError::new(
            "too_many_tags",
            format!("A flashcard can have at most {MAX_TAGS_PER_CARD} tags."),
        ));
    }
    Ok(tags)
}

pub(crate) fn replace_note_tags(
    connection: &Connection,
    note_id: &str,
    tags: &[NormalizedTag],
    now: i64,
) -> Result<(), CommandError> {
    connection
        .execute("DELETE FROM note_tags WHERE note_id = ?1", [note_id])
        .map_err(CommandError::database)?;

    for tag in tags {
        let existing_id = connection
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                [&tag.normalized_name],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        let tag_id = if let Some(id) = existing_id {
            id
        } else {
            let id = generate_id(connection)?;
            connection
                .execute(
                    "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)\n\
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![id, tag.name, tag.normalized_name, now],
                )
                .map_err(CommandError::database)?;
            id
        };
        connection
            .execute(
                "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

pub(crate) fn load_tags_for_note(
    connection: &Connection,
    note_id: &str,
) -> Result<Vec<TagDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT t.id, t.name\n\
             FROM tags t\n\
             JOIN note_tags nt ON nt.tag_id = t.id\n\
             WHERE nt.note_id = ?1\n\
             ORDER BY t.normalized_name ASC, t.id ASC",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([note_id], |row| {
            Ok(TagDto {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub(crate) fn list_tag_summaries(
    connection: &Connection,
    deck_id: &str,
) -> Result<Vec<TagSummaryDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT t.id, t.name, COUNT(DISTINCT nt.note_id)\n\
             FROM tags t\n\
             JOIN note_tags nt ON nt.tag_id = t.id\n\
             JOIN notes n ON n.id = nt.note_id\n\
             JOIN cards c ON c.note_id = n.id\n\
             WHERE n.deck_id = ?1\n\
               AND n.deleted_at IS NULL\n\
               AND c.deleted_at IS NULL\n\
               AND c.direction = 'forward'\n\
             GROUP BY t.id, t.name, t.normalized_name\n\
             ORDER BY t.normalized_name ASC, t.id ASC",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([deck_id], |row| {
            Ok(TagSummaryDto {
                id: row.get(0)?,
                name: row.get(1)?,
                card_count: row.get(2)?,
            })
        })
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub(crate) fn remove_orphan_tags(connection: &Connection) -> Result<(), CommandError> {
    connection
        .execute(
            "DELETE FROM tags\n\
             WHERE NOT EXISTS (SELECT 1 FROM note_tags nt WHERE nt.tag_id = tags.id)",
            [],
        )
        .map_err(CommandError::database)?;
    Ok(())
}
