use super::*;

fn fixture_at(version: usize) -> Connection {
    let mut connection = public_baseline_connection().expect("baseline fixture");
    for migration in &MIGRATIONS[1..version] {
        apply_transactional_migration(&mut connection, migration).expect("known migration");
    }
    connection
}

#[test]
fn every_supported_entry_reaches_the_same_current_schema_and_then_is_a_no_op() {
    let mut current = fixture_at(MIGRATIONS.len());
    initialize_connection(&mut current).unwrap();
    let expected_schema = schema_objects(&current).unwrap();
    for version in 1..=MIGRATIONS.len() {
        let mut connection = fixture_at(version);
        connection
            .execute_batch(
                "INSERT INTO decks (id, name, created_at, updated_at)
                 VALUES ('00000000000000000000000000000001', 'Retained deck', 10, 20);
                 INSERT INTO app_settings (key, value_json, updated_at)
                 VALUES ('lk02-preservation-sentinel', '{\"value\":17}', 30);",
            )
            .unwrap();
        let before_admission = connection.total_changes();
        validate_backup_lineage(&connection).expect("supported backup input");
        assert_eq!(connection.total_changes(), before_admission);
        initialize_connection(&mut connection).expect("normalize before runtime");
        assert_eq!(
            entry::user_version(&connection).unwrap(),
            LATEST_SCHEMA_VERSION
        );
        assert_eq!(schema_objects(&connection).unwrap(), expected_schema);
        let retained: (String, i64, i64) = connection
            .query_row(
                "SELECT name, created_at, updated_at FROM decks",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(retained, ("Retained deck".to_string(), 10, 20));
        let setting: (String, i64) = connection
            .query_row(
                "SELECT value_json, updated_at FROM app_settings WHERE key = 'lk02-preservation-sentinel'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(setting, ("{\"value\":17}".to_string(), 30));
        let changes = connection.total_changes();
        initialize_connection(&mut connection).expect("current-to-current entry");
        assert_eq!(
            connection.total_changes(),
            changes,
            "input schema {version}"
        );
    }
}

#[test]
fn future_schema_error_identifies_running_build_and_preserves_input() {
    let connection = fixture_at(MIGRATIONS.len());
    let future = LATEST_SCHEMA_VERSION + 1;
    connection
        .execute_batch(&format!(
            "INSERT INTO schema_migrations VALUES ({future}, 'future', 'future', 1);
             PRAGMA user_version = {future};"
        ))
        .unwrap();

    let before = connection.total_changes();
    let error = entry::validate_public_input(&connection).unwrap_err();
    assert!(error.contains(&format!("public schema {future}")));
    assert!(error.contains(&format!(
        "this oStudy build supports through public schema {LATEST_SCHEMA_VERSION}"
    )));
    assert!(error.contains("Update or check out a build that supports schema"));
    assert!(error.contains("The database was not modified."));
    assert_eq!(connection.total_changes(), before);

    let backup_error = validate_backup_lineage(&connection).unwrap_err();
    assert_eq!(backup_error.code, "invalid_backup");
    assert_eq!(backup_error.message, error);
    assert_eq!(connection.total_changes(), before);
}

#[test]
fn invalid_later_migration_is_rejected_before_an_earlier_missing_migration_runs() {
    let mut connection = fixture_at(1);
    connection
        .execute_batch(
            "INSERT INTO schema_migrations VALUES (3, 'anydoc_document_adapters', 'corrupt', 1);
             PRAGMA user_version = 3;",
        )
        .unwrap();
    let schema = schema_objects(&connection).unwrap();
    let changes = connection.total_changes();
    let error = initialize_connection(&mut connection).unwrap_err();
    assert!(error.contains("exact supported prefix"));
    assert_eq!(connection.total_changes(), changes);
    assert_eq!(schema_objects(&connection).unwrap(), schema);
    assert!(validate_backup_lineage(&connection).is_err());
}

#[test]
fn invalid_later_repair_is_rejected_before_an_earlier_missing_repair_runs() {
    let mut connection = fixture_at(MIGRATIONS.len());
    initialize_connection(&mut connection).unwrap();
    connection
        .execute_batch(
            "DELETE FROM data_repairs WHERE version = 1;
             UPDATE data_repairs SET checksum = 'corrupt' WHERE version = 3;",
        )
        .unwrap();
    let changes = connection.total_changes();
    assert!(initialize_connection(&mut connection).is_err());
    assert_eq!(connection.total_changes(), changes);
    assert!(validate_backup_lineage(&connection).is_err());
}

#[test]
fn rejected_disk_inputs_preserve_bytes_and_do_not_switch_journal_mode() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-lk02-entry-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let corruptions = [
        "UPDATE schema_metadata SET value = 'unknown' WHERE key = 'schema_lineage'",
        "DELETE FROM schema_migrations WHERE version = 2",
        "UPDATE schema_migrations SET checksum = 'corrupt' WHERE version = 17",
        "INSERT INTO schema_migrations VALUES (999, 'future', 'future', 1)",
        "PRAGMA user_version = 999",
        "INSERT INTO data_repairs VALUES (999, 'future', 'future', 1, 0)",
        "UPDATE data_repairs SET checksum = 'corrupt' WHERE version = 3",
        "DROP TABLE schema_migrations",
        "DROP TABLE data_repairs",
    ];
    for (index, corruption) in corruptions.iter().enumerate() {
        let path = root.join(format!("input-{index}.sqlite3"));
        drop(Database::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(corruption).unwrap();
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
            .unwrap();
        drop(connection);
        let before = std::fs::read(&path).unwrap();
        assert!(Database::open(&path).is_err(), "must reject {corruption}");
        assert_eq!(std::fs::read(&path).unwrap(), before, "{corruption}");
        assert!(!Path::new(&format!("{}-wal", path.display())).exists());
        assert!(!Path::new(&format!("{}-shm", path.display())).exists());
    }
    let corrupt_path = root.join("not-sqlite.sqlite3");
    std::fs::write(&corrupt_path, b"not a SQLite database").unwrap();
    assert!(Database::open(&corrupt_path).is_err());
    assert_eq!(
        std::fs::read(&corrupt_path).unwrap(),
        b"not a SQLite database"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn versioned_empty_input_is_not_relabelled_as_a_new_library() {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("PRAGMA user_version = 999")
        .unwrap();
    assert!(initialize_connection(&mut connection).is_err());
    assert_eq!(entry::user_version(&connection).unwrap(), 999);
    assert!(schema_objects(&connection).unwrap().is_empty());
}

#[test]
fn interrupted_conversion_rolls_back_its_ddl_data_and_ledger() {
    let mut connection = fixture_at(1);
    let before = schema_objects(&connection).unwrap();
    let failing = Migration {
        version: 2,
        name: "fault_injection",
        checksum: "not-a-released-migration",
        sql: "CREATE TABLE lk02_partial (id INTEGER); INSERT INTO lk02_partial VALUES (1); SELECT * FROM lk02_missing;",
        rebuilds_tables: false,
    };
    assert!(apply_transactional_migration(&mut connection, &failing).is_err());
    assert_eq!(schema_objects(&connection).unwrap(), before);
    assert_eq!(entry::validate_public_input(&connection).unwrap(), 1);
    initialize_connection(&mut connection).expect("retry the unchanged supported input");
    assert_eq!(
        entry::user_version(&connection).unwrap(),
        LATEST_SCHEMA_VERSION
    );
}
