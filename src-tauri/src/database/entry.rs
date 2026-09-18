//! Read-only admission before any migration, repair, or journal-mode change.
//! The migration chain is the converter; feature code only receives Database
//! after that chain and the required repairs have reached the current contract.

use rusqlite::Connection;

#[path = "payload_entry.rs"]
mod payload_entry;

use super::{verify_public_ledger_exists, LATEST_SCHEMA_VERSION, MIGRATIONS};

pub(super) fn user_version(connection: &Connection) -> Result<i64, String> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| format!("Could not read SQLite user_version: {error}"))
}

/// Shared by startup and read-only backup admission. A missing suffix is a
/// supported older input; a hole, altered checksum, or future row is not.
pub(super) fn validate_public_input(connection: &Connection) -> Result<i64, String> {
    verify_public_ledger_exists(connection)?;
    let mut statement = connection
        .prepare("SELECT version, name, checksum FROM schema_migrations ORDER BY version")
        .map_err(|error| format!("Could not inspect the migration ledger: {error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("Could not read the migration ledger: {error}"))?;
    let mut count = 0;
    let mut version = 0;
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("Could not read a migration ledger row: {error}"))?
    {
        let recorded: (i64, String, String) = (
            row.get(0)
                .map_err(|error| format!("Invalid migration version: {error}"))?,
            row.get(1)
                .map_err(|error| format!("Invalid migration name: {error}"))?,
            row.get(2)
                .map_err(|error| format!("Invalid migration checksum: {error}"))?,
        );
        if recorded.0 > LATEST_SCHEMA_VERSION {
            return Err(format!(
                "This database was created by a newer oStudy version (public schema {}). The database was not modified.",
                recorded.0
            ));
        }
        let expected = MIGRATIONS.get(count).ok_or_else(|| {
            "The migration ledger has unexpected rows. The database was not modified.".to_string()
        })?;
        if recorded.0 != expected.version
            || recorded.1 != expected.name
            || recorded.2 != expected.checksum
        {
            return Err(format!(
                "The migration ledger is not an exact supported prefix at version {}. The database was not modified.",
                expected.version
            ));
        }
        count += 1;
        version = recorded.0;
    }
    if count == 0 || user_version(connection)? != version {
        return Err(
            "SQLite user_version and the migration ledger disagree or are empty. The database was not modified."
                .to_string(),
        );
    }
    // Do this before applying even an earlier missing repair. Otherwise a later
    // unknown repair could be discovered only after the library has changed.
    crate::data_repairs::verify_repair_ledger(connection)?;
    payload_entry::validate(connection, version)?;
    Ok(version)
}
