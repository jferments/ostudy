use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use rusqlite::{backup::Backup, params};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::*;
use crate::{
    cards::questions::{
        DisplayOrder, InstanceIdentity, QuestionAttempt, QuestionInstance, ValidatedQuestion,
    },
    database::Database,
    device_sync::package::{create_package, extract_package},
    paths::AppPaths,
    util::unix_time_millis,
};

const PRIVATE_TEXT: &str = "OCCLUSION_PRIVATE_ANSWER_AND_DRAFT_602047";
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
        database.with_connection(|connection| {
            let definition = json!({"schemaVersion":1,"question":{
                "kind":"image_occlusion","imageAssetId":id(8),
                "masks":[{"id":id(20),"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}}],
                "questions":[{"id":id(30),"prompt":rich("Identify this region"),
                    "answer":rich(PRIVATE_TEXT),"explanation":null,
                    "maskIds":[id(20)],"mode":"hide_all"}]}});
            let attempt = QuestionAttempt::start(QuestionInstance::capture(
                InstanceIdentity {
                    instance_id: id(6), note_id: id(2), card_id: id(3), note_revision: 1,
                },
                ValidatedQuestion::parse(&definition.to_string())?,
                id(30), DisplayOrder::default(),
            )?)?;
            connection.execute(
                "INSERT INTO decks (id,name,created_at,updated_at) VALUES (?1,'Test',1,1)",
                [id(1)],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO notes (id,deck_id,note_type,content_format,question_definition,
                    front_text,back_text,created_at,updated_at)
                 VALUES (?1,?2,'interactive','question-json',?3,'Image question','',1,1)",
                params![id(2),id(1),definition.to_string()],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO cards (id,note_id,deck_id,created_at,updated_at) VALUES (?1,?2,?3,1,1)",
                params![id(3),id(2),id(1)],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO card_question_presentations (card_id,note_id,question_key) VALUES (?1,?2,?3)",
                params![id(3),id(2),id(30)],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO assets (id,original_filename,mime_type,byte_size,relative_path,created_at)
                 VALUES (?1,'source.png','image/png',1,?2,1)",
                params![id(8),format!("assets/images/{}.png",id(8))],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO note_assets (note_id,asset_id,side) VALUES (?1,?2,'content')",
                params![id(2),id(8)],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO question_attempts (instance_id,card_id,note_id,question_kind,
                    instance_hash,revision,phase,assisted,checkpoint_json,created_at,updated_at)
                 VALUES (?1,?2,?3,'image_occlusion',?4,1,'answering',0,?5,1,1)",
                params![id(6),id(3),id(2),attempt.instance().fingerprint(),attempt.to_json()?],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO question_attempt_assets (instance_id,asset_id) VALUES (?1,?2)",
                params![id(6),id(8)],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO asset_reconciliation_events (id,asset_id,action,detail,occurred_at)
                 VALUES (?1,?2,'verified',?3,1)",
                params![id(9),id(8),PRIVATE_TEXT],
            ).map_err(CommandError::database)?;
            let payload = json!({"selectedTemplateKey":"image_occlusion","answer":PRIVATE_TEXT}).to_string();
            connection.execute(
                "INSERT INTO editor_drafts (id,open_operation_id,draft_kind,payload_schema_version,
                    payload_json,payload_sha256,state,created_at,updated_at,last_opened_at)
                 VALUES (?1,?2,'card',1,?3,?4,'active',1,1,1)",
                params![id(10),id(11),payload,format!("{:x}",Sha256::digest(payload.as_bytes()))],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO editor_draft_assets (draft_id,asset_id) VALUES (?1,?2)",
                params![id(10),id(8)],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO question_note_writes (operation_id,request_hash,note_id,note_revision,
                    card_ids_json,committed_at) VALUES (?1,?2,?3,1,?4,1)",
                params![id(12),"a".repeat(64),id(2),json!([id(3)]).to_string()],
            ).map_err(CommandError::database)?;
            Ok(())
        }).unwrap();
        Self { database, paths, root: root.clone(), _cleanup: Cleanup(root) }
    }

    fn assert_source_intact(&self) {
        self.database.with_connection(|connection| {
            for table in ["notes", "question_attempts", "editor_drafts", "question_note_writes", "asset_reconciliation_events"] {
                let count: i64 = connection.query_row(
                    &format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0),
                ).map_err(CommandError::database)?;
                assert_eq!(count, 1);
            }
            let checkpoint: String = connection.query_row(
                "SELECT checkpoint_json FROM question_attempts", [], |row| row.get(0),
            ).map_err(CommandError::database)?;
            assert!(checkpoint.contains(PRIVATE_TEXT));
            Ok(())
        }).unwrap();
    }
}

