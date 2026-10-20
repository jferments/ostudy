//! Versioned preservation proof for forward release upgrades. This is not a
//! reverse migration and never changes either database. Legacy public inputs
//! retain the database owner's migration/repair and release-conversion proofs.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{types::ValueRef, Connection, OpenFlags};
use sha2::{Digest, Sha256};

use crate::database::{CURRENT_SCHEMA_LINEAGE, CURRENT_SCHEMA_VERSION};

#[path = "checkpoint_preservation_subjects.rs"]
mod subjects;

type Receipt = BTreeMap<String, (Vec<String>, Vec<[u8; 32]>)>;

const NO_CONTRACT: &str = "No preservation contract is registered for this release upgrade. The active library was not replaced.";
const CHANGED: &str = "The private release upgrade changed retained library values. The active library was not replaced.";

/// One table that a release migration declares it changes. Proofs are SQL run
/// on a private connection that attaches the retained library as `source` and
/// the upgraded copy as `target`, both read-only. A proof returns the rows that
/// break its rule, so it must return none.
enum Rule {
    /// The migration creates the table and writes no rows into it.
    AddedEmpty {
        table: &'static str,
        /// Names the table in user-facing errors.
        description: &'static str,
    },
    /// The migration creates the table and fills it from retained rows.
    Added {
        table: &'static str,
        description: &'static str,
        /// Rows the migration could not have derived from the source.
        proof: &'static str,
    },
    /// The migration removes the table after carrying its rows elsewhere.
    Dropped {
        table: &'static str,
        description: &'static str,
        /// Source rows whose content did not arrive.
        proof: &'static str,
    },
    /// Rows keep their key and every retained value, except as declared.
    Projected(Projection),
}

/// A table whose rows the migration keeps by key and changes only where
/// declared. Every value outside the declaration must be identical, typed.
struct Projection {
    table: &'static str,
    description: &'static str,
    /// The columns that identify a row on both sides.
    key: &'static [&'static str],
    /// Columns only one side has: dropped from the source or added to the
    /// upgraded table. `proof` checks what an added column holds.
    ignored: &'static [&'static str],
    /// Retained columns whose values may differ, for rows `changed` selects.
    columns: &'static [&'static str],
    /// Keys of rows whose `columns` may differ. Empty when none may.
    changed: &'static str,
    /// Keys of rows the migration may add. Empty when it adds none.
    added: &'static str,
    /// Keys of rows the migration may remove. Empty when it removes none.
    removed: &'static str,
    /// Rows that break the table's rule. Empty when the key sets say it all.
    proof: &'static str,
}

/// What the migration to `version` may change. A table that none of the
/// applicable rules names must keep its column names and typed row values.
#[derive(Clone, Copy)]
struct Contract {
    version: i64,
    /// Temporary views the release's proofs share, created on the private
    /// proof connection before any proof runs.
    setup: &'static str,
    rules: &'static [Rule],
}

