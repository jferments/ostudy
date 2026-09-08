use super::*;
use crate::{database::Database, util::unix_time_millis};
use rusqlite::{backup::Backup, params, OpenFlags};
use std::{fs, time::Duration};

const PRIVATE_TEXT: &str = "TUTOR_PRIVATE_SENTINEL_MUST_NOT_TRAVEL_968523";

fn fixture() -> Database {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        connection.execute(
            "INSERT INTO tutor_conversations (id,operation_id,title,scope_json,summary_text,created_at,updated_at)
             VALUES (?1,?1,'Private chat','{\"kind\":\"general\",\"id\":null}',?2,1,1)",
            params!["a".repeat(32),PRIVATE_TEXT],
        ).map_err(CommandError::database)?;
        connection.execute(
            "INSERT INTO tutor_turns (id,conversation_id,operation_id,sequence,request_hash,request_json,
                 mode,user_text,assistant_text,state,profile_revision,provider_kind,model_id,created_at,updated_at)
             VALUES (?1,?2,?1,1,?3,'{}','explain',?4,?4,'complete',1,'llama_cpp','fixture',1,1)",
            params!["b".repeat(32),"a".repeat(32),"c".repeat(64),PRIVATE_TEXT],
        ).map_err(CommandError::database)?;
        // Real source projections also contain copies of chat text.
        connection.execute(
            "INSERT INTO knowledge_sources (id,kind,owner_id,segment_key,parent_key,owner_revision,
                 content_hash,record_json,conversation_id,turn_id,created_at,updated_at)
             VALUES (?1,'conversation',?2,'turn:0','turn','1',?3,?4,?2,?5,1,1)",
            params!["d".repeat(32),"a".repeat(32),"c".repeat(64),
                serde_json::json!({"text":PRIVATE_TEXT}).to_string(),"b".repeat(32)],
        ).map_err(CommandError::database)?;
        Ok(())
    }).unwrap();
    database
}

#[test]
fn every_valid_sync_selection_excludes_chat_and_semantic_projection_rows() {
    for bits in 1..32_u8 {
        let selection = SyncSelection {
            flashcards: bits & 1 != 0,
            documents: bits & 2 != 0,
            study_guides: bits & 4 != 0,
            api_settings: bits & 8 != 0,
            api_credentials: bits & 16 != 0,
        };
        if selection.validate().is_err() {
            continue;
        }
        let database = fixture();
        database
            .with_connection(|connection| {
                sanitize_snapshot(connection, selection)?;
                for table in [
                    "tutor_turn_sources",
                    "tutor_turns",
                    "tutor_conversations",
                    "knowledge_sources",
                    "knowledge_units",
                    "knowledge_artifact_links",
                    "knowledge_preparations",
                ] {
                    let count: i64 = connection
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                        .map_err(CommandError::database)?;
                    assert_eq!(
                        count, 0,
                        "private rows survived in {table}, selection {bits}"
                    );
                    assert!(!selection.includes_table(table));
                }
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn serialized_snapshot_has_no_deleted_chat_bytes_or_uncheckpointed_wal() {
    let database = fixture();
    let root = std::env::temp_dir().join(format!(
        "ostudy-sync-privacy-{}-{}",
        std::process::id(),
        unix_time_millis().unwrap()
    ));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("snapshot.sqlite3");
    database
        .with_connection(|source| {
            let mut target = Connection::open(&path).map_err(CommandError::database)?;
            let backup = Backup::new(source, &mut target).map_err(CommandError::database)?;
            backup
                .run_to_completion(128, Duration::from_millis(1), None)
                .map_err(CommandError::database)
        })
        .unwrap();
    let copy = Connection::open(&path).unwrap();
    sanitize_snapshot(&copy, SyncSelection::default()).unwrap();
    drop(copy);
    let bytes = fs::read(&path).unwrap();
    assert!(!bytes
        .windows(PRIVATE_TEXT.len())
        .any(|window| window == PRIVATE_TEXT.as_bytes()));
    let snapshot = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let count: i64 = snapshot
        .query_row("SELECT COUNT(*) FROM tutor_turns", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
    let violations: i64 = snapshot
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
    // This was a copy: producing a transfer must not erase local chat history.
    database
        .with_connection(|connection| {
            let text: String = connection
                .query_row("SELECT user_text FROM tutor_turns", [], |r| r.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(text, PRIVATE_TEXT);
            Ok(())
        })
        .unwrap();
    drop(snapshot);
    fs::remove_dir_all(root).unwrap();
}
