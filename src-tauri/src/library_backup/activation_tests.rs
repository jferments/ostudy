use super::*;
use std::process::Command;

fn paths(label: &str) -> AppPaths {
    AppPaths::new(std::env::temp_dir().join(format!(
        "ostudy-restore-journal-{label}-{}-{}-{}",
        std::process::id(),
        crate::util::unix_time_millis().unwrap(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ))).unwrap()
}

fn put(root: &Path, relative: &str, value: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, value).unwrap();
}

fn fixture(paths: &AppPaths) -> PendingRestoreGuard {
    let root = paths.app_data_dir();
    for relative in ENTRIES {
        if ["assets", "documents"].contains(relative) {
            put(root, &format!("{relative}/retained.bin"), relative.as_bytes());
        } else {
            put(root, relative, relative.as_bytes());
        }
    }
    let transaction = new_transaction(root).unwrap();
    put(&transaction, "candidate/library.sqlite3", b"replacement database");
    put(&transaction, "candidate/assets/new.bin", b"replacement asset");
    put(&transaction, "candidate/documents/new.bin", b"replacement document");
    fs::create_dir(transaction.join("previous")).unwrap();
    let journal = Journal {
        version: 1,
        operation_id: transaction.file_name().unwrap().to_str().unwrap().to_string(),
        original: vec![true; ENTRIES.len()],
        source_sha256: "a".repeat(64),
    };
    write_new_file(&root.join(JOURNAL), &serde_json::to_vec(&journal).unwrap()).unwrap();
    PendingRestoreGuard {
        root: root.to_path_buf(), journal, installed: false, finished: false,
    }
}

fn assert_previous(paths: &AppPaths) {
    let root = paths.app_data_dir();
    for relative in ENTRIES {
        let target = if ["assets", "documents"].contains(relative) {
            root.join(relative).join("retained.bin")
        } else {
            root.join(relative)
        };
        assert_eq!(fs::read(&target).unwrap(), relative.as_bytes(), "{relative}");
    }
    assert!(!root.join(JOURNAL).exists());
}

fn child(root: &Path, mode: &str, phase: &str) {
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "library_backup::activation::current_format_tests::crash_driver", "--nocapture"])
        .env("OSTUDY_RESTORE_CRASH_ROOT", root)
        .env("OSTUDY_RESTORE_CRASH_PHASE", phase)
        .env("OSTUDY_RESTORE_CRASH_MODE", mode)
        .output().unwrap();
    assert_eq!(result.status.code(), Some(77), "{}", String::from_utf8_lossy(&result.stderr));
}

#[test]
fn crash_driver() {
    let Some(root) = std::env::var_os("OSTUDY_RESTORE_CRASH_ROOT") else { return; };
    let root = PathBuf::from(root);
    let mode = std::env::var("OSTUDY_RESTORE_CRASH_MODE").unwrap();
    if mode == "wal" {
        let connection = rusqlite::Connection::open(root.join("library.sqlite3")).unwrap();
        connection.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
             CREATE TABLE preserved(value TEXT); INSERT INTO preserved VALUES('previous');"
        ).unwrap();
        std::process::exit(77);
    }
    let journal: Journal = serde_json::from_slice(&fs::read(root.join(JOURNAL)).unwrap()).unwrap();
    let mut guard = PendingRestoreGuard { root, journal, installed: false, finished: false };
    if mode == "rollback" {
        guard.rollback().unwrap();
    } else {
        guard.install().unwrap();
        guard.try_commit().unwrap();
    }
    panic!("the selected crash boundary was not reached");
}

#[test]
fn process_death_at_every_install_boundary_recovers_the_whole_previous_library() {
    let phases = std::iter::once("journal".to_string())
        .chain((0..ENTRIES.len()).map(|index| format!("previous-{index}")))
        .chain((0..INSTALL.len()).map(|index| format!("install-{index}")));
    for phase in phases {
        let paths = paths(&phase);
        let guard = fixture(&paths);
        std::mem::forget(guard);
        child(paths.app_data_dir(), "install", &phase);
        assert!(recover(&paths).unwrap());
        assert_previous(&paths);
        assert!(!recover(&paths).unwrap());
        fs::remove_dir_all(paths.app_data_dir()).unwrap();
    }
}

