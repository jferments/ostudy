use super::*;
use crate::decks::{create_deck_in_connection, CreateDeckInput};

fn paths(label: &str) -> AppPaths {
    AppPaths::new(std::env::temp_dir().join(format!(
        "ostudy-restore-preparation-{label}-{}-{}",
        std::process::id(),
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

fn names(database: &Database) -> Vec<String> {
    database
        .with_connection(|connection| {
            let mut statement = connection
                .prepare("SELECT name FROM decks ORDER BY name")
                .map_err(crate::error::CommandError::database)?;
            let rows = statement
                .query_map([], |row| row.get(0))
                .map_err(crate::error::CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(crate::error::CommandError::database)?;
            Ok(rows)
        })
        .unwrap()
}

fn source_backup(paths: &AppPaths) -> PathBuf {
    let database = Database::open(paths.database_path()).unwrap();
    deck(&database, "Replacement");
    let backup = paths.app_data_dir().join("input.ostudybackup");
    super::super::export_backup(&database, paths, &backup).unwrap();
    backup
}

#[test]
fn invalid_preparation_never_journals_or_closes_the_live_library() {
    let target_paths = paths("invalid");
    let target = Database::open(target_paths.database_path()).unwrap();
    deck(&target, "Preserved");
    let input = target_paths.app_data_dir().join("bad.ostudybackup");
    fs::write(&input, b"not a SQLite backup").unwrap();
    assert!(prepare_source_restore(&target_paths, &input).is_err());
    assert!(!target_paths.app_data_dir().join(JOURNAL).exists());
    assert!(!recover(&target_paths).unwrap());
    deck(&target, "Still writable");
    assert_eq!(names(&target), ["Preserved", "Still writable"]);
    assert_eq!(fs::read(input).unwrap(), b"not a SQLite backup");
    drop(target);
    fs::remove_dir_all(target_paths.app_data_dir()).unwrap();
}

#[test]
fn abandoned_preparation_preserves_current_writes_and_never_auto_activates() {
    let source_paths = paths("abandoned-source");
    let backup = source_backup(&source_paths);
    let target_paths = paths("abandoned-target");
    let target = Database::open(target_paths.database_path()).unwrap();
    deck(&target, "Previous");
    let prepared = prepare_source_restore(&target_paths, &backup).unwrap();
    let retained = prepared.transaction.clone();
    deck(&target, "Written during preparation");
    drop(prepared);
    assert!(retained.join("source.ostudybackup").is_file());
    assert!(retained.join("candidate/library.sqlite3").is_file());
    assert!(!recover(&target_paths).unwrap());
    drop(target);
    let reopened = Database::open(target_paths.database_path()).unwrap();
    assert_eq!(names(&reopened), ["Previous", "Written during preparation"]);
    drop(reopened);
    fs::remove_dir_all(source_paths.app_data_dir()).unwrap();
    fs::remove_dir_all(target_paths.app_data_dir()).unwrap();
}

#[test]
fn activation_uses_the_validated_private_copy_not_the_changed_input_path() {
    let source_paths = paths("private-source");
    let backup = source_backup(&source_paths);
    let target_paths = paths("private-target");
    let target = Database::open(target_paths.database_path()).unwrap();
    deck(&target, "Previous");
    let prepared = prepare_source_restore(&target_paths, &backup).unwrap();
    fs::write(&backup, b"changed external input").unwrap();
    drop(target);
    let mut guard = install_prepared_restore(&target_paths, prepared).unwrap();
    let restored = Database::open(target_paths.database_path()).unwrap();
    assert_eq!(names(&restored), ["Replacement"]);
    guard.try_commit().unwrap();
    drop(restored);
    assert!(!recover(&target_paths).unwrap());
    fs::remove_dir_all(source_paths.app_data_dir()).unwrap();
    fs::remove_dir_all(target_paths.app_data_dir()).unwrap();
}

#[test]
fn changed_private_source_or_candidate_is_rejected_before_any_active_file_moves() {
    for relative in ["source.ostudybackup", "candidate/library.sqlite3"] {
        let source_paths = paths("changed-source");
        let backup = source_backup(&source_paths);
        let target_paths = paths("changed-target");
        let target = Database::open(target_paths.database_path()).unwrap();
        deck(&target, "Preserved");
        let prepared = prepare_source_restore(&target_paths, &backup).unwrap();
        fs::write(prepared.transaction.join(relative), b"tampered bytes").unwrap();
        let error = install_prepared_restore(&target_paths, prepared).unwrap_err();
        assert!(error.contains("candidate changed"));
        assert!(!target_paths.app_data_dir().join(JOURNAL).exists());
        assert!(!recover(&target_paths).unwrap());
        assert_eq!(names(&target), ["Preserved"]);
        deck(&target, "Writable after refusal");
        drop(target);
        fs::remove_dir_all(source_paths.app_data_dir()).unwrap();
        fs::remove_dir_all(target_paths.app_data_dir()).unwrap();
    }
}
