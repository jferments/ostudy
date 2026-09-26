use super::*;
use crate::cards::validate_content;
use crate::decks::create_deck_in_connection;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn id(value: u32) -> String {
    format!("{value:032x}")
}

fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn seed(connection: &mut Connection) -> (String, String) {
    let deck = create_deck_in_connection(
        connection,
        CreateDeckInput {
            name: "Source".to_string(),
            description: None,
        },
    )
    .unwrap();
    let content = validate_content(
        rich("Question"),
        rich("Answer"),
        String::new(),
        String::new(),
    )
    .unwrap();
    let transaction = connection.transaction().unwrap();
    let note = super::super::insert_organized_card(
        &transaction,
        &deck.id,
        content,
        &super::super::validate_tags(vec!["Retained".to_string()]).unwrap(),
        "test-hash",
        100,
    )
    .unwrap();
    transaction.commit().unwrap();
    (deck.id, note)
}

fn input(operation: u32, notes: Vec<String>) -> SaveSelectionAsDeckInput {
    SaveSelectionAsDeckInput {
        operation_id: id(operation),
        name: "Selection".to_string(),
        description: Some("From selected study notes.".to_string()),
        note_ids: notes,
    }
}

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn dump(connection: &Connection, query: &str, owner: &str) -> Vec<Vec<String>> {
    let mut statement = connection.prepare(query).unwrap();
    let columns = statement.column_count();
    let mut rows = statement.query([owner]).unwrap();
    let mut result = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        result.push(
            (0..columns)
                .map(|index| format!("{:?}", row.get_ref(index).unwrap()))
                .collect(),
        );
    }
    result
}

