use std::collections::BTreeSet;

use rusqlite::types::Value as SqlValue;

use crate::{error::CommandError, util::validate_entity_id};

pub const ID_CHUNK_SIZE: usize = 250;

pub fn normalize_entity_ids(
    values: Vec<String>,
    label: &str,
    maximum: usize,
) -> Result<Vec<String>, CommandError> {
    if values.len() > maximum {
        return Err(CommandError::new(
            "too_many_ids",
            format!("Choose at most {maximum} {label}s."),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, label)?;
        unique.insert(value);
    }
    Ok(unique.into_iter().collect())
}

pub fn push_value(values: &mut Vec<SqlValue>, value: impl Into<SqlValue>) -> usize {
    values.push(value.into());
    values.len()
}

pub fn append_in_filter(
    conditions: &mut Vec<String>,
    values: &mut Vec<SqlValue>,
    column: &str,
    ids: &[String],
) {
    if ids.is_empty() {
        return;
    }
    let placeholders = ids
        .iter()
        .map(|id| format!("?{}", push_value(values, id.clone())))
        .collect::<Vec<_>>()
        .join(", ");
    conditions.push(format!("{column} IN ({placeholders})"));
}

pub fn chunks(values: &[String]) -> impl Iterator<Item = &[String]> {
    values.chunks(ID_CHUNK_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_deduplicates_and_sorts_ids() {
        let first = "1".repeat(32);
        let second = "2".repeat(32);
        let values = normalize_entity_ids(vec![second.clone(), first.clone(), second], "card", 10)
            .expect("ids should validate");
        assert_eq!(values, vec![first, "2".repeat(32)]);
    }

    #[test]
    fn chunk_size_is_below_common_sqlite_variable_limits() {
        assert!(ID_CHUNK_SIZE <= 500);
    }
}
