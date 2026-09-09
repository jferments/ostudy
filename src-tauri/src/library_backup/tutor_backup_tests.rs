use super::*;

#[test]
fn full_backup_preserves_conversations_and_memory_policy_but_discards_old_vector_cache() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-tutor-backup-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap()
    ));
    let source_paths = AppPaths::new(root.join("source")).unwrap();
    let source = Database::open(source_paths.database_path()).unwrap();
    source.with_connection(|connection| {
        connection.execute(
            "INSERT INTO tutor_conversations (id,operation_id,title,scope_json,summary_text,
                history_enabled,retrieval_enabled,memory_start_sequence,summary_through_sequence,created_at,updated_at)
             VALUES (?1,?1,'Preserved chat','{\"kind\":\"general\",\"id\":null}','Preserved summary',1,1,2,2,1,1)",
            ["a".repeat(32)],
        ).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    let backup_path = root.join("full.ostudybackup");
    export_backup(&source, &source_paths, &backup_path).unwrap();
    let target_paths = AppPaths::new(root.join("target")).unwrap();
    fs::create_dir_all(target_paths.app_data_dir().join("cache")).unwrap();
    let cache = target_paths
        .app_data_dir()
        .join("cache/semantic-v1.sqlite3");
    fs::write(&cache, b"pre-restore cache must not survive").unwrap();
    copy_and_sync(&backup_path, &pending_restore_path(&target_paths)).unwrap();
    let guard = apply_pending_restore(&target_paths).unwrap().unwrap();
    assert!(!cache.exists());
    let restored = Database::open(target_paths.database_path()).unwrap();
    restored.with_connection(|connection| {
        let row:(String,i64,i64,i64)=connection.query_row(
            "SELECT summary_text,history_enabled,retrieval_enabled,memory_start_sequence FROM tutor_conversations",
            [],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
        ).map_err(CommandError::database)?;
        assert_eq!(row,("Preserved summary".to_string(),1,1,2));
        Ok(())
    }).unwrap();
    drop(restored);
    guard.commit();
    drop(source);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn schema_10_and_11_backups_remain_admissible_after_later_migrations() {
    use sha2::{Digest, Sha256};

    // Build actual historical schemas, not a current database with a falsified
    // version number. Backup admission must not mutate them while validating.
    let migrations = [
        (
            "public_baseline",
            include_str!("../../migrations/0001_public_baseline.sql"),
        ),
        (
            "canonical_documents",
            include_str!("../../migrations/0002_canonical_documents.sql"),
        ),
        (
            "anydoc_document_adapters",
            include_str!("../../migrations/0003_anydoc_document_adapters.sql"),
        ),
        (
            "study_guides_and_learning_topics",
            include_str!("../../migrations/0004_study_guides_and_learning_topics.sql"),
        ),
        (
            "study_guide_pdf_exports",
            include_str!("../../migrations/0005_study_guide_pdf_exports.sql"),
        ),
        (
            "document_citations",
            include_str!("../../migrations/0006_document_citations.sql"),
        ),
        (
            "document_artifact_metadata",
            include_str!("../../migrations/0007_document_artifact_metadata.sql"),
        ),
        (
            "ai_profile_reasoning",
            include_str!("../../migrations/0008_ai_profile_reasoning.sql"),
        ),
        (
            "anki_apkg_staging",
            include_str!("../../migrations/0009_anki_apkg_staging.sql"),
        ),
        (
            "semantic_tutor",
            include_str!("../../migrations/0010_semantic_tutor.sql"),
        ),
        (
            "semantic_lifecycle",
            include_str!("../../migrations/0011_semantic_lifecycle.sql"),
        ),
    ];
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .unwrap();
    for (index, (name, sql)) in migrations.iter().enumerate() {
        connection.execute_batch(sql).unwrap();
        let version = index as i64 + 1;
        connection.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, 1)",
            params![version, name, format!("{:x}", Sha256::digest(sql.as_bytes()))],
        ).unwrap();
        connection
            .pragma_update(None, "user_version", version)
            .unwrap();
        if version == 10 {
            connection.execute(
                "INSERT INTO tutor_conversations (id,operation_id,title,scope_json,created_at,updated_at)
                 VALUES (?1,?1,'Historical chat','{\"kind\":\"general\",\"id\":null}',1,1)",
                ["b".repeat(32)],
            ).unwrap();
        }
        if version >= 10 {
            let changes = connection.total_changes();
            validate_snapshot_database(&connection).unwrap();
            assert_eq!(connection.total_changes(), changes);
            assert_eq!(snapshot_schema_version(&connection).unwrap(), version);
            let title: String = connection
                .query_row("SELECT title FROM tutor_conversations", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(title, "Historical chat");
        }
    }
    connection
        .execute(
            "UPDATE schema_migrations SET checksum = 'tampered' WHERE version = 11",
            [],
        )
        .unwrap();
    assert!(validate_snapshot_database(&connection).is_err());
}
