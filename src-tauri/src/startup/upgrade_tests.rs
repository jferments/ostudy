use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::params;
use sha2::{Digest, Sha256};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ostudy-upgrade-plan-{}-{}-{}",
            std::process::id(),
            crate::util::unix_time_millis().expect("clock"),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("fixture root");
        Self(path)
    }

    fn database(&self) -> std::path::PathBuf {
        self.0.join("library.sqlite3")
    }

    fn baseline(&self) -> Connection {
        let connection = Connection::open(self.database()).expect("baseline connection");
        let sql = include_str!("../../schema/0001_release_baseline.sql");
        connection.execute_batch(sql).expect("immutable baseline");
        connection
            .execute(
                "INSERT INTO schema_metadata(key, value) VALUES ('schema_lineage', ?1)",
                [CURRENT_SCHEMA_LINEAGE],
            )
            .expect("lineage");
        connection
            .execute(
                "INSERT INTO schema_migrations(version, name, checksum, applied_at)
                 VALUES (1, 'release_baseline', ?1, 0)",
                [format!("{:x}", Sha256::digest(sql.as_bytes()))],
            )
            .expect("baseline ledger");
        connection
            .pragma_update(None, "user_version", 1)
            .expect("version");
        connection
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove isolated fixture");
    }
}

#[test]
fn missing_library_is_not_created_by_inspection() {
    let fixture = Fixture::new();
    assert_eq!(inspect(&fixture.database()).expect("inspection"), None);
    assert!(!fixture.database().exists());
}

#[test]
fn supported_baseline_produces_a_plan_without_changing_any_database_bytes() {
    let fixture = Fixture::new();
    drop(fixture.baseline());
    let before = fs::read(fixture.database()).expect("original bytes");
    let plan = inspect(&fixture.database())
        .expect("inspection")
        .expect("upgrade");
    assert_eq!(plan.source_lineage, CURRENT_SCHEMA_LINEAGE);
    assert_eq!(plan.source_version, 1);
    assert_eq!(plan.target_lineage, CURRENT_SCHEMA_LINEAGE);
    assert_eq!(plan.target_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(fs::read(fixture.database()).expect("retained bytes"), before);
}

#[test]
fn current_library_uses_normal_open_without_an_upgrade_plan() {
    let fixture = Fixture::new();
    drop(database::Database::open(&fixture.database()).expect("current database"));
    assert_eq!(inspect(&fixture.database()).expect("inspection"), None);
}

#[test]
fn future_schema_and_forged_ledger_are_rejected_without_relabelling() {
    for future in [false, true] {
        let fixture = Fixture::new();
        let connection = fixture.baseline();
        if future {
            connection
                .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
                .expect("future version fixture");
        } else {
            connection
                .execute(
                    "UPDATE schema_migrations SET checksum = ?1",
                    ["f".repeat(64)],
                )
                .expect("forged checksum fixture");
        }
        drop(connection);
        let before = fs::read(fixture.database()).expect("original bytes");
        assert!(inspect(&fixture.database()).is_err());
        assert_eq!(fs::read(fixture.database()).expect("retained bytes"), before);
    }
}

#[test]
fn committed_wal_rows_participate_in_read_only_admission() {
    let fixture = Fixture::new();
    let connection = fixture.baseline();
    connection
        .execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;")
        .expect("WAL fixture");
    connection
        .execute(
            "UPDATE schema_migrations SET checksum = ?1",
            params!["0".repeat(64)],
        )
        .expect("committed WAL change");
    assert!(fixture
        .0
        .join("library.sqlite3-wal")
        .metadata()
        .expect("WAL")
        .len()
        > 0);
    assert!(
        inspect(&fixture.database()).is_err(),
        "the read must not ignore the WAL ledger change"
    );
    drop(connection);
}

#[test]
fn pre_public_library_keeps_the_existing_manual_recovery_contract() {
    let fixture = Fixture::new();
    let connection = Connection::open(fixture.database()).expect("legacy connection");
    connection
        .execute_batch("CREATE TABLE old_notes(id INTEGER PRIMARY KEY);")
        .expect("legacy fixture");
    drop(connection);
    assert!(inspect(&fixture.database())
        .expect_err("explicit recovery")
        .starts_with(database::PRE_PUBLIC_RESET_PREFIX));
}

#[cfg(unix)]
#[test]
fn a_database_symlink_cannot_redirect_upgrade_inspection() {
    let fixture = Fixture::new();
    let target = fixture.0.join("other.sqlite3");
    drop(Connection::open(&target).expect("target"));
    std::os::unix::fs::symlink(&target, fixture.database()).expect("symlink fixture");
    assert!(inspect(&fixture.database())
        .expect_err("no alias")
        .contains("symbolic link"));
}

#[test]
fn orphaned_journals_never_create_a_replacement_empty_library() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("library.sqlite3-wal"), b"retained WAL").expect("journal");
    assert!(inspect(&fixture.database()).expect_err("orphan").contains("missing"));
    assert!(!fixture.database().exists());
}
