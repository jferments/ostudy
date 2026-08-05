use std::collections::BTreeSet;

use rusqlite::types::Value as SqlValue;

use crate::{error::CommandError, util::validate_entity_id};

pub const ID_CHUNK_SIZE: usize = 250;
pub const MAX_SCOPE_IDS: usize = 10_000;

pub fn normalize_ids(
    values: Vec<String>,
    label: &str,
    maximum: usize,
) -> Result<Vec<String>, CommandError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, label)?;
        unique.insert(value);
    }
    if unique.len() > maximum.min(MAX_SCOPE_IDS) {
        return Err(CommandError::new(
            "too_many_scope_ids",
            format!("Select at most {} {label} values.", maximum.min(MAX_SCOPE_IDS)),
        ));
    }
    Ok(unique.into_iter().collect())
}

pub fn append_in_condition(
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
        .map(|id| {
            values.push(id.clone().into());
            format!("?{}", values.len())
        })
        .collect::<Vec<_>>()
        .join(", ");
    conditions.push(format!("{column} IN ({placeholders})"));
}

pub fn chunks(ids: &[String]) -> impl Iterator<Item = &[String]> {
    ids.chunks(ID_CHUNK_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_deduplicates_and_orders_ids() {
        let values = normalize_ids(
            vec![
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            ],
            "deck",
            10,
        )
        .expect("scope should normalize");
        assert_eq!(values.len(), 2);
        assert!(values[0] < values[1]);
    }

    #[test]
    fn chunks_never_depend_on_sqlite_variable_configuration() {
        let ids = (0..501)
            .map(|index| format!("{index:032x}"))
            .collect::<Vec<_>>();
        assert_eq!(chunks(&ids).map(<[String]>::len).collect::<Vec<_>>(), vec![250, 250, 1]);
    }
}