#[test]
fn interrupted_rollback_is_idempotent_at_every_entry() {
    for index in 0..ENTRIES.len() {
        let paths = paths(&format!("rollback-{index}"));
        let mut guard = fixture(&paths);
        guard.install().unwrap();
        std::mem::forget(guard);
        child(paths.app_data_dir(), "rollback", &format!("rollback-{index}"));
        assert!(recover(&paths).unwrap());
        assert_previous(&paths);
        fs::remove_dir_all(paths.app_data_dir()).unwrap();
    }
}

#[test]
fn a_durable_commit_is_not_rolled_back_after_process_death() {
    let paths = paths("commit");
    let guard = fixture(&paths);
    let previous = guard.transaction().join("previous/library.sqlite3");
    std::mem::forget(guard);
    child(paths.app_data_dir(), "install", "committed");
    assert!(recover(&paths).unwrap());
    assert_eq!(fs::read(paths.database_path()).unwrap(), b"replacement database");
    assert_eq!(fs::read(previous).unwrap(), b"library.sqlite3");
    assert!(!recover(&paths).unwrap());
    fs::remove_dir_all(paths.app_data_dir()).unwrap();
}

#[test]
fn wal_committed_rows_survive_install_and_rollback_without_journal_mispairing() {
    let paths = paths("wal");
    child(paths.app_data_dir(), "wal", "");
    assert!(paths.app_data_dir().join("library.sqlite3-wal").metadata().unwrap().len() > 0);
    let transaction = new_transaction(paths.app_data_dir()).unwrap();
    fs::create_dir_all(transaction.join("candidate/assets")).unwrap();
    fs::create_dir_all(transaction.join("candidate/documents")).unwrap();
    let candidate = rusqlite::Connection::open(transaction.join("candidate/library.sqlite3")).unwrap();
    candidate.execute_batch("CREATE TABLE preserved(value TEXT); INSERT INTO preserved VALUES('replacement');").unwrap();
    drop(candidate);
    let journal = Journal {
        version: 1,
        operation_id: transaction.file_name().unwrap().to_str().unwrap().to_string(),
        original: ENTRIES.iter().map(|relative| paths.app_data_dir().join(relative).exists()).collect(),
        source_sha256: "b".repeat(64),
    };
    write_new_file(&paths.app_data_dir().join(JOURNAL), &serde_json::to_vec(&journal).unwrap()).unwrap();
    let mut guard = PendingRestoreGuard {
        root: paths.app_data_dir().to_path_buf(), journal, installed: false, finished: false,
    };
    guard.install().unwrap();
    let active = rusqlite::Connection::open(paths.database_path()).unwrap();
    let value: String = active.query_row("SELECT value FROM preserved", [], |row| row.get(0)).unwrap();
    assert_eq!(value, "replacement");
    drop(active);
    guard.rollback().unwrap();
    let previous = rusqlite::Connection::open(paths.database_path()).unwrap();
    let value: String = previous.query_row("SELECT value FROM preserved", [], |row| row.get(0)).unwrap();
    assert_eq!(value, "previous");
    drop(previous);
    fs::remove_dir_all(paths.app_data_dir()).unwrap();
}

#[test]
fn legacy_rollback_and_invalid_journals_are_preserved() {
    let paths = paths("legacy");
    put(paths.app_data_dir(), "library.sqlite3.pre-restore", b"irreplaceable previous library");
    put(paths.app_data_dir(), "pending-library-restore.ostudybackup", b"pending backup");
    assert!(apply_pending_restore(&paths).unwrap_err().contains("older interrupted restore"));
    assert_eq!(fs::read(paths.app_data_dir().join("library.sqlite3.pre-restore")).unwrap(), b"irreplaceable previous library");
    put(paths.app_data_dir(), JOURNAL, b"incomplete journal");
    assert!(recover(&paths).unwrap_err().contains("unreadable"));
    assert_eq!(fs::read(paths.app_data_dir().join(JOURNAL)).unwrap(), b"incomplete journal");
    fs::remove_dir_all(paths.app_data_dir()).unwrap();
}

#[cfg(unix)]
#[test]
fn journal_cannot_follow_a_symlink_outside_the_library() {
    let paths = paths("symlink");
    let external = paths.app_data_dir().join("external-evidence");
    fs::write(&external, b"keep").unwrap();
    std::os::unix::fs::symlink(&external, paths.app_data_dir().join(JOURNAL)).unwrap();
    assert!(recover(&paths).unwrap_err().contains("symbolic links"));
    assert_eq!(fs::read(external).unwrap(), b"keep");
    fs::remove_dir_all(paths.app_data_dir()).unwrap();
}
