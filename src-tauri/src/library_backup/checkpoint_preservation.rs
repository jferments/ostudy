//! Versioned preservation proof for forward release upgrades. This is not a
//! reverse migration and never changes either database. Legacy public inputs
//! retain the database owner's migration/repair and release-conversion proofs.

use std::collections::BTreeMap;

use rusqlite::{types::ValueRef, Connection};
use sha2::{Digest, Sha256};

use crate::database::{CURRENT_SCHEMA_LINEAGE, CURRENT_SCHEMA_VERSION};

type Receipt = BTreeMap<String, (Vec<String>, Vec<[u8; 32]>)>;

const NO_CONTRACT: &str = "No preservation contract is registered for this release upgrade. The active library was not replaced.";
const CHANGED: &str = "The private release upgrade changed retained library values. The active library was not replaced.";

/// One table that a release migration declares it changes.
enum Rule {
    /// The migration creates the table and writes no rows into it.
    AddedEmpty {
        table: &'static str,
        /// Names the table in user-facing errors.
        description: &'static str,
    },
}

/// What the migration to `version` may change. A table that none of the
/// applicable rules names must keep its column names and typed row values.
#[derive(Clone, Copy)]
struct Contract {
    version: i64,
    rules: &'static [Rule],
}

/// One contract for each forward release migration after the baseline.
const CONTRACTS: &[Contract] = &[
    Contract {
        version: 2,
        rules: &[Rule::AddedEmpty {
            table: "selection_deck_operations",
            description: "selection-deck receipt",
        }],
    },
    Contract {
        version: 3,
        rules: &[Rule::AddedEmpty {
            table: "deck_merge_operations",
            description: "deck-merge receipt",
        }],
    },
    Contract {
        version: 4,
        rules: &[Rule::AddedEmpty {
            table: "study_target_generation_instructions",
            description: "StudyTarget generation-instruction",
        }],
    },
    // Reviews recorded before schema 5 keep no scheduler-context row.
    Contract {
        version: 5,
        rules: &[Rule::AddedEmpty {
            table: "review_event_scheduler_context",
            description: "review scheduler-context",
        }],
    },
];

/// Learner evidence that no upgrade may rewrite, extend or remove, with the
/// schema whose migration created each table. `verify` compares these tables
/// by name before applying any contract, and a test keeps every contract from
/// listing them. The one exception is the release that creates a table:
/// `review_event_scheduler_context` is new to schema 5, so an upgrade from an
/// older source is governed by schema 5's rule that it arrives empty. From the
/// moment the source has the table, its rows are evidence like any other here.
const ALWAYS_IDENTICAL: &[(&str, i64)] = &[
    ("review_events", 1),
    ("review_event_attributions", 1),
    ("review_event_tag_attributions", 1),
    ("review_event_scheduler_context", 5),
    ("question_attempts", 1),
    ("question_review_links", 1),
    ("practice_session_cards", 1),
    ("practice_session_card_snapshots", 1),
    ("schedule_action_events", 1),
    ("study_target_card_origins", 1),
    ("study_targets", 1),
];

pub(super) fn verify(
    source: &Connection,
    target: &Connection,
    source_lineage: &str,
    source_version: i64,
) -> Result<(), String> {
    verify_with(CONTRACTS, source, target, source_lineage, source_version)
}

/// The registry is a parameter so tests can remove one release's contract.
fn verify_with(
    contracts: &[Contract],
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
        || !(1..CURRENT_SCHEMA_VERSION).contains(&source_version)
    {
        return Err(NO_CONTRACT.into());
    }
    // Every release after the source needs its own contract. A missing one
    // refuses the upgrade before any receipt is taken; there is no weaker
    // fallback comparison.
    let applicable = (source_version + 1..=CURRENT_SCHEMA_VERSION)
        .map(|version| {
            contracts
                .iter()
                .find(|contract| contract.version == version)
                .ok_or_else(|| NO_CONTRACT.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let before = rows(source)?;
    let mut after = rows(target)?;

    // Protected evidence is compared first, so no contract rule can excuse it.
    for (table, introduced_in) in ALWAYS_IDENTICAL {
        if source_version < *introduced_in {
            continue;
        }
        match (before.get(*table), after.get(*table)) {
            (Some(retained), Some(upgraded)) if retained == upgraded => {}
            _ => return Err(CHANGED.into()),
        }
    }

    // Contracts compose end to end: the source is compared with the final
    // schema, never with an intermediate one. Each table that a later release
    // added is removed under its rule; every remaining table must be identical.
    for rule in applicable.iter().flat_map(|contract| contract.rules) {
        match rule {
            Rule::AddedEmpty { table, description } => {
                let added = after.remove(*table).ok_or_else(|| {
                    format!("The upgraded library is missing its {description} table.")
                })?;
                if !added.1.is_empty() {
                    return Err(format!(
                        "The release upgrade unexpectedly created {description} rows. The active library was not replaced."
                    ));
                }
            }
        }
    }

    if before != after {
        return Err(CHANGED.into());
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

    #[test]
    fn every_release_after_the_source_needs_its_own_contract() {
        let versions: Vec<i64> = CONTRACTS.iter().map(|contract| contract.version).collect();
        assert_eq!(versions, (2..=CURRENT_SCHEMA_VERSION).collect::<Vec<_>>());
        // Past the gate, these empty databases fail the protected-evidence
        // comparison instead, so only the gate reports a missing contract.
        let empty = Connection::open_in_memory().unwrap();
        for missing in 2..=CURRENT_SCHEMA_VERSION {
            let contracts: Vec<Contract> = CONTRACTS
                .iter()
                .copied()
                .filter(|contract| contract.version != missing)
                .collect();
            for source_version in 1..CURRENT_SCHEMA_VERSION {
                let refused = verify_with(
                    &contracts,
                    &empty,
                    &empty,
                    CURRENT_SCHEMA_LINEAGE,
                    source_version,
                )
                .is_err_and(|error| error == NO_CONTRACT);
                assert_eq!(
                    refused,
                    source_version < missing,
                    "schema {source_version} source without the schema {missing} contract"
                );
            }
        }
    }

    #[test]
    fn no_contract_lists_protected_evidence_except_the_release_that_creates_it() {
        for contract in CONTRACTS {
            for rule in contract.rules {
                // A new kind of rule must say whether it creates its table.
                let (table, creates) = match rule {
                    Rule::AddedEmpty { table, .. } => (*table, true),
                };
                if let Some((_, introduced_in)) =
                    ALWAYS_IDENTICAL.iter().find(|(name, _)| *name == table)
                {
                    assert!(
                        creates && contract.version == *introduced_in,
                        "the schema {} contract lists protected table {table}",
                        contract.version
                    );
                }
            }
        }
    }
}
