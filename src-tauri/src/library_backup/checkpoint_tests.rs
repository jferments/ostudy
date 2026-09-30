use super::*;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

const APPLICATION: &str = "io.github.jferments.ostudy.dev";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(AppPaths);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-upgrade-checkpoint-{}-{}-{}",
            std::process::id(),
            crate::util::unix_time_millis().expect("clock"),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        let paths = AppPaths::new(root).expect("isolated paths");
        let connection = Connection::open(paths.database_path()).expect("baseline");
        let sql = include_str!("../../schema/0001_release_baseline.sql");
        connection.execute_batch(sql).expect("immutable baseline");
        connection.execute("INSERT INTO schema_metadata(key, value) VALUES ('schema_lineage', ?1)", [crate::database::CURRENT_SCHEMA_LINEAGE]).expect("lineage");
        connection.execute("INSERT INTO schema_migrations(version, name, checksum, applied_at) VALUES (1, 'release_baseline', ?1, 0)", [format!("{:x}", Sha256::digest(sql.as_bytes()))]).expect("ledger");
        connection.pragma_update(None, "user_version", 1).expect("schema 1");
        drop(connection);
        for relative in MANAGED {
            fs::create_dir_all(paths.app_data_dir().join(relative)).expect("managed directory");
            fs::write(paths.app_data_dir().join(relative).join("retained.bin"), relative.as_bytes()).expect("managed fixture");
        }
        Self(paths)
    }

    fn prepare(&self) -> PreparedRestore {
        prepare_upgrade_with(&self.0, APPLICATION, (crate::database::CURRENT_SCHEMA_LINEAGE, 1), &mut |_| {}, None, |path| {
            let database = Database::open(path)?;
            settle_candidate(&database)
        }).expect("private candidate")
    }

    fn transaction(&self) -> PathBuf {
        let intent = read_intent(&self.0).expect("intent read").expect("intent");
        transaction_path(&self.0, &intent.operation_id).expect("transaction")
    }

    fn operation(&self) -> String {
        read_intent(&self.0).expect("intent").expect("present intent").operation_id
    }

    fn version(&self) -> i64 {
        read_database(self.0.database_path()).expect("read library")
            .query_row("PRAGMA user_version", [], |row| row.get(0)).expect("version")
    }

    fn original_managed_files_are_present(&self) {
        for relative in MANAGED {
            assert_eq!(fs::read(self.0.app_data_dir().join(relative).join("retained.bin")).expect("retained bytes"), relative.as_bytes());
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) { fs::remove_dir_all(self.0.app_data_dir()).expect("remove isolated test library"); }
}

#[test]
fn checkpoint_is_verified_before_the_first_private_migration_write() {
    let fixture = Fixture::new();
    let original = fs::read(fixture.0.database_path()).expect("original");
    let result = prepare_upgrade_with(&fixture.0, APPLICATION, (crate::database::CURRENT_SCHEMA_LINEAGE, 1), &mut |_| {}, None, |candidate| {
        let transaction = fixture.transaction();
        let manifest = verify_checkpoint(&transaction)?;
        assert_eq!(manifest.source_schema, 1);
        assert_eq!(manifest.target_schema, crate::database::CURRENT_SCHEMA_VERSION);
        assert!(manifest.from_build.is_none(), "historic build is not guessed");
        // Simulate an early migration committing, followed by a later failure.
        let connection = Connection::open(candidate).expect("candidate");
        connection.execute("UPDATE app_settings SET value_json = '31' WHERE key = 'dailyNewLimit'", []).expect("private committed write");
        Err("injected subsequent migration failure".into())
    });
    assert!(result.expect_err("failed migration").contains("injected"));
    assert_eq!(fs::read(fixture.0.database_path()).expect("live bytes"), original);
    assert_eq!(fixture.version(), 1);
    fixture.original_managed_files_are_present();
    assert!(fixture.transaction().join("candidate/library.sqlite3").exists());
    assert!(admit_startup(&fixture.0).expect_err("explicit retry required").starts_with(RECOVERY_REQUIRED));
}