/// One contract for each forward release migration after the baseline.
const CONTRACTS: &[Contract] = &[
    Contract {
        version: 2,
        setup: "",
        rules: &[Rule::AddedEmpty {
            table: "selection_deck_operations",
            description: "selection-deck receipt",
        }],
    },
    Contract {
        version: 3,
        setup: "",
        rules: &[Rule::AddedEmpty {
            table: "deck_merge_operations",
            description: "deck-merge receipt",
        }],
    },
    Contract {
        version: 4,
        setup: "",
        rules: &[Rule::AddedEmpty {
            table: "study_target_generation_instructions",
            description: "StudyTarget generation-instruction",
        }],
    },
    // Reviews recorded before schema 5 keep no scheduler-context row.
    Contract {
        version: 5,
        setup: "",
        rules: &[Rule::AddedEmpty {
            table: "review_event_scheduler_context",
            description: "review scheduler-context",
        }],
    },
    // Subjects replace deck folders and collection memberships, and
    // collection copies retire into their originals through card_merges.
    Contract {
        version: 6,
        setup: subjects::SETUP,
        rules: subjects::RULES,
    },
    // The ledger fills only as later purges retain review totals, and every
    // deck's rollups are built on first use, so all three tables arrive empty.
    Contract {
        version: 7,
        setup: "",
        rules: &[
            Rule::AddedEmpty {
                table: "study_activity_retained",
                description: "study activity ledger",
            },
            Rule::AddedEmpty {
                table: "review_day_rollups",
                description: "review rollup",
            },
            Rule::AddedEmpty {
                table: "review_day_rollup_built_decks",
                description: "review rollup",
            },
        ],
    },
    // Learners write annotations; the upgrade only creates their table.
    Contract {
        version: 8,
        setup: "",
        rules: &[Rule::AddedEmpty {
            table: "document_annotations",
            description: "document annotation",
        }],
    },
    // Lessons are written in the app; the upgrade only creates their tables.
    Contract {
        version: 9,
        setup: "",
        rules: &[
            Rule::AddedEmpty {
                table: "lessons",
                description: "lesson",
            },
            Rule::AddedEmpty {
                table: "lesson_revisions",
                description: "lesson",
            },
            Rule::AddedEmpty {
                table: "lesson_revision_units",
                description: "lesson",
            },
            Rule::AddedEmpty {
                table: "lesson_progress",
                description: "lesson progress",
            },
        ],
    },
    // Cards are linked to marks in the app; the upgrade only creates the table.
    Contract {
        version: 10,
        setup: "",
        rules: &[Rule::AddedEmpty {
            table: "note_annotation_sources",
            description: "card mark link",
        }],
    },
    // Narration is made in the app; the upgrade only creates its tables.
    Contract {
        version: 11,
        setup: "",
        rules: &[
            Rule::AddedEmpty {
                table: "speech_settings",
                description: "speech setting",
            },
            Rule::AddedEmpty {
                table: "speech_segments",
                description: "lesson narration",
            },
        ],
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
    ("study_activity_retained", 7),
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
    let mut before = rows(source)?;
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

    // Only a release that transforms rows needs the two databases side by side.
    let needs_proofs = applicable
        .iter()
        .flat_map(|contract| contract.rules)
        .any(|rule| !matches!(rule, Rule::AddedEmpty { .. }));
    let proofs = if needs_proofs {
        let proofs = ProofConnection::attach(source, target)?;
        for contract in &applicable {
            proofs.setup(contract.setup)?;
        }
        Some(proofs)
    } else {
        None
    };

    // Contracts compose end to end: the source is compared with the final
    // schema, never with an intermediate one. Each table that a release
    // declares is removed under its rule; every remaining table must be
    // identical.
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
            Rule::Added {
                table,
                description,
                proof,
            } => {
                let proofs = proofs
                    .as_ref()
                    .expect("transforming releases attach proofs");
                if before.contains_key(*table) || after.remove(*table).is_none() {
                    return Err(format!(
                        "The upgraded library is missing its {description} table."
                    ));
                }
                proofs.require_none(proof, description)?;
            }
            Rule::Dropped {
                table,
                description,
                proof,
            } => {
                let proofs = proofs
                    .as_ref()
                    .expect("transforming releases attach proofs");
                if before.remove(*table).is_none() || after.contains_key(*table) {
                    return Err(CHANGED.into());
                }
                proofs.require_none(proof, description)?;
            }
            Rule::Projected(projection) => {
                let proofs = proofs
                    .as_ref()
                    .expect("transforming releases attach proofs");
                let (Some(retained), Some(upgraded)) = (
                    before.remove(projection.table),
                    after.remove(projection.table),
                ) else {
                    return Err(CHANGED.into());
                };
                // An identical receipt needs no row-by-row comparison.
                if retained != upgraded {
                    proofs.compare(projection)?;
                }
                proofs.require_none(projection.proof, projection.description)?;
            }
        }
    }

    if before != after {
        return Err(CHANGED.into());
    }
    Ok(())
}

/// A private in-memory connection with the retained library attached as
/// `source` and the upgraded copy as `target`, both read-only. Dropping it
/// detaches both; neither database is written.
struct ProofConnection(Connection);

/// How `row_hashes` reads one projected table on either side.
struct RowShape<'a> {
    table: &'a str,
    /// Retained columns, in table order.
    columns: &'a [String],
    /// Positions of the key columns in `columns`.
    key: &'a [usize],
    /// Positions of the columns that may differ for `changed` rows.
    declared: &'a [usize],
    changed: &'a BTreeSet<Vec<u8>>,
}

