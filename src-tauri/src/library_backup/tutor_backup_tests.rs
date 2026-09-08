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
