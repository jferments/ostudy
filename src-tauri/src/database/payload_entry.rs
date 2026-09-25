//! Admit retained study payloads without rewriting them or guessing defaults.

use rusqlite::Connection;

use crate::cards::{questions::QuestionAttempt, questions::ValidatedQuestion, validate_document};

pub(super) fn validate(connection: &Connection, version: i64) -> Result<(), String> {
    let definition_column = if version >= 15 {
        "question_definition"
    } else {
        "NULL"
    };
    let mut statement = connection
        .prepare(&format!(
            "SELECT id, note_type, content_format, content_schema_version,
                    front_doc, back_doc, {definition_column} FROM notes"
        ))
        .map_err(|_| "The stored note contract is damaged.".to_string())?;
    let mut rows = statement
        .query([])
        .map_err(|_| "The stored notes could not be inspected.".to_string())?;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let id: String = row.get(0).map_err(|error| error.to_string())?;
        let kind: String = row.get(1).map_err(|error| error.to_string())?;
        let format: String = row.get(2).map_err(|error| error.to_string())?;
        let schema: i64 = row.get(3).map_err(|error| error.to_string())?;
        let front: Option<String> = row.get(4).map_err(|error| error.to_string())?;
        let back: Option<String> = row.get(5).map_err(|error| error.to_string())?;
        let definition: Option<String> = row.get(6).map_err(|error| error.to_string())?;
        let accepted = match (kind.as_str(), format.as_str(), schema) {
            ("basic" | "cloze", "tiptap-json", 1) => {
                definition.is_none() && rich(front.as_deref()) && rich(back.as_deref())
            }
            ("interactive", "question-json", 1) => {
                front.is_none()
                    && back.is_none()
                    && definition
                        .as_deref()
                        .is_some_and(|value| ValidatedQuestion::parse(value).is_ok())
            }
            _ => false,
        };
        if !accepted {
            return Err(format!(
                "Note {id} has unsupported or corrupt stored content. The database was not modified."
            ));
        }
    }
    if version >= 15 {
        validate_attempts(connection)?;
    }
    Ok(())
}

fn rich(value: Option<&str>) -> bool {
    value
        .and_then(|value| serde_json::from_str(value).ok())
        .is_some_and(|value| validate_document(&value).is_ok())
}

fn validate_attempts(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT instance_id, note_id, card_id, instance_hash, revision,
                    checkpoint_json FROM question_attempts",
        )
        .map_err(|_| "The stored attempt contract is damaged.".to_string())?;
    let mut rows = statement
        .query([])
        .map_err(|_| "The stored attempts could not be inspected.".to_string())?;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        let id: String = row.get(0).map_err(|error| error.to_string())?;
        let note: String = row.get(1).map_err(|error| error.to_string())?;
        let card: String = row.get(2).map_err(|error| error.to_string())?;
        let hash: String = row.get(3).map_err(|error| error.to_string())?;
        let revision: i64 = row.get(4).map_err(|error| error.to_string())?;
        let checkpoint: String = row.get(5).map_err(|error| error.to_string())?;
        let attempt = QuestionAttempt::restore(&checkpoint).map_err(|_| {
            format!("Attempt {id} has unsupported or corrupt stored content. The database was not modified.")
        })?;
        let identity = attempt.instance().identity();
        if identity.instance_id != id
            || identity.note_id != note
            || identity.card_id != card
            || attempt.instance().fingerprint() != hash
            || attempt.revision() != revision
        {
            return Err(format!(
                "Attempt {id} does not match its stored identity. The database was not modified."
            ));
        }
    }
    Ok(())
}
