use super::*;

#[test]
fn current_reopen_validates_payload_once_on_every_admission_without_writes() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            for _ in 0..3 {
                let passes = entry::current_payload_passes();
                let changes = connection.total_changes();
                initialize_connection(connection)
                    .map_err(|error| CommandError::new("test_open_failed", error))?;
                assert_eq!(entry::current_payload_passes() - passes, 1);
                assert_eq!(connection.total_changes(), changes);
            }
            let passes = entry::current_payload_passes();
            validate_backup_lineage(connection)?;
            assert_eq!(entry::current_payload_passes() - passes, 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_successful_open_never_authenticates_later_corrupt_payload_bytes() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-current-payload-reopen-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("library.sqlite3");
    let database = Database::open(&path).unwrap();
    database
        .with_connection(|connection| {
            let rich = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Retained"}]}]}"#;
            connection
                .execute(
                    "INSERT INTO decks(id, name, created_at, updated_at)
                     VALUES (?1, 'Retained deck', 1, 1)",
                    ["a".repeat(32)],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "INSERT INTO notes(id, deck_id, front_doc, back_doc,
                         front_text, back_text, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?3, 'Retained', 'Retained', 1, 1)",
                    params!["b".repeat(32), "a".repeat(32), rich],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    drop(database);
    let passes = entry::current_payload_passes();
    drop(Database::open(&path).unwrap());
    assert_eq!(entry::current_payload_passes() - passes, 1);

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE notes SET front_doc = '{\"type\":\"unsupported\"}'",
            [],
        )
        .unwrap();
    let changes = connection.total_changes();
    let error = validate_backup_lineage(&connection).unwrap_err();
    assert_eq!(error.code, "invalid_backup");
    assert!(error.message.contains("corrupt stored content"));
    assert_eq!(connection.total_changes(), changes);
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
        .unwrap();
    drop(connection);

    let before = std::fs::read(&path).unwrap();
    let error = Database::open(&path).err().unwrap();
    assert!(error.contains("corrupt stored content"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!root.join("library.sqlite3-wal").exists());
    assert!(!root.join("library.sqlite3-shm").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn connection_measurement_preserves_success_error_and_lock_release() {
    let database = Database::open_in_memory().unwrap();
    let value = database
        .with_connection(|connection| {
            connection
                .query_row("SELECT 42", [], |row| row.get::<_, i64>(0))
                .map_err(CommandError::database)
        })
        .unwrap();
    assert_eq!(value, 42);
    let error = database
        .with_connection::<()>(|_| Err(CommandError::new("test_failure", "Preserved failure")))
        .unwrap_err();
    assert_eq!(error.code, "test_failure");
    assert_eq!(error.message, "Preserved failure");
    assert!(database.connection.try_lock().is_ok());
    assert_eq!(database.with_connection(|_| Ok(7)).unwrap(), 7);
}
