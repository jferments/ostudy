use serde_json::{json, Value};

use super::*;
use crate::cards::questions::ValidatedQuestion;

fn questions() -> Vec<Value> {
    vec![
        json!({"kind":"single_choice","prompt":"Which is a prime?",
            "options":[{"id":"a","content":"2","rationale":"2 is prime."},
                       {"id":"b","content":"4","rationale":"4 is composite."}],
            "correctOptionId":"a"}),
        json!({"kind":"select_all","prompt":"Select all even numbers.",
            "options":[{"id":"a","content":"2"},{"id":"b","content":"3"},{"id":"c","content":"4"}],
            "correctOptionIds":["a","c"]}),
        json!({"kind":"true_false","statement":"Every integer is positive.","correct":false,
            "correctedStatement":"Integers can be negative, zero or positive."}),
        json!({"kind":"matching","prompt":"Match animals to their classes.",
            "pairs":[{"id":"a","left":"Cat","right":"Mammal"},
                     {"id":"b","left":"Robin","right":"Bird"}]}),
        json!({"kind":"grouping","prompt":"Group these numbers by parity.",
            "categories":[{"id":"even","content":"Even"},{"id":"odd","content":"Odd"}],
            "items":[{"id":"two","content":"2","categoryId":"even"},
                     {"id":"three","content":"3","categoryId":"odd"}]}),
        json!({"kind":"ordering","prompt":"Order these integers from smallest to largest.",
            "items":[{"id":"a","content":"1"},{"id":"b","content":"2"},{"id":"c","content":"3"}]}),
    ]
}

fn file(cards: Vec<Value>, version: i64, mode: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "format":"ostudy-flashcards","formatVersion":version,
        "deck":{"mode":mode,"name":"Typed generation"},
        "defaultTags":["generated"],"cards":cards
    }))
    .unwrap()
}

fn wrap(question: Value) -> Value {
    json!({"type":"interactive","question":question,"tags":["objective"]})
}

