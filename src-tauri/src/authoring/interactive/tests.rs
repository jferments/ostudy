use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use serde_json::{json, Value};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    database: Database,
    paths: AppPaths,
    _cleanup: Cleanup,
}

struct Cleanup(std::path::PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl Fixture {
    fn new() -> Self {
        let folder = std::env::temp_dir().join(format!(
            "ostudy-question-authoring-{}-{}-{}",
            std::process::id(),
            crate::util::unix_time_millis().unwrap(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let paths = AppPaths::new(folder.clone()).unwrap();
        let database = Database::open(paths.database_path()).unwrap();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO decks (id, name, created_at, updated_at) VALUES (?1, 'Questions', 1, 1)",
                        [id(1)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        Self {
            database,
            paths,
            _cleanup: Cleanup(folder),
        }
    }

    fn save(&self, input: SaveQuestionNoteInput) -> Result<QuestionWriteReceipt, CommandError> {
        self.database
            .with_connection(|connection| persistence::save(connection, &self.paths, input))
    }

    fn count(&self, table: &str) -> i64 {
        // Fixed private fixture names only, never runtime/user query structure.
        self.database
            .with_connection(|connection| {
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })
                    .map_err(CommandError::database)
            })
            .unwrap()
    }

    fn add_image(&self) {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
            .unwrap();
        let relative = AppPaths::image_relative_path(&id(40), "png");
        std::fs::write(self.paths.resolve_relative(&relative), &bytes).unwrap();
        self.database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO assets (id, original_filename, mime_type, byte_size, relative_path, created_at)
                         VALUES (?1, 'diagram.png', 'image/png', ?2, ?3, 1)",
                        params![id(40), bytes.len() as i64, relative],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
    }
}

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn item(value: u32) -> Value {
    json!({"id":id(value),"content":rich(&format!("Item {value}"))})
}

fn choice() -> Value {
    json!({
        "kind":"single_choice", "prompt":rich("Choose the best answer."),
        "options":[
            {"id":id(11),"content":rich("First"),"rationale":rich("First explanation")},
            {"id":id(12),"content":rich("Second"),"rationale":rich("Second explanation")}
        ],
        "correctOptionId":id(12), "explanation":rich("Overall explanation")
    })
}

fn input(operation: u32, body: Value) -> SaveQuestionNoteInput {
    SaveQuestionNoteInput {
        operation_id: id(operation),
        note_id: None,
        revision: None,
        deck_id: id(1),
        definition_json: json!({"schemaVersion":1,"question":body}).to_string(),
        tags: vec!["Science".to_string()],
        allow_duplicate: false,
        asset_draft_id: None,
        editor_draft: None,
    }
}

fn occlusion() -> Value {
    json!({
        "kind":"image_occlusion", "imageAssetId":id(40),
        "masks":[
            {"id":id(50),"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}},
            {"id":id(51),"rect":{"x":0.5,"y":0.5,"width":0.2,"height":0.2}}
        ],
        "questions":[
            {"id":id(60),"prompt":rich("Name region one."),"answer":rich("First region"),
             "explanation":null,"maskIds":[id(50)],"mode":"hide_all"},
            {"id":id(61),"prompt":rich("Name region two."),"answer":rich("Second region"),
             "explanation":null,"maskIds":[id(51)],"mode":"hide_target"}
        ]
    })
}

#[test]
fn save_and_file_reopen_keep_typed_source_tags_and_one_schedule() {
    let fixture = Fixture::new();
    let request = input(2, choice());
    let receipt = fixture.save(request.clone()).unwrap();
    assert_eq!(receipt.card_ids.len(), 1);
    assert_eq!(receipt.note_revision, 1);
    assert_eq!(fixture.count("notes"), 1);
    assert_eq!(fixture.count("card_schedules"), 1);
    let reopened = Database::open(fixture.paths.database_path()).unwrap();
    reopened
        .with_connection(|connection| {
            let note = load(connection, &fixture.paths, &receipt.note_id)?;
            assert_eq!(note.card_ids, receipt.card_ids);
            assert_eq!(note.tags, vec!["Science"]);
            assert_eq!(
                serde_json::to_value(note.definition).unwrap(),
                serde_json::from_str::<Value>(&request.definition_json).unwrap()
            );
            let owner: (String, Option<String>, Option<String>) = connection
                .query_row(
                    "SELECT content_format, front_doc, back_doc FROM notes WHERE id = ?1",
                    [&receipt.note_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(owner, ("question-json".to_string(), None, None));
            Ok(())
        })
        .unwrap();
}

#[test]
fn save_replay_is_stable_even_after_the_material_is_deleted() {
    let fixture = Fixture::new();
    let request = input(2, choice());
    let first = fixture.save(request.clone()).unwrap();
    assert_eq!(fixture.save(request.clone()).unwrap(), first);
    assert_eq!(fixture.count("question_note_writes"), 1);
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute("DELETE FROM notes WHERE id = ?1", [&first.note_id])
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(fixture.save(request).unwrap(), first);
    assert_eq!(fixture.count("notes"), 0);
    assert_eq!(fixture.count("cards"), 0);
    assert_eq!(fixture.count("card_schedules"), 0);
}

#[test]
fn changed_content_cannot_reuse_a_committed_save_identity() {
    let fixture = Fixture::new();
    fixture.save(input(2, choice())).unwrap();
    let mut different = choice();
    different["correctOptionId"] = json!(id(11));
    let error = fixture.save(input(2, different)).unwrap_err();
    assert_eq!(error.code, "question_operation_conflict");
    assert_eq!(fixture.count("notes"), 1);
    assert_eq!(fixture.count("question_note_writes"), 1);
}

#[test]
fn duplicate_identity_ignores_generated_ids_and_choice_order_but_not_the_key() {
    let fixture = Fixture::new();
    fixture.save(input(2, choice())).unwrap();
    let mut same = choice();
    same["options"][0]["id"] = json!(id(21));
    same["options"][1]["id"] = json!(id(22));
    same["correctOptionId"] = json!(id(22));
    same["options"].as_array_mut().unwrap().reverse();
    assert_eq!(
        fixture.save(input(3, same.clone())).unwrap_err().code,
        "duplicate_card"
    );
    same["correctOptionId"] = json!(id(21));
    fixture.save(input(4, same)).unwrap();
    assert_eq!(fixture.count("notes"), 2);
    let mut override_request = input(5, choice());
    override_request.allow_duplicate = true;
    fixture.save(override_request).unwrap();
    assert_eq!(fixture.count("notes"), 3);
}

#[test]
fn updates_preserve_card_identity_and_schedule_and_reject_stale_revisions() {
    let fixture = Fixture::new();
    let first = fixture.save(input(2, choice())).unwrap();
    fixture
        .database
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE card_schedules SET due_at = 123456, reps = 7 WHERE card_id = ?1",
                    [&first.card_ids[0]],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let mut body = choice();
    body["prompt"] = rich("Updated prompt.");
    let mut update = input(3, body);
    update.note_id = Some(first.note_id.clone());
    update.revision = Some(1);
    let saved = fixture.save(update.clone()).unwrap();
    assert_eq!(saved.card_ids, first.card_ids);
    assert_eq!(saved.note_revision, 2);
    update.operation_id = id(4);
    assert_eq!(fixture.save(update).unwrap_err().code, "card_changed");
    fixture
        .database
        .with_connection(|connection| {
            let schedule: (i64, i64) = connection
                .query_row(
                    "SELECT due_at, reps FROM card_schedules WHERE card_id = ?1",
                    [&first.card_ids[0]],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(schedule, (123456, 7));
            Ok(())
        })
        .unwrap();
}

#[test]
fn all_objective_types_save_without_becoming_basic_front_back_cards() {
    let fixture = Fixture::new();
    let bodies = [
        choice(),
        json!({"kind":"select_all","prompt":rich("Select all."),"options":choice()["options"],"correctOptionIds":[],"explanation":null}),
        json!({"kind":"true_false","statement":rich("A statement."),"correct":false,"explanation":null,"correctedStatement":null}),
        json!({"kind":"matching","prompt":rich("Match."),"pairs":[{"left":item(11),"right":item(12),"rationale":null},{"left":item(13),"right":item(14),"rationale":null}],"explanation":null}),
        json!({"kind":"grouping","prompt":rich("Group."),"categories":[item(11),item(12)],"items":[{"item":item(13),"categoryId":id(11),"rationale":null},{"item":item(14),"categoryId":id(12),"rationale":null}],"explanation":null}),
        json!({"kind":"ordering","prompt":rich("Order from first to last."),"items":[item(11),item(12)],"explanation":null}),
    ];
    for (index, body) in bodies.into_iter().enumerate() {
        let saved = fixture.save(input(index as u32 + 100, body)).unwrap();
        assert_eq!(saved.card_ids.len(), 1);
    }
    assert_eq!(fixture.count("notes"), 6);
    assert_eq!(fixture.count("card_question_presentations"), 6);
    assert_eq!(fixture.count("card_schedules"), 6);
}

#[test]
fn invalid_definitions_missing_assets_and_draft_failure_commit_nothing() {
    let fixture = Fixture::new();
    let mut invalid = choice();
    invalid["correctOptionId"] = json!(id(99));
    assert!(fixture.save(input(2, invalid)).is_err());
    let mut missing_asset = choice();
    missing_asset["options"][0]["rationale"] = json!({"type":"doc","content":[{"type":"image","attrs":{"assetId":id(40),"src":format!("study-asset://{}",id(40))}}]});
    assert!(fixture.save(input(3, missing_asset)).is_err());
    let mut missing_draft = input(4, choice());
    missing_draft.editor_draft = Some(EditorDraftCommitRef {
        draft_id: id(90),
        revision: 1,
    });
    assert!(fixture.save(missing_draft).is_err());
    for table in [
        "notes",
        "cards",
        "card_schedules",
        "note_tags",
        "question_note_writes",
    ] {
        assert_eq!(fixture.count(table), 0, "rollback left rows in {table}");
    }
}

#[test]
fn a_saved_question_cannot_be_reinterpreted_as_another_question_kind() {
    let fixture = Fixture::new();
    let first = fixture.save(input(2, choice())).unwrap();
    let mut update = input(
        3,
        json!({"kind":"true_false","statement":rich("Statement"),"correct":true,"explanation":null,"correctedStatement":null}),
    );
    update.note_id = Some(first.note_id);
    update.revision = Some(1);
    assert_eq!(fixture.save(update).unwrap_err().code, "card_type_locked");
    assert_eq!(fixture.count("question_note_writes"), 1);
}

#[test]
fn occlusion_questions_keep_schedule_ownership_across_reorder_removal_and_restore() {
    let fixture = Fixture::new();
    fixture.add_image();
    let first = fixture.save(input(2, occlusion())).unwrap();
    assert_eq!(first.card_ids.len(), 2);
    let mut body = occlusion();
    body["questions"].as_array_mut().unwrap().reverse();
    body["masks"][0]["rect"]["x"] = json!(0.2);
    let mut update = input(3, body.clone());
    update.note_id = Some(first.note_id.clone());
    update.revision = Some(1);
    let second = fixture.save(update).unwrap();
    assert_eq!(
        second.card_ids,
        vec![first.card_ids[1].clone(), first.card_ids[0].clone()]
    );
    body["questions"].as_array_mut().unwrap().remove(1);
    let mut remove = input(4, body);
    remove.note_id = Some(first.note_id.clone());
    remove.revision = Some(2);
    let third = fixture.save(remove).unwrap();
    assert_eq!(third.card_ids, vec![first.card_ids[1].clone()]);
    let mut restore = input(5, occlusion());
    restore.note_id = Some(first.note_id);
    restore.revision = Some(3);
    let restored = fixture.save(restore).unwrap();
    assert_eq!(restored.card_ids, first.card_ids);
    assert_eq!(fixture.count("cards"), 2);
    assert_eq!(fixture.count("card_schedules"), 2);
    assert_eq!(fixture.count("note_assets"), 1);
}