#[test]
fn committed_operation_replays_before_source_checks_and_rejects_conflicting_input() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (_, note) = seed(connection);
            let request = input(1, vec![note.clone(), note.clone()]);
            let first = save_selection_as_deck_in_connection(connection, request.clone())?;
            connection
                .execute("UPDATE notes SET revision=revision+1 WHERE id=?1", [&note])
                .map_err(CommandError::database)?;
            let replay = save_selection_as_deck_in_connection(connection, input(1, vec![note]))?;
            assert_eq!(first, replay);
            assert_eq!(count(connection, "decks"), 2);
            assert_eq!(count(connection, "selection_deck_operations"), 1);
            let mut conflict = request;
            conflict.description = Some("A different intention".to_string());
            assert_eq!(
                save_selection_as_deck_in_connection(connection, conflict)
                    .unwrap_err()
                    .code,
                "selection_deck_operation_conflict"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn failed_receipt_insert_rolls_back_every_copy_and_destination() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (_, note) = seed(connection);
            connection
                .execute_batch(
                    "CREATE TEMP TRIGGER fail_selection_receipt
                 BEFORE INSERT ON selection_deck_operations
                 BEGIN SELECT RAISE(ABORT,'injected receipt failure'); END;",
                )
                .map_err(CommandError::database)?;
            let tables = ["decks", "notes", "cards", "card_schedules", "note_tags"];
            let before = tables.map(|table| count(connection, table));
            assert!(
                save_selection_as_deck_in_connection(connection, input(2, vec![note.clone()]))
                    .is_err()
            );
            assert_eq!(before, tables.map(|table| count(connection, table)));
            assert_eq!(count(connection, "selection_deck_operations"), 0);
            connection
                .execute_batch("DROP TRIGGER fail_selection_receipt;")
                .map_err(CommandError::database)?;
            let result = save_selection_as_deck_in_connection(connection, input(2, vec![note]))?;
            assert_eq!(result.affected_count, 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn empty_or_missing_selection_never_leaves_an_empty_destination() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (_, note) = seed(connection);
            for notes in [vec![], vec![note, id(999_999)]] {
                assert!(save_selection_as_deck_in_connection(connection, input(3, notes)).is_err());
                assert_eq!(count(connection, "decks"), 1);
                assert_eq!(count(connection, "notes"), 1);
                assert_eq!(count(connection, "selection_deck_operations"), 0);
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn copied_presentations_are_fresh_and_original_notes_schedules_and_reviews_are_unchanged() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (deck, note) = seed(connection);
            let card: String = connection.query_row(
                "SELECT id FROM cards WHERE note_id=?1", [&note], |row| row.get(0),
            ).map_err(CommandError::database)?;
            connection.execute(
                "UPDATE card_schedules SET reps=7,lapses=2,revision=4 WHERE card_id=?1",
                [&card],
            ).map_err(CommandError::database)?;
            connection.execute(
                "INSERT INTO review_events (
                     id,submission_id,session_id,card_id,reviewed_at,rating,answer_correct,
                     confidence,response_time_ms,state_before,state_after,due_before,due_after,
                     stability_before,stability_after,difficulty_before,difficulty_after,
                     scheduled_days_before,scheduled_days_after,scheduler_name,scheduler_version,
                     created_at,request_fingerprint,result_completed,result_rated_count,result_correct_count
                 ) VALUES(?1,?2,NULL,?3,123,3,1,'somewhat_confident',100,0,2,100,5000,
                     0.0,3.0,0.0,4.5,0,5,'fsrs','test',123,?4,1,1,1)",
                params![id(801),id(802),card,"f".repeat(64)],
            ).map_err(CommandError::database)?;
            let source_notes = dump(connection, "SELECT * FROM notes WHERE deck_id=?1 ORDER BY id", &deck);
            let source_schedules = dump(connection, "SELECT * FROM card_schedules WHERE card_id=?1", &card);
            let source_reviews = dump(connection, "SELECT * FROM review_events WHERE card_id=?1", &card);
            let result = save_selection_as_deck_in_connection(connection, input(4, vec![note]))?;
            assert_eq!(source_notes, dump(connection, "SELECT * FROM notes WHERE deck_id=?1 ORDER BY id", &deck));
            assert_eq!(source_schedules, dump(connection, "SELECT * FROM card_schedules WHERE card_id=?1", &card));
            assert_eq!(source_reviews, dump(connection, "SELECT * FROM review_events WHERE card_id=?1", &card));
            assert_eq!(count(connection, "review_events"), 1);
            let fresh: i64 = connection.query_row(
                "SELECT COUNT(*) FROM card_schedules s JOIN cards c ON c.id=s.card_id
                 WHERE c.deck_id=?1 AND s.state=0 AND s.reps=0 AND s.lapses=0 AND s.revision=1",
                [&result.deck.id], |row| row.get(0),
            ).map_err(CommandError::database)?;
            assert_eq!(fresh, result.deck.generated_card_count);
            assert_eq!(count(connection, "note_tags"), 2);
            Ok(())
        })
        .unwrap();
}