fn counts(connection: &Connection) -> (i64, i64, i64, i64) {
    connection
        .query_row(
            "SELECT (SELECT COUNT(*) FROM notes), (SELECT COUNT(*) FROM cards),
                    (SELECT COUNT(*) FROM card_schedules), (SELECT COUNT(*) FROM review_events)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap()
}

#[test]
fn mixed_import_preserves_all_six_typed_keys_with_fresh_native_presentations() {
    let database = Database::open_in_memory().unwrap();
    let mut cards = vec![
        json!({"type":"basic","front":"Recall question","back":"Recall answer"}),
        json!({"type":"cloze","text":"The capital is {{c1::Paris}}."}),
    ];
    cards.extend(questions().into_iter().map(wrap));
    let mut identities = Vec::new();
    let result = import_generated_flashcards_bytes_with_commit_context(
        &database,
        &file(cards, 2, "create"),
        |connection, _, context, _| {
            assert_eq!(counts(connection), (8, 8, 8, 0));
            identities = context.created_notes.clone();
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(result.created_note_count, 8);
    assert_eq!(result.created_card_count, 8);
    assert_eq!(
        identities
            .iter()
            .map(|item| item.source_index)
            .collect::<Vec<_>>(),
        (0..8).collect::<Vec<_>>()
    );
    database
        .with_connection(|connection| {
            let rows = connection
                .prepare(
                    "SELECT n.question_definition, qp.question_key, c.id, n.content_format
                     FROM notes n JOIN cards c ON c.note_id = n.id
                     JOIN card_question_presentations qp ON qp.card_id = c.id
                     WHERE n.note_type = 'interactive' ORDER BY n.id",
                )
                .unwrap()
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(rows.len(), 6);
            let mut kinds = HashSet::new();
            for (source, key, card_id, format) in rows {
                let question = ValidatedQuestion::parse(&source)?;
                kinds.insert(question.kind().as_str());
                assert_eq!(key, "main");
                assert_eq!(format, "question-json");
                assert_eq!(card_id.len(), 32);
                assert!(question.asset_ids().is_empty());
                assert!(!question.search_text()?.prompt.is_empty());
            }
            assert_eq!(kinds.len(), 6);
            let tags: i64 = connection
                .query_row("SELECT COUNT(*) FROM note_tags", [], |row| row.get(0))
                .unwrap();
            assert_eq!(tags, 14);
            let invalid: i64 = connection
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(invalid, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn any_invalid_component_rejects_the_whole_mixed_file_before_writing() {
    let database = Database::open_in_memory().unwrap();
    let mut malformed = questions();
    malformed[0]["correctOptionId"] = json!("missing");
    malformed[1]["correctOptionIds"] = json!(["a", "a"]);
    malformed[3]["pairs"][1]["id"] = json!("a");
    malformed[4]["items"][0]["categoryId"] = json!("missing");
    malformed[5]["items"][0]["content"] = json!("![remote](https://example.invalid/image)");
    for invalid in malformed
        .into_iter()
        .enumerate()
        .filter(|(index, _)| *index != 2)
    {
        let bytes = file(
            vec![
                json!({"type":"basic","front":"Valid","back":"Valid answer"}),
                wrap(invalid.1),
            ],
            2,
            "create",
        );
        assert!(import_generated_flashcards_bytes_with_commit_context(
            &database,
            &bytes,
            |_, _, _, _| Ok(())
        )
        .is_err());
        database
            .with_connection(|connection| {
                assert_eq!(counts(connection), (0, 0, 0, 0));
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn generated_occlusion_and_version_one_typed_payloads_fail_explicitly() {
    let unknown = wrap(json!({"kind":"image_occlusion","imageAssetId":"a".repeat(32)}));
    assert!(prepare_flashcard_import_bytes(&file(vec![unknown], 2, "create")).is_err());
    assert!(
        prepare_flashcard_import_bytes(&file(vec![wrap(questions()[0].clone())], 1, "create"))
            .is_err()
    );
    assert!(
        prepare_flashcard_import_bytes(&file(vec![wrap(questions()[0].clone())], 3, "create"))
            .is_err()
    );
}

#[test]
fn typed_duplicate_identity_includes_the_key_not_provider_local_ids() {
    let database = Database::open_in_memory().unwrap();
    let first = questions()[0].clone();
    let mut renamed = first.clone();
    renamed["options"][0]["id"] = json!("new_a");
    renamed["options"][1]["id"] = json!("new_b");
    renamed["correctOptionId"] = json!("new_a");
    let mut changed_key = renamed.clone();
    changed_key["correctOptionId"] = json!("new_b");
    let result = database
        .with_connection(|connection| {
            import_prepared_flashcards(
                connection,
                prepare_flashcard_import_bytes(&file(
                    vec![wrap(first), wrap(renamed.clone()), wrap(changed_key)],
                    2,
                    "create",
                ))?,
            )
        })
        .unwrap();
    assert_eq!(result.created_note_count, 2);
    assert_eq!(result.skipped_duplicate_count, 1);
    let appended = database
        .with_connection(|connection| {
            import_prepared_flashcards(
                connection,
                prepare_flashcard_import_bytes(&file(vec![wrap(renamed)], 2, "append"))?,
            )
        })
        .unwrap();
    assert_eq!(appended.created_note_count, 0);
    assert_eq!(appended.skipped_duplicate_count, 1);
}

#[test]
fn typed_publication_and_provenance_roll_back_together() {
    let database = Database::open_in_memory().unwrap();
    let result = import_generated_flashcards_bytes_with_commit_context(
        &database,
        &file(questions().into_iter().map(wrap).collect(), 2, "create"),
        |connection, _, context, _| {
            assert_eq!(context.created_notes.len(), 6);
            assert_eq!(counts(connection), (6, 6, 6, 0));
            Err(CommandError::new(
                "injected_evidence_failure",
                "Stop before commit",
            ))
        },
    );
    assert_eq!(result.unwrap_err().code, "injected_evidence_failure");
    database
        .with_connection(|connection| {
            assert_eq!(counts(connection), (0, 0, 0, 0));
            let decks: i64 = connection
                .query_row("SELECT COUNT(*) FROM decks", [], |row| row.get(0))
                .unwrap();
            assert_eq!(decks, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn every_component_has_a_stable_grounding_address_and_canonical_math() {
    for value in questions() {
        let question: question::GeneratedQuestion = serde_json::from_value(value).unwrap();
        let keys = question.evidence_keys();
        assert_eq!(keys[0], "prompt");
        assert_eq!(keys.iter().collect::<HashSet<_>>().len(), keys.len());
        assert!(keys.len() >= 2);
        question.admit().unwrap();
    }
    let mut math = questions()[0].clone();
    math["options"][0]["content"] = json!(r"\(x^2\)");
    let question: question::GeneratedQuestion = serde_json::from_value(math).unwrap();
    assert!(question
        .admit()
        .unwrap()
        .to_json()
        .unwrap()
        .contains("inlineMath"));
}
