use super::*;
use crate::decks::{create_deck_in_connection, CreateDeckInput};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn paths() -> AppPaths {
    AppPaths::new(std::env::temp_dir().join(format!(
        "ostudy-live-restore-{}-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed),
    )))
    .unwrap()
}

fn deck(database: &Database, name: &str) {
    database
        .with_connection(|connection| {
            create_deck_in_connection(
                connection,
                CreateDeckInput {
                    name: name.to_string(),
                    description: None,
                },
            )?;
            Ok(())
        })
        .unwrap();
}

fn deck_names(database: &Database) -> Vec<String> {
    database
        .with_connection(|connection| {
            let mut statement = connection
                .prepare("SELECT name FROM decks ORDER BY name")
                .map_err(CommandError::database)?;
            let result = statement
                .query_map([], |row| row.get(0))
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database);
            result
        })
        .unwrap()
}

#[test]
fn live_activation_replaces_the_connection_seen_by_existing_database_clones() {
    let source_paths = paths();
    let source = Database::open(source_paths.database_path()).unwrap();
    deck(&source, "Restored deck");
    let backup = source_paths.app_data_dir().join("source.ostudybackup");
    export_backup(&source, &source_paths, &backup).unwrap();
    let target_paths = paths();
    let target = Database::open(target_paths.database_path()).unwrap();
    let existing_clone = target.clone();
    deck(&target, "Previous deck");
    let operation = "a".repeat(32);
    let outcome = activate(
        &target,
        &target_paths,
        &backup,
        &operation,
        crate::startup::reconcile_library,
        || {},
    );
    assert!(outcome.connection_ready);
    assert!(outcome.changed);
    let result = outcome.result.unwrap();
    assert!(result.completed);
    assert!(!result.restart_required);
    assert_eq!(result.deck_count, 1);
    assert_eq!(deck_names(&existing_clone), ["Restored deck"]);
    // Replaying a committed receipt must not wipe changes made after restore.
    deck(&existing_clone, "Created after restore");
    assert!(completed_result(&target_paths, &operation)
        .unwrap()
        .is_some());
    assert_eq!(
        deck_names(&target),
        ["Created after restore", "Restored deck"]
    );
    drop((target, existing_clone));
    let reopened = Database::open(target_paths.database_path()).unwrap();
    assert_eq!(
        deck_names(&reopened),
        ["Created after restore", "Restored deck"]
    );
    drop((reopened, source));
    fs::remove_dir_all(source_paths.app_data_dir()).unwrap();
    fs::remove_dir_all(target_paths.app_data_dir()).unwrap();
}

#[test]
fn failed_reconciliation_reopens_old_connection_and_has_no_completed_receipt() {
    let source_paths = paths();
    let source = Database::open(source_paths.database_path()).unwrap();
    deck(&source, "Replacement");
    let backup = source_paths.app_data_dir().join("source.ostudybackup");
    export_backup(&source, &source_paths, &backup).unwrap();
    let target_paths = paths();
    let target = Database::open(target_paths.database_path()).unwrap();
    let clone = target.clone();
    deck(&target, "Preserved");
    let operation = "b".repeat(32);
    let outcome = activate(
        &target,
        &target_paths,
        &backup,
        &operation,
        |_, _| {
            Err(CommandError::new(
                "injected_failure",
                "reconciliation failed",
            ))
        },
        || {},
    );
    assert!(outcome.connection_ready);
    assert_eq!(
        outcome.result.unwrap_err().code,
        "library_restore_rolled_back"
    );
    assert_eq!(deck_names(&clone), ["Preserved"]);
    assert!(completed_result(&target_paths, &operation)
        .unwrap()
        .is_none());
    assert!(backup.is_file());
    drop((source, target, clone));
    fs::remove_dir_all(source_paths.app_data_dir()).unwrap();
    fs::remove_dir_all(target_paths.app_data_dir()).unwrap();
}

#[test]
fn invalid_receipt_never_becomes_a_completed_restore() {
    let paths = paths();
    let operation = "c".repeat(32);
    fs::create_dir_all(paths.app_data_dir().join(RESULTS)).unwrap();
    fs::write(
        paths
            .app_data_dir()
            .join(RESULTS)
            .join(format!("{operation}.json")),
        b"{}",
    )
    .unwrap();
    assert!(completed_result(&paths, &operation).is_err());
    fs::remove_dir_all(paths.app_data_dir()).unwrap();
}

#[test]
fn superseded_pending_backup_is_preserved_and_cannot_activate_on_next_launch() {
    let paths = paths();
    let pending = pending_restore_path(&paths);
    fs::write(&pending, b"retained historical input").unwrap();
    let hash = activation::hash_file(&pending).unwrap();
    activation::preserve_pending(&paths).unwrap();
    let retained = paths
        .app_data_dir()
        .join("restore-inputs")
        .join(format!("{hash}.ostudybackup"));
    assert_eq!(fs::read(&retained).unwrap(), b"retained historical input");
    assert!(!pending.exists());
    // Same input can be preserved twice without losing either the sole valid
    // copy or retrying an already retired startup restore.
    fs::write(&pending, b"retained historical input").unwrap();
    activation::preserve_pending(&paths).unwrap();
    assert!(retained.is_file());
    assert!(!pending.exists());
    fs::remove_dir_all(paths.app_data_dir()).unwrap();
}