impl ProofConnection {
    fn attach(source: &Connection, target: &Connection) -> Result<Self, String> {
        let connection = Connection::open_with_flags(
            ":memory:",
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(sql_error)?;
        for (database, schema) in [(source, "source"), (target, "target")] {
            let path = database
                .path()
                .filter(|path| !path.is_empty())
                .ok_or_else(|| {
                    "Could not prove retained library values: the library is not stored in a file."
                        .to_string()
                })?;
            connection
                .execute(
                    &format!("ATTACH DATABASE ?1 AS {schema}"),
                    [read_only_uri(path)],
                )
                .map_err(sql_error)?;
        }
        Ok(Self(connection))
    }

    fn setup(&self, sql: &str) -> Result<(), String> {
        if sql.is_empty() {
            return Ok(());
        }
        self.0.execute_batch(sql).map_err(sql_error)
    }

    /// Run a proof. Any row it returns is a change the release cannot account for.
    fn require_none(&self, proof: &str, description: &str) -> Result<(), String> {
        if proof.is_empty() {
            return Ok(());
        }
        let mut statement = self.0.prepare(proof).map_err(sql_error)?;
        if statement
            .query([])
            .map_err(sql_error)?
            .next()
            .map_err(sql_error)?
            .is_some()
        {
            return Err(format!(
                "The release upgrade could not account for its changes to {description}. The active library was not replaced."
            ));
        }
        Ok(())
    }

    /// Compare a projected table. Each row is hashed whole or, when the
    /// projection lets that row change, without its declared columns. Rows the
    /// projection may add (upgraded side) or remove (retained side) are set
    /// aside, and what remains must match row for row. One hash per row is
    /// held, as for a receipt.
    fn compare(&self, projection: &Projection) -> Result<(), String> {
        let retained = self.retained_columns(projection)?;
        let position = |column: &str| {
            retained
                .iter()
                .position(|name| name == column)
                .ok_or_else(|| CHANGED.to_string())
        };
        let key = projection
            .key
            .iter()
            .map(|column| position(column))
            .collect::<Result<Vec<_>, _>>()?;
        let declared = projection
            .columns
            .iter()
            .map(|column| position(column))
            .collect::<Result<Vec<_>, _>>()?;
        let shape = RowShape {
            table: projection.table,
            columns: &retained,
            key: &key,
            declared: &declared,
            changed: &self.keys(projection.changed, key.len())?,
        };
        let removed = self.keys(projection.removed, key.len())?;
        let added = self.keys(projection.added, key.len())?;
        if self.row_hashes("source", &shape, &removed)?
            != self.row_hashes("target", &shape, &added)?
        {
            return Err(CHANGED.into());
        }
        Ok(())
    }

    /// Columns both sides keep, in table order. Any other difference in the
    /// two column lists is an undeclared schema change.
    fn retained_columns(&self, projection: &Projection) -> Result<Vec<String>, String> {
        let mut sides = Vec::new();
        for schema in ["source", "target"] {
            let mut statement = self
                .0
                .prepare("SELECT name FROM pragma_table_info(?1, ?2) ORDER BY cid")
                .map_err(sql_error)?;
            let mut columns: Vec<String> = statement
                .query_map([projection.table, schema], |row| row.get(0))
                .map_err(sql_error)?
                .collect::<Result<Vec<String>, _>>()
                .map_err(sql_error)?
                .into_iter()
                .filter(|column| !projection.ignored.contains(&column.as_str()))
                .collect();
            if projection.table == "notes" {
                // The FTS index addresses notes by rowid.
                columns.insert(0, "rowid".into());
            }
            sides.push(columns);
        }
        let target = sides.pop().expect("two sides");
        let source = sides.pop().expect("two sides");
        if source != target || source.is_empty() {
            return Err(CHANGED.into());
        }
        Ok(source)
    }

    /// The sorted row hashes of `schema.table`, leaving out rows whose key is
    /// in `set_aside`.
    fn row_hashes(
        &self,
        schema: &str,
        shape: &RowShape<'_>,
        set_aside: &BTreeSet<Vec<u8>>,
    ) -> Result<Vec<[u8; 32]>, String> {
        let list = shape
            .columns
            .iter()
            .map(|column| format!("\"{}\"", column.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(", ");
        let mut statement = self
            .0
            .prepare(&format!(
                "SELECT {list} FROM {schema}.\"{}\"",
                shape.table.replace('"', "\"\"")
            ))
            .map_err(sql_error)?;
        let mut query = statement.query([]).map_err(sql_error)?;
        let mut hashes = Vec::new();
        while let Some(row) = query.next().map_err(sql_error)? {
            let mut identity = Vec::with_capacity(32 * shape.key.len());
            for index in shape.key {
                let mut digest = Sha256::new();
                hash_value(&mut digest, row.get_ref(*index).map_err(sql_error)?);
                identity.extend_from_slice(&digest.finalize());
            }
            if set_aside.contains(&identity) {
                continue;
            }
            let changed = shape.changed.contains(&identity);
            let mut digest = Sha256::new();
            digest.update(if changed {
                b"ostudy-upgrade-changed-row-v1".as_slice()
            } else {
                b"ostudy-upgrade-kept-row-v1".as_slice()
            });
            for index in 0..shape.columns.len() {
                if changed && shape.declared.contains(&index) {
                    continue;
                }
                hash_value(&mut digest, row.get_ref(index).map_err(sql_error)?);
            }
            hashes.push(<[u8; 32]>::from(digest.finalize()));
        }
        hashes.sort_unstable();
        Ok(hashes)
    }

    /// The keys a projection's SQL selects, encoded like `row_hashes` keys.
    fn keys(&self, sql: &str, width: usize) -> Result<BTreeSet<Vec<u8>>, String> {
        let mut keys = BTreeSet::new();
        if sql.is_empty() {
            return Ok(keys);
        }
        let mut statement = self.0.prepare(sql).map_err(sql_error)?;
        if statement.column_count() != width {
            return Err(format!(
                "Could not prove retained library values: a key query returns {} columns, not {width}.",
                statement.column_count()
            ));
        }
        let mut query = statement.query([]).map_err(sql_error)?;
        while let Some(row) = query.next().map_err(sql_error)? {
            let mut identity = Vec::new();
            for index in 0..width {
                let mut digest = Sha256::new();
                hash_value(&mut digest, row.get_ref(index).map_err(sql_error)?);
                identity.extend_from_slice(&digest.finalize());
            }
            keys.insert(identity);
        }
        Ok(keys)
    }
}

/// A read-only SQLite URI for `path`, percent-encoding everything but
/// unreserved characters and separators.
fn read_only_uri(path: &str) -> String {
    let mut uri = String::from("file:");
    let path = path.replace('\\', "/");
    if !path.starts_with('/') {
        // A Windows drive path becomes file:/C:/...
        uri.push('/');
    }
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/:".contains(&byte) {
            uri.push(byte as char);
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri.push_str("?mode=ro");
    uri
}

fn hash_value(digest: &mut Sha256, value: ValueRef<'_>) {
    match value {
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
                hash_value(&mut digest, row.get_ref(index).map_err(sql_error)?);
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
                    Rule::AddedEmpty { table, .. } | Rule::Added { table, .. } => (*table, true),
                    Rule::Dropped { table, .. } => (*table, false),
                    Rule::Projected(projection) => (projection.table, false),
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

    #[test]
    fn each_contract_names_a_table_at_most_once() {
        for contract in CONTRACTS {
            let mut seen = BTreeSet::new();
            for rule in contract.rules {
                let table = match rule {
                    Rule::AddedEmpty { table, .. }
                    | Rule::Added { table, .. }
                    | Rule::Dropped { table, .. } => *table,
                    Rule::Projected(projection) => projection.table,
                };
                assert!(
                    seen.insert(table),
                    "the schema {} contract names {table} twice",
                    contract.version
                );
            }
        }
    }

    #[test]
    fn proofs_need_both_libraries_in_files_and_cannot_write_them() {
        let memory = Connection::open_in_memory().unwrap();
        let error = ProofConnection::attach(&memory, &memory)
            .err()
            .expect("an in-memory library cannot be attached read-only");
        assert!(error.contains("not stored in a file"), "{error}");

        let directory = std::env::temp_dir().join(format!(
            "ostudy-preservation-proof-{}-{}",
            std::process::id(),
            crate::util::unix_time_millis().unwrap()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("library #1.sqlite3");
        let file = Connection::open(&path).unwrap();
        file.execute_batch(
            "CREATE TABLE probe(id INTEGER PRIMARY KEY); INSERT INTO probe VALUES (1);",
        )
        .unwrap();
        let proofs = ProofConnection::attach(&file, &file).unwrap();
        for schema in ["source", "target"] {
            assert!(proofs
                .0
                .execute_batch(&format!("INSERT INTO {schema}.probe VALUES (2)"))
                .is_err());
        }
        proofs
            .require_none("SELECT id FROM source.probe WHERE id <> 1", "probe")
            .unwrap();
        let refusal = proofs
            .require_none("SELECT id FROM target.probe", "probe rows")
            .unwrap_err();
        assert!(refusal.contains("changes to probe rows"), "{refusal}");
        drop(proofs);
        drop(file);
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn read_only_uris_escape_reserved_characters() {
        assert_eq!(
            read_only_uri("/data/o Study/#1?/library.sqlite3"),
            "file:/data/o%20Study/%231%3F/library.sqlite3?mode=ro"
        );
        assert_eq!(
            read_only_uri(r"C:\Users\learner\library.sqlite3"),
            "file:/C:/Users/learner/library.sqlite3?mode=ro"
        );
    }
}
