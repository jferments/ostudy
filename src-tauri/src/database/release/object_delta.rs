//! Admission for a release migration that adds, changes or drops schema
//! objects beyond one table: every object the migration touches is compared
//! with the bundled schema at the stored version, so a library that
//! half-applied the migration, or applied a different one, is refused before
//! anything writes to it.

use std::{collections::BTreeMap, sync::OnceLock};

use rusqlite::{Connection, OptionalExtension};

use super::{normalize_schema_sql, BASELINE_SQL, RELEASE_MIGRATIONS};

/// (type, name) of a schema object.
type ObjectKey = (String, String);

/// Every object one migration adds, changes or drops, with its normalized SQL
/// before and after it. `None` means the object is absent.
pub(super) type Delta = Vec<(ObjectKey, Option<String>, Option<String>)>;

/// Checks the objects migration `migration` touches against a library stored
/// at `version`. `label` names the migration in errors; `cache` holds the
/// bundled delta, computed once.
pub(super) fn validate(
    connection: &Connection,
    version: i64,
    migration: i64,
    label: &str,
    cache: &'static OnceLock<Result<Delta, String>>,
) -> Result<(), String> {
    let delta = cache
        .get_or_init(|| bundled_delta(migration, label))
        .as_ref()
        .map_err(Clone::clone)?;
    for ((kind, name), before, after) in delta {
        let actual: Option<String> = connection
            .query_row(
                "SELECT sql FROM sqlite_schema WHERE type = ?1 AND name = ?2",
                [kind, name],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|error| format!("Could not inspect the {label} schema: {error}"))?
            .flatten()
            .map(|sql| normalize_schema_sql(&sql));
        if version < migration {
            if actual != *before {
                return Err(format!(
                    "An unregistered release-v{migration} {label} change is present. The database was not modified."
                ));
            }
        } else if actual != *after {
            return Err(format!(
                "The {label} schema does not match this build ({kind} {name}). The database was not modified."
            ));
        }
    }
    Ok(())
}

fn bundled_delta(migration: i64, label: &str) -> Result<Delta, String> {
    let before = bundled_objects(migration - 1, label)?;
    let after = bundled_objects(migration, label)?;
    let mut keys: Vec<&ObjectKey> = before.keys().chain(after.keys()).collect();
    keys.sort();
    keys.dedup();
    Ok(keys
        .into_iter()
        .filter(|key| before.get(*key) != after.get(*key))
        .map(|key| {
            (
                (*key).clone(),
                before.get(key).cloned(),
                after.get(key).cloned(),
            )
        })
        .collect())
}

/// Normalized SQL of every named schema object in a fresh bundled library at
/// `version`. SQLite's own automatic indexes have no SQL and are skipped.
fn bundled_objects(version: i64, label: &str) -> Result<BTreeMap<ObjectKey, String>, String> {
    let connection = Connection::open_in_memory()
        .map_err(|error| format!("Could not open the {label} schema verifier: {error}"))?;
    connection
        .execute_batch(BASELINE_SQL)
        .map_err(|error| format!("Could not load the {label} schema verifier: {error}"))?;
    for migration in RELEASE_MIGRATIONS.iter().filter(|m| m.version <= version) {
        connection
            .execute_batch(migration.sql)
            .map_err(|error| format!("Could not load the {label} schema verifier: {error}"))?;
    }
    let mut statement = connection
        .prepare(
            "SELECT type, name, sql FROM sqlite_schema
             WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'",
        )
        .map_err(|error| format!("Could not inspect the {label} schema verifier: {error}"))?;
    let objects = statement
        .query_map([], |row| {
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                normalize_schema_sql(&row.get::<_, String>(2)?),
            ))
        })
        .map_err(|error| format!("Could not read the {label} schema verifier: {error}"))?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(|error| format!("Could not decode the {label} schema verifier: {error}"))?;
    Ok(objects)
}
