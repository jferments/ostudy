//! Bounded scope validation and SQLite bind construction.

use std::collections::BTreeSet;

use rusqlite::types::Value as SqlValue;

use crate::{error::CommandError, util::validate_entity_id};

pub const ID_CHUNK_SIZE: usize = 250;
pub const MAX_DECK_SCOPE: usize = 256;
pub const MAX_TAG_SCOPE: usize = 64;
pub const MAX_EXACT_CARD_SCOPE: usize = 10_000;

pub fn validated_ids(
    values: Vec<String>,
    label: &str,
    maximum: usize,
) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, label)?;
        unique.insert(value);
    }
    if unique.len() > maximum {
        return Err(CommandError::new(
            "scope_too_large",
            format!("Select at most {maximum} {label} values."),
        ));
    }
    Ok(unique.into_iter().collect())
}

pub fn push_text(values: &mut Vec<SqlValue>, value: String) -> usize {
    values.push(SqlValue::Text(value));
    values.len()
}

pub fn push_integer(values: &mut Vec<SqlValue>, value: i64) -> usize {
    values.push(SqlValue::Integer(value));
    values.len()
}

pub fn placeholders(values: &mut Vec<SqlValue>, ids: impl IntoIterator<Item = String>) -> String {
    ids.into_iter()
        .map(|id| format!("?{}", push_text(values, id)))
        .collect::<Vec<_>>()
        .join(", ")
}
