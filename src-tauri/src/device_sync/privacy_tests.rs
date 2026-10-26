use super::*;
use crate::{database::Database, util::unix_time_millis};
use rusqlite::{backup::Backup, params, OpenFlags};
use std::{fs, time::Duration};

const PRIVATE_TEXT: &str = "TUTOR_PRIVATE_SENTINEL_MUST_NOT_TRAVEL_968523";

fn fixture() -> Database {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        connection.execute(
            "INSERT INTO tutor_projects(
                 id,title,description,instructions,default_scope_json,created_at,updated_at
             ) VALUES(?1,'Private project',?2,?2,'{\"kind\":\"general\",\"id\":null}',1,1)",
            params!["e".repeat(32), PRIVATE_TEXT],
        ).map_err(CommandError::database)?;
        connection.execute(
            "INSERT INTO tutor_organization_operations(
                 operation_id,action,target_kind,target_id,request_hash,result_json,created_at
             ) VALUES(?1,'update_project','project',?2,?3,?4,1)",
            params!["f".repeat(32),"e".repeat(32),"c".repeat(64),
                serde_json::json!({"private":PRIVATE_TEXT}).to_string()],
        ).map_err(CommandError::database)?;
        connection.execute(
            "INSERT INTO tutor_conversations (
                 id,operation_id,title,scope_json,summary_text,retrieval_enabled,project_id,created_at,updated_at
             ) VALUES (?1,?1,'Private chat','{\"kind\":\"general\",\"id\":null}',?2,1,?3,1,1)",
            params!["a".repeat(32),PRIVATE_TEXT,"e".repeat(32)],
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
            notes: false,
            lessons: false,
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
                    "tutor_organization_operations",
                    "tutor_projects",
                    "tutor_turn_sources",
                    "tutor_turn_replacements",
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

/// A document with its generated deck and a card the learner reset: the
/// document's deck link and the reset point into both categories' graphs.
fn studied_document_fixture() -> Database {
    let database = Database::open_in_memory().unwrap();
    let (deck, document, job, note, card) = (
        "1".repeat(32),
        "2".repeat(32),
        "3".repeat(32),
        "4".repeat(32),
        "5".repeat(32),
    );
    database
        .with_connection(|connection| {
            connection
                .execute_batch(&format!(
                    "INSERT INTO decks(id, name, created_at, updated_at) VALUES ('{deck}', 'Deck', 1, 1);
                     INSERT INTO documents(id, title, original_filename, mime_type, byte_size, sha256,
                         source_relative_path, lifecycle_state, created_at, updated_at,
                         generated_deck_id)
                     VALUES ('{document}', 'Source', 'source.pdf', 'application/pdf', 10,
                         '{sha}', 'documents/source.pdf', 'ready', 1, 1, '{deck}');
                     INSERT INTO document_generation_jobs(id, document_id, provider_kind, model_id,
                         profile_revision, credential_epoch, network_scope, state, page_count,
                         created_at, updated_at, generated_deck_id)
                     VALUES ('{job}', '{document}', 'local', 'model', 1, 0, 'local', 'succeeded', 1,
                         1, 1, '{deck}');
                     INSERT INTO document_decks(document_id, deck_id, job_id, created_at)
                     VALUES ('{document}', '{deck}', '{job}', 1);
                     INSERT INTO notes(id, deck_id, note_type, content_format,
                         content_schema_version, front_doc, back_doc, front_text, back_text,
                         created_at, updated_at, revision)
                     VALUES ('{note}', '{deck}', 'basic', 'tiptap-json', 1, '{doc}', '{doc}',
                         'question', 'question', 1, 1, 1);
                     INSERT INTO cards(id, note_id, deck_id, direction, template_ordinal,
                         created_at, updated_at)
                     VALUES ('{card}', '{note}', '{deck}', 'forward', 0, 1, 1);
                     INSERT INTO card_schedules(card_id, state, due_at, stability, difficulty,
                         elapsed_days, scheduled_days, reps, lapses, last_review_at,
                         scheduler_name, scheduler_version, updated_at, revision)
                     VALUES ('{card}', 0, 2, 0.0, 0.0, 0, 0, 0, 0, 2, 'fsrs', 'rs-fsrs-1.2.1', 2, 2);
                     INSERT INTO schedule_action_operations(id, action, requested_count,
                         changed_count, unchanged_count, rejected_count, occurred_at, created_at)
                     VALUES ('{operation}', 'reset', 1, 1, 0, 0, 2, 2);
                     INSERT INTO schedule_action_events(id, operation_id, card_id, action,
                         due_before, due_after, state_before, state_after,
                         schedule_revision_before, schedule_revision_after, occurred_at,
                         created_at)
                     VALUES ('{event}', '{operation}', '{card}', 'reset', 1, 2, 0, 0, 1, 2, 2, 2);",
                    sha = "a".repeat(64),
                    doc = serde_json::json!({
                        "type": "doc",
                        "content": [{
                            "type": "paragraph",
                            "content": [{"type": "text", "text": "question"}],
                        }],
                    }),
                    operation = "6".repeat(32),
                    event = "7".repeat(32),
                ))
                .map_err(CommandError::database)
        })
        .unwrap();
    database
}

#[test]
fn every_valid_sync_selection_leaves_a_complete_graph_for_a_studied_document() {
    for bits in 1..32_u8 {
        let selection = SyncSelection {
            flashcards: bits & 1 != 0,
            documents: bits & 2 != 0,
            study_guides: bits & 4 != 0,
            notes: false,
            lessons: false,
            api_settings: bits & 8 != 0,
            api_credentials: bits & 16 != 0,
        };
        if selection.validate().is_err() {
            continue;
        }
        studied_document_fixture()
            .with_connection(|connection| sanitize_snapshot(connection, selection))
            .unwrap_or_else(|error| panic!("selection {bits}: {error:?}"));
    }
}