#[test]
fn concurrent_retries_commit_one_result() {
    let database = Database::open_in_memory().unwrap();
    let note = database
        .with_connection(|connection| Ok(seed(connection).1))
        .unwrap();
    let request = input(5, vec![note]);
    let other_database = database.clone();
    let other_request = request.clone();
    let worker = std::thread::spawn(move || {
        other_database
            .with_connection(|connection| {
                save_selection_as_deck_in_connection(connection, other_request)
            })
            .unwrap()
    });
    let result = database
        .with_connection(|connection| save_selection_as_deck_in_connection(connection, request))
        .unwrap();
    assert_eq!(result, worker.join().unwrap());
    database
        .with_connection(|connection| {
            assert_eq!(count(connection, "decks"), 2);
            assert_eq!(count(connection, "selection_deck_operations"), 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn receipt_survives_reopen_and_deleted_destination_without_resurrection() {
    let root = std::env::temp_dir().join(format!(
        "ostudy-selection-replay-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("library.sqlite3");
    let database = Database::open(&path).unwrap();
    let (request, first) = database
        .with_connection(|connection| {
            let (source_deck, note) = seed(connection);
            let request = input(6, vec![note]);
            let first = save_selection_as_deck_in_connection(connection, request.clone())?;
            connection
                .execute(
                    "DELETE FROM decks WHERE id IN (?1,?2)",
                    params![source_deck, first.deck.id],
                )
                .map_err(CommandError::database)?;
            Ok((request, first))
        })
        .unwrap();
    drop(database);
    let reopened = Database::open(&path).unwrap();
    reopened
        .with_connection(|connection| {
            let replay = save_selection_as_deck_in_connection(connection, request)?;
            assert_eq!(replay, first);
            assert_eq!(count(connection, "decks"), 0);
            assert_eq!(count(connection, "selection_deck_operations"), 1);
            crate::database::validate_backup_lineage(connection)?;
            Ok(())
        })
        .unwrap();
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn long_unicode_names_and_descriptions_fit_the_receipt_and_replay() {
    let database = Database::open_in_memory().unwrap();
    database
        .with_connection(|connection| {
            let (_, note) = seed(connection);
            let mut request = input(7, vec![note]);
            request.name = "🧠".repeat(200);
            request.description = Some("🧠".repeat(2000));
            let first = save_selection_as_deck_in_connection(connection, request.clone())?;
            assert_eq!(
                save_selection_as_deck_in_connection(connection, request)?,
                first
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn distinct_reverse_and_cloze_behavior_is_not_deduplicated_as_plain_text() {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        let (_, first) = seed(connection);
        let (_, reverse) = seed(connection);
        let (_, cloze) = seed(connection);
        let (_, exact) = seed(connection);
        connection.execute("UPDATE notes SET reverse_enabled=1 WHERE id=?1", [&reverse]).map_err(CommandError::database)?;
        let reverse_card = id(811);
        connection.execute(
            "INSERT INTO cards(id,note_id,deck_id,direction,created_at,updated_at)
             SELECT ?1,id,deck_id,'reverse',100,100 FROM notes WHERE id=?2",
            params![reverse_card,reverse],
        ).map_err(CommandError::database)?;
        insert_new_schedule(connection, &reverse_card, 100)?;
        connection.execute(
            "UPDATE notes SET note_type='cloze',cloze_source='{{c1::Question}}',cloze_mode='one_at_a_time' WHERE id=?1",
            [&cloze],
        ).map_err(CommandError::database)?;
        let result = save_selection_as_deck_in_connection(connection, input(8, vec![first,reverse,cloze,exact]))?;
        assert_eq!(result.affected_count, 3);
        assert_eq!(result.skipped_duplicates, 1);
        assert_eq!(result.deck.generated_card_count, 4);
        Ok(())
    }).unwrap();
}

fn question_body(index: u32) -> Value {
    let a = id(index * 20 + 1);
    let b = id(index * 20 + 2);
    let c = id(index * 20 + 3);
    let d = id(index * 20 + 4);
    match index {
        1 => json!({"kind":"single_choice","prompt":rich("Pick one."),"options":[
            {"id":a,"content":rich("A"),"rationale":null},
            {"id":b,"content":rich("B"),"rationale":rich("Reason")}
        ],"correctOptionId":a,"explanation":null}),
        2 => json!({"kind":"select_all","prompt":rich("Pick all."),"options":[
            {"id":a,"content":rich("A"),"rationale":null},
            {"id":b,"content":rich("B"),"rationale":null}
        ],"correctOptionIds":[a,b],"explanation":rich("Both")}),
        3 => json!({"kind":"true_false","statement":rich("A proposition."),"correct":false,
            "explanation":null,"correctedStatement":rich("A correction.")}),
        4 => json!({"kind":"matching","prompt":rich("Match."),"pairs":[
            {"left":{"id":a,"content":rich("A")},"right":{"id":c,"content":rich("C")},"rationale":null},
            {"left":{"id":b,"content":rich("B")},"right":{"id":d,"content":rich("D")},"rationale":null}
        ],"explanation":null}),
        5 => json!({"kind":"grouping","prompt":rich("Group."),"categories":[
            {"id":a,"content":rich("First")},{"id":b,"content":rich("Second")}
        ],"items":[
            {"item":{"id":c,"content":rich("C")},"categoryId":a,"rationale":null},
            {"item":{"id":d,"content":rich("D")},"categoryId":b,"rationale":null}
        ],"explanation":null}),
        6 => json!({"kind":"ordering","prompt":rich("Order."),"items":[
            {"id":a,"content":rich("First")},{"id":b,"content":rich("Second")}
        ],"explanation":null}),
        7 => json!({"kind":"image_occlusion","imageAssetId":id(999),"masks":[
            {"id":a,"rect":{"x":0.1,"y":0.1,"width":0.2,"height":0.2}},
            {"id":b,"rect":{"x":0.5,"y":0.5,"width":0.2,"height":0.2}}
        ],"questions":[
            {"id":c,"prompt":rich("Name first."),"answer":rich("First answer"),
                "explanation":null,"maskIds":[a],"mode":"hide_target"},
            {"id":d,"prompt":rich("Name both."),"answer":rich("Both answers"),
                "explanation":rich("Explanation"),"maskIds":[a,b],"mode":"hide_all"}
        ]}),
        _ => unreachable!(),
    }
}

#[test]
fn all_seven_typed_question_families_preserve_definitions_keys_and_managed_assets() {
    let database = Database::open_in_memory().unwrap();
    database.with_connection(|connection| {
        let (deck_id, _) = seed(connection);
        connection.execute(
            "INSERT INTO assets(id,original_filename,mime_type,byte_size,relative_path,
             created_at,lifecycle_state,ready_at,verified_at,updated_at)
             VALUES(?1,'retained.png','image/png',12,?2,100,'ready',100,100,100)",
            params![id(999),format!("assets/images/{}.png", id(999))],
        ).map_err(CommandError::database)?;
        let mut notes = Vec::new();
        for index in 1..=7 {
            let note_id = id(1000+index);
            let definition = json!({"schemaVersion":1,"question":question_body(index)}).to_string();
            let question = ValidatedQuestion::parse(&definition)?;
            let text = question.search_text()?;
            connection.execute(
                "INSERT INTO notes(id,deck_id,note_type,content_format,content_schema_version,
                 question_definition,front_text,back_text,duplicate_hash,created_at,updated_at,revision)
                 VALUES(?1,?2,'interactive','question-json',1,?3,?4,?5,?6,100,100,1)",
                params![note_id,deck_id,definition,text.prompt,text.answer,question.duplicate_hash()?],
            ).map_err(CommandError::database)?;
            for (ordinal,key) in question.presentation_keys().iter().enumerate() {
                let card_id = generate_id(connection)?;
                connection.execute(
                    "INSERT INTO cards(id,note_id,deck_id,direction,template_ordinal,created_at,updated_at)
                     VALUES(?1,?2,?3,'forward',?4,100,100)",
                    params![card_id,note_id,deck_id,ordinal as i64],
                ).map_err(CommandError::database)?;
                connection.execute("INSERT INTO card_question_presentations VALUES(?1,?2,?3)", params![card_id,note_id,key]).map_err(CommandError::database)?;
                insert_new_schedule(connection, &card_id, 100)?;
            }
            for asset in question.asset_ids() {
                connection.execute("INSERT INTO note_assets VALUES(?1,?2,'content')", params![note_id,asset]).map_err(CommandError::database)?;
            }
            notes.push(note_id);
        }
        let result = save_selection_as_deck_in_connection(connection, input(9, notes.clone()))?;
        assert_eq!(result.affected_count, 7);
        assert_eq!(result.deck.generated_card_count, 8);
        for note in notes {
            let definition: String = connection.query_row("SELECT question_definition FROM notes WHERE id=?1", [&note], |row| row.get(0)).map_err(CommandError::database)?;
            let copies: i64 = connection.query_row("SELECT COUNT(*) FROM notes WHERE deck_id=?1 AND question_definition=?2", params![result.deck.id,definition], |row| row.get(0)).map_err(CommandError::database)?;
            assert_eq!(copies, 1);
        }
        assert_eq!(count(connection, "assets"), 1);
        assert_eq!(count(connection, "note_assets"), 2);
        let copied_assets: i64 = connection.query_row(
            "SELECT COUNT(*) FROM note_assets a JOIN notes n ON n.id=a.note_id WHERE n.deck_id=?1 AND a.asset_id=?2",
            params![result.deck.id,id(999)], |row| row.get(0),
        ).map_err(CommandError::database)?;
        assert_eq!(copied_assets, 1);
        assert_eq!(count(connection, "question_attempts"), 0);
        crate::database::validate_backup_lineage(connection)?;
        Ok(())
    }).unwrap();
}
