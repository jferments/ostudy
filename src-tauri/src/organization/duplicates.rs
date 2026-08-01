use std::collections::HashSet;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{database::Database, error::CommandError, util::validate_entity_id};

const MAX_DUPLICATE_MATCHES: usize = 25;
const MAX_DUPLICATE_TEXT_CHARACTERS: usize = 200_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckCardDuplicatesInput {
    pub front_text: String,
    pub back_text: String,
    pub exclude_note_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateMatchDto {
    pub note_id: String,
    pub deck_id: String,
    pub deck_name: String,
    pub front_text: String,
    pub back_text: String,
}

#[tauri::command]
pub fn check_card_duplicates(
    database: State<'_, Database>,
    input: CheckCardDuplicatesInput,
) -> Result<Vec<DuplicateMatchDto>, CommandError> {
    validate_duplicate_text(&input.front_text, "question")?;
    validate_duplicate_text(&input.back_text, "answer")?;
    if let Some(note_id) = &input.exclude_note_id {
        validate_entity_id(note_id, "note")?;
    }
    database.with_connection(|connection| {
        find_duplicate_matches(
            connection,
            &input.front_text,
            &input.back_text,
            input.exclude_note_id.as_deref(),
        )
    })
}

fn validate_duplicate_text(value: &str, label: &str) -> Result<(), CommandError> {
    if value.chars().count() > MAX_DUPLICATE_TEXT_CHARACTERS {
        return Err(CommandError::new(
            "card_text_too_large",
            format!("The {label} text is too large to compare."),
        ));
    }
    Ok(())
}

pub(crate) fn duplicate_key(front_text: &str, back_text: &str) -> String {
    format!(
        "{}\u{1f}{}",
        front_text.trim().to_lowercase(),
        back_text.trim().to_lowercase()
    )
}

pub(crate) fn load_duplicate_keys_for_deck(
    connection: &Connection,
    deck_id: &str,
) -> Result<HashSet<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT n.front_text, n.back_text\n\
             FROM notes n\n\
             JOIN cards c ON c.note_id = n.id\n\
             WHERE n.deck_id = ?1\n\
               AND n.deleted_at IS NULL\n\
               AND c.deleted_at IS NULL\n\
               AND c.direction = 'forward'",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([deck_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
            ))
        })
        .map_err(CommandError::database)?;
    let mut keys = HashSet::new();
    for row in rows {
        let (front, back) = row.map_err(CommandError::database)?;
        keys.insert(duplicate_key(&front, &back));
    }
    Ok(keys)
}

pub(crate) fn find_duplicate_matches(
    connection: &Connection,
    front_text: &str,
    back_text: &str,
    exclude_note_id: Option<&str>,
) -> Result<Vec<DuplicateMatchDto>, CommandError> {
    let wanted = duplicate_key(front_text, back_text);
    if front_text.trim().is_empty() || back_text.trim().is_empty() {
        return Ok(Vec::new());
    }

    // SQLite's built-in lower() is ASCII-oriented. Length-prefilter candidates
    // in SQL, then apply Rust's Unicode-aware lowercase comparison so duplicate
    // warnings remain correct for non-English cards without scanning large text.
    let front_length = i64::try_from(front_text.trim().chars().count()).unwrap_or(i64::MAX);
    let back_length = i64::try_from(back_text.trim().chars().count()).unwrap_or(i64::MAX);
    let mut statement = connection
        .prepare(
            "SELECT n.id, n.deck_id, d.name, n.front_text, n.back_text\n\
             FROM notes n\n\
             JOIN decks d ON d.id = n.deck_id\n\
             JOIN cards c ON c.note_id = n.id\n\
             WHERE n.deleted_at IS NULL\n\
               AND d.deleted_at IS NULL\n\
               AND d.archived_at IS NULL\n\
               AND c.deleted_at IS NULL\n\
               AND c.direction = 'forward'\n\
               AND length(trim(n.front_text)) = ?1\n\
               AND length(trim(n.back_text)) = ?2\n\
             ORDER BY n.updated_at DESC, n.id ASC",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([front_length, back_length], |row| {
            Ok(DuplicateMatchDto {
                note_id: row.get(0)?,
                deck_id: row.get(1)?,
                deck_name: row.get(2)?,
                front_text: row.get(3)?,
                back_text: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?;

    let mut matches = Vec::new();
    for row in rows {
        let candidate = row.map_err(CommandError::database)?;
        if exclude_note_id == Some(candidate.note_id.as_str()) {
            continue;
        }
        if duplicate_key(&candidate.front_text, &candidate.back_text) == wanted {
            matches.push(candidate);
            if matches.len() >= MAX_DUPLICATE_MATCHES {
                break;
            }
        }
    }
    Ok(matches)
}
