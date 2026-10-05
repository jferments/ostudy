//! A backup made by a schema-5 build restores into this build. The restore
//! upgrades the backup's private copy with the release migrations, which file
//! and merge the library exactly as upgrading it in place does.

use rusqlite::types::Value;
use sha2::{Digest, Sha256};

use super::*;
use crate::database::subjects_fixture::{advance_release_library, release_library, seed};
use crate::database::CURRENT_SCHEMA_VERSION;

/// Rows the subjects upgrade decides or must keep.
const UPGRADED: &[&str] = &[
    "SELECT * FROM document_collections ORDER BY id",
    "SELECT id, subject_id, archived_at, deleted_at, revision FROM decks ORDER BY id",
    "SELECT id, subject_id FROM documents ORDER BY id",
    "SELECT * FROM document_study_ranges ORDER BY id",
    "SELECT id, deck_id, deleted_at, revision FROM notes ORDER BY id",
    "SELECT id, deck_id, deleted_at FROM cards ORDER BY id",
    "SELECT * FROM card_schedules ORDER BY card_id",
    "SELECT * FROM card_merges ORDER BY retired_card_id",
    "SELECT * FROM subject_filing_reviews ORDER BY entity_kind, entity_id",
    "SELECT * FROM review_events ORDER BY id",
    "SELECT * FROM library_organization",
    "SELECT * FROM document_collection_organization",
];

fn rows(connection: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut statement = connection
        .prepare(sql)
        .unwrap_or_else(|error| panic!("{error}: {sql}"));
    let width = statement.column_count();
    let rows = statement
        .query_map([], |row| {
            (0..width)
                .map(|index| row.get::<_, Value>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    rows
}

fn pairs(connection: &Connection, sql: &str) -> Vec<(String, String)> {
    let mut statement = connection.prepare(sql).unwrap();
    let pairs = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    pairs
}

#[test]
fn a_schema_five_backup_restores_as_the_subjects_library() {
    let source_paths = temp_paths("subjects-v5-source");
    let schema_five = release_library(5);
    let library = seed(&schema_five);
    // A schema-5 build keeps a source file for every document, and its backup
    // carries them.
    let mut documents = Vec::new();
    for (id, _) in pairs(&schema_five, "SELECT id, title FROM documents ORDER BY id") {
        let relative = AppPaths::document_source_relative_path(&id);
        let bytes = format!("%PDF-1.4\n% {id}\n").into_bytes();
        let path = source_paths.resolve_relative(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &bytes).unwrap();
        schema_five
            .execute(
                "UPDATE documents SET source_relative_path = ?1, byte_size = ?2, sha256 = ?3
                 WHERE id = ?4",
                params![
                    relative,
                    bytes.len() as i64,
                    format!("{:x}", Sha256::digest(&bytes)),
                    id
                ],
            )
            .unwrap();
        documents.push((id, relative));
    }
    let snapshot_path = source_paths.app_data_dir().join("schema-5.sqlite3");
    schema_five
        .execute("VACUUM INTO ?1", [snapshot_path.to_str().unwrap()])
        .unwrap();
    let upgraded_in_place = schema_five;
    advance_release_library(&upgraded_in_place, 5, CURRENT_SCHEMA_VERSION);

    // The package format is unchanged since schema 5, so a schema-5 backup is
    // a current package around the schema-5 snapshot and its document files.
    let current = source_paths.app_data_dir().join("current.ostudybackup");
    export_backup(
        &Database::open(source_paths.database_path()).unwrap(),
        &source_paths,
        &current,
    )
    .unwrap();
    let backup = source_paths.app_data_dir().join("schema-5.ostudybackup");
    copy_and_sync(&current, &backup).unwrap();
    {
        let mut package = Connection::open(&backup).unwrap();
        let snapshot =
            Connection::open_with_flags(&snapshot_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let transaction = package.transaction().unwrap();
        transaction
            .execute(
                "UPDATE database_snapshot SET data = ?1 WHERE id = 1",
                [fs::read(&snapshot_path).unwrap()],
            )
            .unwrap();
        transaction
            .execute(
                "UPDATE backup_metadata SET value = '5' WHERE key = 'schemaVersion'",
                [],
            )
            .unwrap();
        let files =
            crate::document_backup::export_document_files(&snapshot, &source_paths, &transaction)
                .unwrap();
        assert_eq!(files, documents.len() as i64);
        transaction
            .execute(
                "UPDATE backup_metadata SET value = ?1 WHERE key = 'documentFileCount'",
                [files.to_string()],
            )
            .unwrap();
        transaction.commit().unwrap();
    }
    let summary = validate_backup_package(&backup, None).unwrap();
    assert_eq!(summary.review_count, library.reviews.len() as i64);

    let target_paths = temp_paths("subjects-v5-target");
    copy_and_sync(&backup, &pending_restore_path(&target_paths)).unwrap();
    let guard = apply_pending_restore(&target_paths)
        .expect("the schema-5 backup restores")
        .expect("a pending restore");
    let restored = Database::open(target_paths.database_path()).expect("restored library");
    restored
        .with_connection(|connection| {
            assert_eq!(
                crate::database::schema_identity(connection)?.1,
                CURRENT_SCHEMA_VERSION
            );
            for sql in UPGRADED {
                assert_eq!(
                    rows(connection, sql),
                    rows(&upgraded_in_place, sql),
                    "{sql}"
                );
            }
            assert_eq!(
                pairs(
                    connection,
                    "SELECT retired_card_id, surviving_card_id FROM card_merges
                     ORDER BY retired_card_id"
                ),
                library.expected_merges()
            );
            Ok(())
        })
        .unwrap();
    for (id, relative) in &documents {
        assert_eq!(
            fs::read(target_paths.resolve_relative(relative)).unwrap(),
            format!("%PDF-1.4\n% {id}\n").into_bytes()
        );
    }
    drop(restored);
    guard.commit();
    let _ = fs::remove_dir_all(source_paths.app_data_dir());
    let _ = fs::remove_dir_all(target_paths.app_data_dir());
}
