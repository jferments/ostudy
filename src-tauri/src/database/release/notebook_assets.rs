//! Additive release-v18 admission for notebook images.
//!
//! Migration 18 adds the table recording which managed asset each image on a
//! notebook page uses, so the asset store keeps the picture while it is on a
//! page. An upgrade writes no rows: no build before it placed pictures.

use std::sync::OnceLock;

use rusqlite::Connection;

use super::object_delta::{self, Delta};

const VERSION: i64 = 18;

pub(super) fn validate_schema(connection: &Connection, version: i64) -> Result<(), String> {
    static DELTA: OnceLock<Result<Delta, String>> = OnceLock::new();
    object_delta::validate(connection, version, VERSION, "notebook image", &DELTA)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::release::{
        apply_migrations, apply_migrations_through, migration_sql, preservation_receipt,
        subjects_fixture::release_library, validate_backup_input, validate_input,
        validate_supported_input,
    };

    fn schema_seventeen() -> Connection {
        let connection = release_library(17);
        assert_eq!(validate_supported_input(&connection).unwrap(), 17);
        connection
    }

    #[test]
    fn schema_seventeen_upgrades_additively_with_no_notebook_images() {
        let mut connection = schema_seventeen();
        let before = preservation_receipt(&connection).unwrap();
        assert!(apply_migrations_through(&mut connection, VERSION).unwrap());
        assert_eq!(validate_supported_input(&connection).unwrap(), VERSION);
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM notebook_item_assets", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 0);

        let comparison = connection.transaction().unwrap();
        comparison
            .execute_batch("DROP TABLE notebook_item_assets;")
            .unwrap();
        assert_eq!(preservation_receipt(&comparison).unwrap(), before);
        comparison.rollback().unwrap();
        validate_input(&connection).unwrap();
    }

    #[test]
    fn interrupted_eighteenth_migration_rolls_back_to_schema_seventeen_and_retries() {
        let mut connection = schema_seventeen();
        let before = preservation_receipt(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER reject_notebook_assets_migration
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version=18
                 BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
            )
            .unwrap();
        assert!(apply_migrations(&mut connection).is_err());
        assert_eq!(validate_supported_input(&connection).unwrap(), 17);
        assert_eq!(preservation_receipt(&connection).unwrap(), before);
        validate_schema(&connection, 17).unwrap();

        connection
            .execute_batch("DROP TRIGGER reject_notebook_assets_migration;")
            .unwrap();
        assert!(apply_migrations(&mut connection).unwrap());
        assert!(!apply_migrations(&mut connection).unwrap());
        validate_input(&connection).unwrap();
    }

    #[test]
    fn unregistered_missing_and_substituted_notebook_image_schemas_fail_read_only() {
        let mut connection = schema_seventeen();
        let transaction = connection.transaction().unwrap();
        transaction.execute_batch(migration_sql(VERSION)).unwrap();
        let changes = transaction.total_changes();
        assert!(validate_supported_input(&transaction).is_err());
        assert_eq!(transaction.total_changes(), changes);
        transaction.rollback().unwrap();

        apply_migrations(&mut connection).unwrap();
        for damage in [
            "DROP INDEX notebook_item_assets_by_asset;",
            // Without the cascade, deleting an image would leave its asset kept.
            "DROP TABLE notebook_item_assets;
             CREATE TABLE notebook_item_assets(
                 item_id TEXT PRIMARY KEY REFERENCES notebook_items(id),
                 asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT
             ) STRICT;
             CREATE INDEX notebook_item_assets_by_asset ON notebook_item_assets(asset_id);",
        ] {
            let transaction = connection.transaction().unwrap();
            transaction.execute_batch(damage).unwrap();
            let changes = transaction.total_changes();
            assert!(validate_backup_input(&transaction).is_err(), "{damage}");
            assert_eq!(transaction.total_changes(), changes);
            transaction.rollback().unwrap();
        }
        validate_input(&connection).unwrap();
    }
}
