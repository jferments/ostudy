use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use rusqlite::{backup::Backup, params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::*;
use crate::{
    cards::questions::{
        DisplayOrder, InstanceIdentity, QuestionAttempt, QuestionInstance, ValidatedQuestion,
    },
    database::Database,
    device_sync::{
        merge::merge_remote_database,
        package::{create_package, extract_package},
        store::SyncStore,
    },
    paths::AppPaths,
    util::unix_time_millis,
};

const PRIVATE_ANSWER: &str = "OCCLUSION_PRIVATE_ANSWER_602047";
const DRAFT_ONLY_PRIVATE: &str = "EDITOR_DRAFT_PRIVATE_602047";
static NEXT: AtomicU64 = AtomicU64::new(1);

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    database: Database,
    paths: AppPaths,
    root: PathBuf,
    _cleanup: Cleanup,
}

fn id(value: u32) -> String {
    format!("{value:032x}")
}
fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}
fn png_bytes() -> Vec<u8> {
    vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0]
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ostudy-question-transfer-{}-{}-{}",
            std::process::id(),
            unix_time_millis().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = AppPaths::new(root.clone()).unwrap();
        let database = Database::open_in_memory().unwrap();
        let image_relative = format!("assets/images/{}.png", id(8));
        let image_path = paths.resolve_relative(&image_relative);
        fs::create_dir_all(image_path.parent().unwrap()).unwrap();
        fs::write(&image_path, png_bytes()).unwrap();
        database
            .with_connection(|connection| {
                let definition = json!({"schemaVersion":1,"question":{
                    "kind":"image_occlusion","imageAssetId":id(8),
                    "masks":[{"id":id(20),"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}}],
                    "questions":[{"id":id(30),"prompt":rich("Identify this region"),
                        "answer":rich(PRIVATE_ANSWER),"explanation":null,
                        "maskIds":[id(20)],"mode":"hide_all"}]}});
                let question = ValidatedQuestion::parse(&definition.to_string())?;
                let text = question.search_text()?;
                let attempt = QuestionAttempt::start(QuestionInstance::capture(
                    InstanceIdentity {
                        instance_id: id(6),
                        note_id: id(2),
                        card_id: id(3),
                        note_revision: 1,
                    },
                    question.clone(),
                    id(30),
                    DisplayOrder::default(),
                )?)?;
                connection
                    .execute(
                        "INSERT INTO decks (id,name,created_at,updated_at) VALUES (?1,'Test',1,1)",
                        [id(1)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO notes (id,deck_id,note_type,content_format,question_definition,
                            front_text,back_text,duplicate_hash,created_at,updated_at)
                         VALUES (?1,?2,'interactive','question-json',?3,?4,?5,?6,1,1)",
                        params![
                            id(2),
                            id(1),
                            question.to_json()?,
                            text.prompt,
                            text.answer,
                            question.duplicate_hash()?
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO cards (id,note_id,deck_id,created_at,updated_at) VALUES (?1,?2,?3,1,1)",
                        params![id(3), id(2), id(1)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO card_question_presentations (card_id,note_id,question_key) VALUES (?1,?2,?3)",
                        params![id(3), id(2), id(30)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO assets (id,original_filename,mime_type,byte_size,relative_path,created_at)
                         VALUES (?1,'source.png','image/png',?2,?3,1)",
                        params![id(8), i64::try_from(png_bytes().len()).unwrap(), image_relative],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO note_assets (note_id,asset_id,side) VALUES (?1,?2,'content')",
                        params![id(2), id(8)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO question_attempts (instance_id,card_id,note_id,question_kind,
                            instance_hash,revision,phase,assisted,checkpoint_json,created_at,updated_at)
                         VALUES (?1,?2,?3,'image_occlusion',?4,1,'answering',0,?5,1,1)",
                        params![
                            id(6),
                            id(3),
                            id(2),
                            attempt.instance().fingerprint(),
                            attempt.to_json()?
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO question_attempt_assets (instance_id,asset_id) VALUES (?1,?2)",
                        params![id(6), id(8)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO asset_reconciliation_events (id,asset_id,action,detail,occurred_at)
                         VALUES (?1,?2,'verified',?3,1)",
                        params![id(9), id(8), DRAFT_ONLY_PRIVATE],
                    )
                    .map_err(CommandError::database)?;
                let payload = json!({
                    "selectedTemplateKey":"image_occlusion",
                    "answer":DRAFT_ONLY_PRIVATE
                })
                .to_string();
                connection
                    .execute(
                        "INSERT INTO editor_drafts (id,open_operation_id,draft_kind,payload_schema_version,
                            payload_json,payload_sha256,state,created_at,updated_at,last_opened_at)
                         VALUES (?1,?2,'card',1,?3,?4,'active',1,1,1)",
                        params![
                            id(10),
                            id(11),
                            payload,
                            format!("{:x}", Sha256::digest(payload.as_bytes()))
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO editor_draft_assets (draft_id,asset_id) VALUES (?1,?2)",
                        params![id(10), id(8)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO question_note_writes (operation_id,request_hash,note_id,note_revision,
                            card_ids_json,committed_at) VALUES (?1,?2,?3,1,?4,1)",
                        params![id(12), "a".repeat(64), id(2), json!([id(3)]).to_string()],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        Self {
            database,
            paths,
            root: root.clone(),
            _cleanup: Cleanup(root),
        }
    }

    fn assert_source_intact(&self) {
        self.database
            .with_connection(|connection| {
                for table in [
                    "notes",
                    "card_question_presentations",
                    "question_attempts",
                    "question_attempt_assets",
                    "editor_drafts",
                    "question_note_writes",
                    "asset_reconciliation_events",
                ] {
                    let count: i64 = connection
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
                        .map_err(CommandError::database)?;
                    assert_eq!(count, 1, "source row disappeared from {table}");
                }
                let checkpoint: String = connection
                    .query_row("SELECT checkpoint_json FROM question_attempts", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert!(checkpoint.contains(PRIVATE_ANSWER));
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn selected_question_graph_packages_extracts_and_merges_without_drafts_or_write_receipts() {
    let fixture = Fixture::new();
    let destination = fixture.root.join("typed.syncpkg");
    create_package(
        &fixture.database,
        &fixture.paths,
        &destination,
        SyncSelection::default(),
    )
    .unwrap();
    let extracted = extract_package(&destination, &fixture.root.join("received")).unwrap();
    assert_eq!(extracted.assets.len(), 1);
    let remote = Connection::open(&extracted.database_path).unwrap();
    validate_question_snapshot(&remote).unwrap();
    for table in ["editor_drafts", "editor_draft_assets", "question_note_writes", "asset_reconciliation_events"] {
        let count: i64 = remote
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "private authoring receipt survived in {table}");
    }
    let checkpoint: String = remote
        .query_row("SELECT checkpoint_json FROM question_attempts", [], |row| row.get(0))
        .unwrap();
    assert!(checkpoint.contains(PRIVATE_ANSWER));
    QuestionAttempt::restore(&checkpoint).unwrap();

    let receiver_root = fixture.root.join("receiver-library");
    let receiver_paths = AppPaths::new(receiver_root).unwrap();
    let receiver = Database::open_in_memory().unwrap();
    let store = SyncStore::open(&receiver_paths).unwrap();
    store
        .upsert_peer(&id(101), "sender", "public-key", &"b".repeat(64), "")
        .unwrap();
    receiver
        .with_connection(|local| {
            let summary = merge_remote_database(
                local,
                &remote,
                &store,
                &id(100),
                &id(101),
                extracted.selection,
            )?;
            assert!(summary.changed_row_count > 0);
            assert_eq!(summary.conflict_count, 0);
            validate_question_snapshot(local)
        })
        .unwrap();
    receiver
        .with_connection(|connection| {
            for (table, expected) in [
                ("notes", 1_i64),
                ("card_question_presentations", 1),
                ("question_attempts", 1),
                ("question_attempt_assets", 1),
            ] {
                let count: i64 = connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(count, expected);
            }
            Ok(())
        })
        .unwrap();
    fixture.assert_source_intact();
}

#[test]
fn unselected_question_attempts_and_editor_drafts_do_not_survive_in_serialized_transfer_bytes() {
    let fixture = Fixture::new();
    let destination = fixture.root.join("documents-only.syncpkg");
    let selection = SyncSelection {
        flashcards: false,
        ..SyncSelection::default()
    };
    create_package(&fixture.database, &fixture.paths, &destination, selection).unwrap();
    let package = Connection::open(&destination).unwrap();
    let bytes: Vec<u8> = package
        .query_row("SELECT data FROM database_snapshot WHERE id=1", [], |row| row.get(0))
        .unwrap();
    for private in [PRIVATE_ANSWER, DRAFT_ONLY_PRIVATE] {
        assert!(!bytes.windows(private.len()).any(|part| part == private.as_bytes()));
    }
    let extracted = extract_package(&destination, &fixture.root.join("documents-received")).unwrap();
    let connection = Connection::open(&extracted.database_path).unwrap();
    for table in [
        "notes",
        "cards",
        "assets",
        "question_generation_evidence",
        "card_question_presentations",
        "question_attempts",
        "question_attempt_assets",
        "question_review_links",
        "question_remediation_attempts",
        "question_note_writes",
        "editor_drafts",
        "editor_draft_assets",
        "asset_reconciliation_events",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "unselected private rows survived in {table}");
    }
    let violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(violations, 0);
    fixture.assert_source_intact();
}

#[test]
fn a_sender_cannot_bypass_native_question_admission_with_a_semantically_malformed_snapshot() {
    let fixture = Fixture::new();
    let destination = fixture.root.join("malformed-sender.syncpkg");
    create_package(
        &fixture.database,
        &fixture.paths,
        &destination,
        SyncSelection::default(),
    )
    .unwrap();

    let copy_path = fixture.root.join("malformed.sqlite3");
    fixture
        .database
        .with_connection(|source| {
            let mut target = Connection::open(&copy_path).map_err(CommandError::database)?;
            let backup = Backup::new(source, &mut target).map_err(CommandError::database)?;
            backup
                .run_to_completion(128, Duration::from_millis(1), None)
                .map_err(CommandError::database)
        })
        .unwrap();
    let replacement = json!({"schemaVersion":1,"question":{
        "kind":"image_occlusion","imageAssetId":id(8),
        "masks":[{"id":id(20),"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}}],
        "questions":[{"id":id(31),"prompt":rich("Changed target"),
            "answer":rich(PRIVATE_ANSWER),"explanation":null,
            "maskIds":[id(20)],"mode":"hide_all"}]}});
    let malformed = Connection::open(&copy_path).unwrap();
    malformed
        .execute(
            "UPDATE notes SET question_definition = ?1 WHERE id = ?2",
            params![replacement.to_string(), id(2)],
        )
        .unwrap();
    drop(malformed);
    let bytes = fs::read(&copy_path).unwrap();
    let package = Connection::open(&destination).unwrap();
    package
        .execute("UPDATE database_snapshot SET data=?1 WHERE id=1", [bytes])
        .unwrap();
    drop(package);

    let received = fixture.root.join("rejected");
    let error = extract_package(&destination, &received)
        .expect_err("malformed typed graph must not reach merge or asset installation");
    assert_eq!(error.code, "device_sync_question_invalid");
    assert!(!received.join("assets").exists());
    fixture.assert_source_intact();
}