#[test]
fn sqlite_full_stops_before_any_migration_or_live_replacement() {
    let fixture = Fixture::new();
    let original = fs::read(fixture.0.database_path()).expect("original");
    let mut called = false;
    let result = prepare_upgrade_with(&fixture.0, APPLICATION, (crate::database::CURRENT_SCHEMA_LINEAGE, 1), &mut |_| {}, Some(1), |_| {
        called = true;
        Ok(())
    });
    assert!(result.is_err());
    assert!(!called);
    assert_eq!(fs::read(fixture.0.database_path()).expect("live bytes"), original);
    assert!(!fixture.0.app_data_dir().join(JOURNAL).exists());
    assert!(has_intent(&fixture.0).expect("retained failure intent"));
}

#[test]
fn a_sqlite_backup_includes_committed_wal_rows_without_copying_only_the_main_file() {
    let fixture = Fixture::new();
    let source = Connection::open(fixture.0.database_path()).expect("source");
    source.execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;").expect("WAL");
    source.execute("UPDATE app_settings SET value_json = '27' WHERE key = 'dailyNewLimit'", []).expect("committed WAL write");
    let prepared = fixture.prepare();
    let snapshot = read_database(&prepared.transaction.join("checkpoint/library.sqlite3")).expect("snapshot");
    let value: String = snapshot.query_row("SELECT value_json FROM app_settings WHERE key = 'dailyNewLimit'", [], |row| row.get(0)).expect("WAL value");
    assert_eq!(value, "27");
    assert_eq!(fixture.version(), 1);
    drop(snapshot);
    drop(source);
    drop(prepared);
}

#[test]
fn tampered_candidate_or_checkpoint_is_rejected_before_journaling() {
    for checkpoint in [false, true] {
        let fixture = Fixture::new();
        let prepared = fixture.prepare();
        let tree = if checkpoint { "checkpoint" } else { "candidate" };
        fs::write(prepared.transaction.join(tree).join("documents/retained.bin"), b"tampered").expect("tamper fixture");
        assert!(install_prepared_restore(&fixture.0, prepared).is_err());
        assert_eq!(fixture.version(), 1);
        assert!(!fixture.0.app_data_dir().join(JOURNAL).exists());
        fixture.original_managed_files_are_present();
    }
}

#[test]
fn failed_reconciliation_restores_all_managed_trees_and_retains_the_failed_library() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare();
    let transaction = prepared.transaction.clone();
    let guard = install_prepared_restore(&fixture.0, prepared).expect("activation");
    assert_eq!(fixture.version(), crate::database::CURRENT_SCHEMA_VERSION);
    for relative in MANAGED {
        fs::write(fixture.0.app_data_dir().join(relative).join("retained.bin"), b"candidate reconciliation change").expect("private active change");
    }
    drop(guard);
    assert_eq!(fixture.version(), 1);
    fixture.original_managed_files_are_present();
    assert!(transaction.join("failed/library.sqlite3").exists());
    assert_eq!(fs::read(transaction.join("failed/transfers/retained.bin")).expect("failed bytes"), b"candidate reconciliation change");
    verify_checkpoint(&transaction).expect("checkpoint retained");
    assert!(admit_startup(&fixture.0).is_err());
}

#[test]
fn successful_activation_keeps_checkpoint_and_previous_library_without_repeating_upgrade() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare();
    let transaction = prepared.transaction.clone();
    let mut guard = install_prepared_restore(&fixture.0, prepared).expect("activation");
    commit_upgrade(&fixture.0, &mut guard).expect("commit");
    drop(guard);
    assert_eq!(fixture.version(), crate::database::CURRENT_SCHEMA_VERSION);
    assert!(!has_intent(&fixture.0).expect("intent finalized"));
    admit_startup(&fixture.0).expect("normal startup");
    verify_checkpoint(&transaction).expect("checkpoint preserved");
    let previous = read_database(&transaction.join("previous/library.sqlite3")).expect("previous");
    assert_eq!(validate_database(&previous).expect("previous schema").1, 1);
    fixture.original_managed_files_are_present();
}

