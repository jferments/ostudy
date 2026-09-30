//! Versioned preservation proof for forward release upgrades. This is not a
//! reverse migration and never changes either database. Legacy public inputs
//! retain the database owner's migration/repair and release-conversion proofs.

use std::collections::BTreeMap;

use rusqlite::{types::ValueRef, Connection};
use sha2::{Digest, Sha256};

use crate::database::{CURRENT_SCHEMA_LINEAGE, CURRENT_SCHEMA_VERSION};

type Receipt = BTreeMap<String, (Vec<String>, Vec<[u8; 32]>)>;

pub(super) fn verify(
    source: &Connection,
    target: &Connection,
    source_lineage: &str,
    source_version: i64,
) -> Result<(), String> {
    if source_lineage == "ostudy-public-v1" {
        // Database::open already applied the immutable public migrations and
        // the staged typed-row proof at the public-to-release conversion owner.
        return Ok(());
    }
    if source_lineage != CURRENT_SCHEMA_LINEAGE
        || source_version != 1
        || CURRENT_SCHEMA_VERSION != 2
    {
        return Err("No preservation contract is registered for this release upgrade. The active library was not replaced.".into());
    }
    let before = rows(source)?;
    let mut after = rows(target)?;
    let receipt_table = after.remove("selection_deck_operations").ok_or_else(|| {
        "The upgraded library is missing its selection-deck receipt table.".to_string()
    })?;
    if !receipt_table.1.is_empty() || before != after {
        return Err("The private release upgrade changed retained library values. The active library was not replaced.".into());
    }
    Ok(())
}

/// Typed row hashes, column identities, multiplicity and the FTS-owned notes
/// rowid are preserved. Counts alone cannot detect changed learning content.
pub(super) fn rows(connection: &Connection) -> Result<Receipt, String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM pragma_table_list WHERE schema = 'main'
             AND type = 'table' AND name NOT LIKE 'sqlite_%'
             AND name NOT IN ('schema_migrations', 'data_repairs') ORDER BY name",
        )
        .map_err(sql_error)?;
    let tables = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    let mut receipt = BTreeMap::new();
    for table in tables {
        let identifier = table.replace('"', "\"\"");
        let columns = if table == "notes" { "rowid, *" } else { "*" };
        let mut statement = connection
            .prepare(&format!("SELECT {columns} FROM \"{identifier}\""))
            .map_err(sql_error)?;
        let names = statement
            .column_names()
            .into_iter()
            .map(str::to_string)
            .collect();
        let width = statement.column_count();
        let mut query = statement.query([]).map_err(sql_error)?;
        let mut hashes = Vec::new();
        while let Some(row) = query.next().map_err(sql_error)? {
            let mut digest = Sha256::new();
            digest.update(b"ostudy-upgrade-values-v1");
            digest.update((width as u64).to_be_bytes());
            for index in 0..width {
                match row.get_ref(index).map_err(sql_error)? {
                    ValueRef::Null => digest.update([0]),
                    ValueRef::Integer(value) => {
                        digest.update([1]);
                        digest.update(value.to_be_bytes());
                    }
                    ValueRef::Real(value) => {
                        digest.update([2]);
                        digest.update(value.to_bits().to_be_bytes());
                    }
                    ValueRef::Text(value) => {
                        digest.update([3]);
                        digest.update((value.len() as u64).to_be_bytes());
                        digest.update(value);
                    }
                    ValueRef::Blob(value) => {
                        digest.update([4]);
                        digest.update((value.len() as u64).to_be_bytes());
                        digest.update(value);
                    }
                }
            }
            hashes.push(<[u8; 32]>::from(digest.finalize()));
        }
        hashes.sort_unstable();
        receipt.insert(table, (names, hashes));
    }
    Ok(receipt)
}

fn sql_error(error: rusqlite::Error) -> String {
    format!("Could not prove retained library values: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_counts_do_not_hide_changed_values_types_identities_or_duplicates() {
        let source = Connection::open_in_memory().unwrap();
        source
            .execute_batch(
                "CREATE TABLE probe(id, value); INSERT INTO probe VALUES (1, '2'), (1, '2');",
            )
            .unwrap();
        let before = rows(&source).unwrap();
        for sql in [
            "UPDATE probe SET value = '3'",
            "UPDATE probe SET value = 2",
            "UPDATE probe SET id = 2",
            "DELETE FROM probe WHERE rowid = 2",
        ] {
            source.execute_batch("SAVEPOINT test_change").unwrap();
            source.execute_batch(sql).unwrap();
            assert_ne!(rows(&source).unwrap(), before);
            source
                .execute_batch("ROLLBACK TO test_change; RELEASE test_change")
                .unwrap();
        }
        assert_eq!(rows(&source).unwrap(), before);
    }
}
