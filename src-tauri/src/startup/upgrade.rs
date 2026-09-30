//! Read-only admission for a whole-library upgrade. Startup owns the process
//! library lock before calling this boundary. Migration SQL and compatibility
//! policy remain native database contracts; this module never edits a ledger.

use std::{fs, path::Path};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

use crate::database::{self, CURRENT_SCHEMA_LINEAGE, CURRENT_SCHEMA_VERSION};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpgradePlan {
    pub source_lineage: String,
    pub source_version: i64,
    pub target_lineage: String,
    pub target_version: i64,
}

/// None means normal initialization/open, not that a current database has
/// already passed payload validation. Database::open still owns that admission.
/// Keeping the current-header fast path avoids a second full payload scan.
pub(super) fn inspect(path: &Path) -> Result<Option<UpgradePlan>, String> {
    let mut has_sidecar = false;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        match fs::symlink_metadata(Path::new(&name)) {
            Ok(metadata) if metadata.file_type().is_file() => has_sidecar = true,
            Ok(_) => return Err("Library journals must be regular files, not symbolic links.".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect library journals: {error}")),
        }
    }
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !has_sidecar => {
            return Ok(None);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("The library database is missing but its journal exists. Preserve the recovery files.".into());
        }
        Err(error) => return Err(format!("Could not inspect the library: {error}")),
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err("A library database must be a regular file, not a symbolic link.".into());
        }
        Ok(_) => {}
    }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("Could not inspect the library read-only: {error}"))?;
    inspect_connection(&connection)
}

fn inspect_connection(connection: &Connection) -> Result<Option<UpgradePlan>, String> {
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let has_metadata: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_metadata')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if !has_metadata {
        let objects: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if objects == 0 {
            return if version == 0 {
                Ok(None)
            } else {
                Err("An empty library has an unsupported schema version.".into())
            };
        }
        return Err(format!(
            "{}: This library requires explicit pre-public recovery. No upgrade was attempted.",
            database::PRE_PUBLIC_RESET_PREFIX
        ));
    }
    let lineage: Option<String> = connection
        .query_row(
            "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if lineage.as_deref() == Some(CURRENT_SCHEMA_LINEAGE) && version == CURRENT_SCHEMA_VERSION {
        return Ok(None);
    }
    // This validates the exact supported prefix and repairs, not just a numeric
    // version. Unknown, future and relabelled inputs cannot become candidates.
    let (source_lineage, source_version) =
        database::schema_identity(connection).map_err(|error| error.message)?;
    verify_integrity(connection)?;
    Ok(Some(UpgradePlan {
        source_lineage,
        source_version,
        target_lineage: CURRENT_SCHEMA_LINEAGE.to_string(),
        target_version: CURRENT_SCHEMA_VERSION,
    }))
}

fn verify_integrity(connection: &Connection) -> Result<(), String> {
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("Could not verify the pre-upgrade library: {error}"))?;
    if integrity != "ok" {
        return Err("The pre-upgrade library failed SQLite integrity validation.".into());
    }
    let has_broken_reference: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not verify the pre-upgrade relationships: {error}"))?;
    if has_broken_reference {
        return Err("The pre-upgrade library contains an invalid relationship.".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "upgrade_tests.rs"]
mod tests;