#[test]
fn explicit_restore_reinstates_old_schema_without_reverse_sql_and_preserves_new_state() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare();
    let original_transaction = prepared.transaction.clone();
    let mut guard = install_prepared_restore(&fixture.0, prepared).expect("activation");
    // A committed activation with its intent still present models a lost
    // acknowledgement/cleanup boundary while the recovery UI still owns it.
    guard.try_commit().expect("activation committed");
    drop(guard);
    fs::write(fixture.0.app_data_dir().join("documents/retained.bin"), b"new work retained separately").expect("later bytes");
    restore_checkpoint(&fixture.0, &fixture.operation(), &mut |_| {}).expect("explicit checkpoint restore");
    assert_eq!(fixture.version(), 1);
    fixture.original_managed_files_are_present();
    let revert_transaction = fixture.transaction();
    assert_ne!(revert_transaction, original_transaction);
    assert_eq!(fs::read(revert_transaction.join("previous/documents/retained.bin")).expect("preserved newer work"), b"new work retained separately");
    assert!(admit_startup(&fixture.0).is_err(), "new executable must not immediately re-upgrade the restored library");
    allow_retry(&fixture.0, &fixture.operation()).expect("explicit later retry authorization");
    admit_startup(&fixture.0).expect("retry allowed only by user decision");
}

#[test]
fn abandoned_preparation_never_auto_installs_and_retry_retains_every_attempt() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare();
    let first = prepared.transaction.clone();
    drop(prepared);
    assert!(super::super::apply_pending_restore(&fixture.0).expect("no activation").is_none());
    assert_eq!(fixture.version(), 1);
    assert!(admit_startup(&fixture.0).is_err());
    allow_retry(&fixture.0, &fixture.operation()).expect("explicit retry");
    let second = fixture.prepare();
    assert_ne!(first, second.transaction);
    verify_checkpoint(&first).expect("old checkpoint retained");
    verify_checkpoint(&second.transaction).expect("new checkpoint retained");
}

#[cfg(unix)]
#[test]
fn private_checkpoint_permissions_and_symlink_refusal() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let prepared = fixture.prepare();
    assert_eq!(fs::metadata(&prepared.transaction).expect("private directory").permissions().mode() & 0o077, 0);
    drop(prepared);
    allow_retry(&fixture.0, &fixture.operation()).expect("retry");
    std::os::unix::fs::symlink(fixture.0.database_path(), fixture.0.app_data_dir().join("documents/unsafe")).expect("symlink fixture");
    assert!(begin_upgrade(&fixture.0, APPLICATION, (crate::database::CURRENT_SCHEMA_LINEAGE, 1), &mut |_| {}).is_err());
    assert_eq!(fixture.version(), 1);
}

#[test]
fn upgrade_crash_child() {
    let Some(root) = std::env::var_os("OSTUDY_UPGRADE_CHILD_ROOT") else { return; };
    let paths = AppPaths::new(PathBuf::from(root)).expect("child paths");
    let mut guard = begin_upgrade(&paths, APPLICATION, (crate::database::CURRENT_SCHEMA_LINEAGE, 1), &mut |_| {}).expect("child upgrade");
    if std::env::var("OSTUDY_RESTORE_CRASH_PHASE").expect("phase").starts_with("rollback-") {
        guard.rollback().expect("rollback cutpoint");
    } else {
        guard.try_commit().expect("commit cutpoint");
    }
    panic!("the requested crash cutpoint was not reached");
}

#[test]
fn upgrade_activation_and_rollback_crashes_recover_from_the_existing_journal() {
    for phase in ["journal", "previous-0", "previous-4", "previous-7", "install-0", "install-2", "install-4", "install-5", "rollback-8", "rollback-4", "rollback-0", "committed"] {
        let fixture = Fixture::new();
        let status = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", "library_backup::activation::checkpoint::tests::upgrade_crash_child", "--nocapture", "--test-threads=1"])
            .env("OSTUDY_UPGRADE_CHILD_ROOT", fixture.0.app_data_dir())
            .env("OSTUDY_RESTORE_CRASH_ROOT", fixture.0.app_data_dir())
            .env("OSTUDY_RESTORE_CRASH_PHASE", phase)
            .status().expect("crash child");
        assert_eq!(status.code(), Some(77), "cutpoint {phase}");
        assert!(super::super::apply_pending_restore(&fixture.0).expect("journal recovery").is_none());
        assert_eq!(fixture.version(), if phase == "committed" { crate::database::CURRENT_SCHEMA_VERSION } else { 1 }, "cutpoint {phase}");
        fixture.original_managed_files_are_present();
        assert!(super::super::apply_pending_restore(&fixture.0).expect("idempotent recovery").is_none());
        if phase == "committed" { admit_startup(&fixture.0).expect("committed intent cleanup"); }
        else { assert!(admit_startup(&fixture.0).is_err(), "interrupted migration must not silently retry"); }
    }
}