#[test]
fn incompatible_question_send_is_explicit_and_does_not_replace_an_existing_destination() {
    let fixture = Fixture::new();
    let destination = fixture.root.join("previous.syncpkg");
    fs::write(&destination, b"PREVIOUS EXPORT").unwrap();
    let error = create_package(&fixture.database, &fixture.paths, &destination, SyncSelection::default())
        .expect_err("incomplete typed sync must be rejected");
    assert_eq!(error.code, "interactive_sync_not_supported");
    assert_eq!(fs::read(&destination).unwrap(), b"PREVIOUS EXPORT");
    fixture.assert_source_intact();
}

#[test]
fn unselected_question_attempts_and_editor_drafts_do_not_survive_in_serialized_transfer_bytes() {
    let fixture = Fixture::new();
    let destination = fixture.root.join("documents-only.syncpkg");
    let selection = SyncSelection { flashcards: false, ..SyncSelection::default() };
    create_package(&fixture.database, &fixture.paths, &destination, selection).unwrap();
    let package = Connection::open(&destination).unwrap();
    let bytes: Vec<u8> = package.query_row(
        "SELECT data FROM database_snapshot WHERE id=1", [], |row| row.get(0),
    ).unwrap();
    assert!(!bytes.windows(PRIVATE_TEXT.len()).any(|part| part == PRIVATE_TEXT.as_bytes()));
    let extracted = extract_package(&destination, &fixture.root.join("received")).unwrap();
    let connection = Connection::open(&extracted.database_path).unwrap();
    for table in [
        "notes", "cards", "assets", "card_question_presentations", "question_attempts",
        "question_attempt_assets", "question_review_links", "question_remediation_attempts",
        "question_note_writes", "editor_drafts", "editor_draft_assets", "asset_reconciliation_events",
    ] {
        let count: i64 = connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0, "unselected private rows survived in {table}");
    }
    let violations: i64 = connection.query_row(
        "SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0),
    ).unwrap();
    assert_eq!(violations, 0);
    fixture.assert_source_intact();
}

#[test]
fn an_older_sender_cannot_bypass_question_admission_on_the_receiving_side() {
    let fixture = Fixture::new();
    let destination = fixture.root.join("old-sender.syncpkg");
    let empty = Database::open_in_memory().unwrap();
    create_package(&empty, &fixture.paths, &destination, SyncSelection::default()).unwrap();
    let copy_path = fixture.root.join("unfiltered.sqlite3");
    fixture.database.with_connection(|source| {
        let mut target = Connection::open(&copy_path).map_err(CommandError::database)?;
        let backup = Backup::new(source, &mut target).map_err(CommandError::database)?;
        backup.run_to_completion(128, Duration::from_millis(1), None).map_err(CommandError::database)
    }).unwrap();
    let bytes = fs::read(&copy_path).unwrap();
    let package = Connection::open(&destination).unwrap();
    package.execute("UPDATE database_snapshot SET data=?1 WHERE id=1", [bytes]).unwrap();
    drop(package);
    let received = fixture.root.join("rejected");
    let error = extract_package(&destination, &received).expect_err("incomplete typed graph must not reach merge");
    assert_eq!(error.code, "interactive_sync_not_supported");
    assert!(!received.join("assets").exists());
    fixture.assert_source_intact();
}
