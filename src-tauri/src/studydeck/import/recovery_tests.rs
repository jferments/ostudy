use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    database: Database,
    paths: AppPaths,
    operation: String,
    draft: String,
    asset: String,
    staged: PathBuf,
    published: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-import-recovery-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = AppPaths::new(root).unwrap();
        let database = Database::open_in_memory().unwrap();
        let operation = "1".repeat(32);
        let (draft, asset, staged, published) = database
            .with_connection(|connection| {
                prepare_operation(connection, &paths, &operation, &"f".repeat(64), 3)?;
                let draft = create_import_draft(connection, &operation)?;
                let asset = generate_id(connection)?;
                let relative = AppPaths::image_relative_path(&asset, "png");
                let staged_relative = AppPaths::staged_asset_relative_path(&draft, &asset, "png");
                let staged = paths.resolve_relative(&staged_relative);
                let published = paths.resolve_relative(&relative);
                write_new_file(&staged, b"retained import bytes")?;
                connection
                    .execute(
                        "INSERT INTO assets(id,original_filename,mime_type,byte_size,relative_path,
                            created_at,lifecycle_state,draft_id,staged_relative_path,updated_at)
                         VALUES(?1,'fixture.png','image/png',21,?2,1,'staged',?3,?4,1)",
                        params![asset, relative, draft, staged_relative],
                    )
                    .map_err(CommandError::database)?;
                Ok((draft, asset, staged, published))
            })
            .unwrap();
        Self {
            database,
            paths,
            operation,
            draft,
            asset,
            staged,
            published,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.paths.app_data_dir());
    }
}

#[test]
fn secondary_cleanup_failure_preserves_primary_error_and_recovery_inputs() {
    let fixture = Fixture::new();
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TEMP TRIGGER reject_import_cleanup BEFORE DELETE ON assets
                     BEGIN SELECT RAISE(ABORT, 'injected cleanup failure'); END;",
                )
                .map_err(CommandError::database)?;
            let primary = record_failed_operation(
                connection,
                &fixture.paths,
                &fixture.operation,
                &fixture.draft,
                CommandError::new("original_import_failure", "The original import failed."),
                false,
            );
            assert_eq!(primary.code, "original_import_failure");
            assert_eq!(primary.message, "The original import failed.");
            let rows: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM assets WHERE id=?1",
                    [&fixture.asset],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(rows, 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(fs::read(&fixture.staged).unwrap(), b"retained import bytes");
}

#[test]
fn pending_recovery_is_recorded_without_deleting_assets_or_canceling_the_draft() {
    let fixture = Fixture::new();
    fixture
        .database
        .with_connection(|connection| {
            let error = record_failed_operation(
                connection,
                &fixture.paths,
                &fixture.operation,
                &fixture.draft,
                CommandError::new("original_import_failure", "Original failure."),
                true,
            );
            assert_eq!(error.code, "original_import_failure");
            let canceled: Option<i64> = connection
                .query_row(
                    "SELECT canceled_at FROM asset_drafts WHERE id=?1",
                    [&fixture.draft],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let status: String = connection
                .query_row(
                    "SELECT status FROM deck_import_operations WHERE id=?1",
                    [&fixture.operation],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(canceled, None);
            assert_eq!(status, "failed");
            Ok(())
        })
        .unwrap();
    assert_eq!(fs::read(&fixture.staged).unwrap(), b"retained import bytes");
}

#[test]
fn cleanup_never_deletes_bytes_for_a_retained_referenced_asset() {
    let fixture = Fixture::new();
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO card_templates(id,name,normalized_name,created_at,updated_at)
                     VALUES(?1,'Retained template','retained template',1,1)",
                    ["2".repeat(32)],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO card_template_assets(template_id,asset_id) VALUES(?1,?2)",
                    params!["2".repeat(32), fixture.asset],
                )
                .map_err(CommandError::database)?;
            cleanup_draft(connection, &fixture.paths, &fixture.draft)?;
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM assets WHERE id=?1",
                    [&fixture.asset],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(fs::read(&fixture.staged).unwrap(), b"retained import bytes");
}

#[test]
fn failed_sql_commit_defers_file_compensation_and_retains_promoted_bytes() {
    let fixture = Fixture::new();
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TABLE injected_commit_constraint(deck_id TEXT REFERENCES decks(id));
                     CREATE TRIGGER fail_import_commit AFTER INSERT ON decks
                     BEGIN INSERT INTO injected_commit_constraint VALUES('missing-parent'); END;
                     PRAGMA defer_foreign_keys=ON;",
                )
                .map_err(CommandError::database)?;
            let package = PackageData {
                version: CURRENT_PACKAGE_VERSION,
                deck: PackageDeck {
                    source_id: "3".repeat(32),
                    name: "Candidate".into(),
                    description: String::new(),
                    sort_order: 0,
                    created_at: 1,
                    updated_at: 1,
                    revision: 1,
                },
                notes: vec![],
                cards: vec![],
                tags: vec![],
                note_tags: vec![],
                note_assets: vec![],
                assets: vec![PackageAsset {
                    source_id: fixture.asset.clone(),
                    original_filename: "fixture.png".into(),
                    mime_type: "image/png".into(),
                    byte_size: 21,
                    data: b"retained import bytes".to_vec(),
                }],
            };
            let mapping = HashMap::from([(fixture.asset.clone(), fixture.asset.clone())]);
            let failure = match apply_package_transaction(
                connection,
                &fixture.paths,
                &fixture.operation,
                &"f".repeat(64),
                &fixture.draft,
                package,
                mapping,
            ) {
                Ok(_) => panic!("The injected deferred constraint must fail commit"),
                Err(failure) => failure,
            };
            assert!(failure.preserve_inputs);
            let original_code = failure.primary.code.clone();
            let error = record_failed_operation(
                connection,
                &fixture.paths,
                &fixture.operation,
                &fixture.draft,
                failure.primary,
                failure.preserve_inputs,
            );
            assert_eq!(error.code, original_code);
            let decks: i64 = connection
                .query_row("SELECT COUNT(*) FROM decks", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            let asset_state: String = connection
                .query_row(
                    "SELECT lifecycle_state FROM assets WHERE id=?1",
                    [&fixture.asset],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(decks, 0);
            assert_eq!(asset_state, "staged");
            Ok(())
        })
        .unwrap();
    assert!(!fixture.staged.exists());
    assert_eq!(fs::read(&fixture.published).unwrap(), b"retained import bytes");
}

#[test]
fn successful_cleanup_removes_only_unreferenced_owned_files() {
    let fixture = Fixture::new();
    fixture
        .database
        .with_connection(|connection| {
            cleanup_draft(connection, &fixture.paths, &fixture.draft)?;
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM assets WHERE id=?1",
                    [&fixture.asset],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    assert!(!fixture.staged.exists());
}
