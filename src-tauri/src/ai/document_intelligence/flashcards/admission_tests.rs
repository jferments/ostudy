//! Exercise the actual generation parser and public importer, not a permissive
//! replacement validator. Synthetic evidence IDs test contracts, not entailment.

use super::*;
use crate::{
    authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit_context,
    document_card_topics::{link_imported_notes_to_topics, SourceGroundedGeneratedCard},
    knowledge::{
        assessment::policy::QuestionFamily,
        authoring::{tests::catalog_fixture, without_card_support},
    },
};

fn recall(id: &str, front: &str) -> Value {
    json!({"card":{"type":"basic","front":front,"back":"Three sides."},
        "knowledgeUnitIds":[id]})
}

fn choice(id: &str) -> Value {
    json!({"card":{"type":"interactive","question":{
    "kind":"single_choice","prompt":"Which shape has three sides?",
    "options":[{"id":"triangle","content":"Triangle"},
        {"id":"square","content":"Square","rationale":"A square has four sides."}],
    "correctOptionId":"triangle"}},
    "assessment":{"criterion":"Number of straight sides","ambiguous":false,
    "components":[
        {"key":"option:triangle","stance":"supports","knowledgeUnitIds":[id]},
        {"key":"option:square","stance":"contradicts","knowledgeUnitIds":[id]}
    ]}})
}

fn retained_next_to(bad: Value) -> Vec<Value> {
    let catalog = catalog_fixture();
    parse_knowledge_cards(
        json!({"cards":[bad, recall(&catalog.units[0].id, "Retained question?")]}),
        2,
        &catalog,
    )
    .expect("one rejected candidate must not discard an admissible sibling")
}

#[test]
fn malformed_envelopes_and_assessment_shapes_are_rejected_individually() {
    let id = "a".repeat(32);
    let mut wrong_evidence_type = choice(&id);
    wrong_evidence_type["assessment"]["components"][0]["knowledgeUnitIds"] = json!(42);
    let mut unknown_stance = choice(&id);
    unknown_stance["assessment"]["components"][0]["stance"] = json!("UNKNOWN_PRIVATE_VALUE");
    let mut unexpected = recall(&id, "PRIVATE_SENTINEL");
    unexpected["unexpected"] = json!("PRIVATE_SENTINEL");
    for bad in [
        Value::Null,
        json!(7),
        json!([]),
        json!({}),
        json!({"card":false,"knowledgeUnitIds":[id]}),
        json!({"card":{"type":"basic"},"knowledgeUnitIds":[3]}),
        wrong_evidence_type,
        unknown_stance,
        unexpected,
    ] {
        let cards = retained_next_to(bad);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0]["front"], "Retained question?");
    }
}

#[test]
fn native_question_errors_do_not_bypass_per_card_rejection() {
    let id = "a".repeat(32);
    let good = choice(&id);
    let mut unknown_key = good.clone();
    unknown_key["card"]["question"]["correctOptionId"] = json!("absent");
    let mut duplicate_key = good.clone();
    duplicate_key["card"]["question"]["options"][1]["id"] = json!("triangle");
    let mut remote_image = good.clone();
    remote_image["card"]["question"]["prompt"] = json!("![private](https://example.invalid/p)");
    let mut missing_binding = good.clone();
    missing_binding["assessment"]["components"]
        .as_array_mut()
        .unwrap()
        .pop();
    let mut wrong_role = good.clone();
    wrong_role["assessment"]["components"][1]["stance"] = json!("supports");
    let mut invented_evidence = good.clone();
    invented_evidence["assessment"]["components"][0]["knowledgeUnitIds"] = json!(["f".repeat(32)]);
    let mut optional_context_is_not_trusted = good.clone();
    optional_context_is_not_trusted["assessment"]["components"]
        .as_array_mut()
        .unwrap()
        .push(json!({"key":"prompt","stance":"supports","knowledgeUnitIds":["f".repeat(32)]}));
    let mut missing_rationale = good.clone();
    missing_rationale["card"]["question"]["options"][1]["rationale"] = Value::Null;
    let mut unknown_kind = good;
    unknown_kind["card"]["question"]["kind"] = json!("image_occlusion");
    for bad in [
        unknown_key,
        duplicate_key,
        remote_image,
        missing_binding,
        wrong_role,
        invented_evidence,
        optional_context_is_not_trusted,
        missing_rationale,
        unknown_kind,
    ] {
        assert_eq!(retained_next_to(bad).len(), 1);
    }
}

#[test]
fn recall_is_prepared_by_the_final_import_validator_before_selection() {
    let id = "a".repeat(32);
    // The legacy generation text normalizer accepts nonempty strings. The public
    // importer rejects remote images and unclosed Markdown: catch that HERE.
    for bad in [
        recall(&id, "![private](https://example.invalid/image)"),
        recall(&id, "```unclosed"),
    ] {
        assert_eq!(retained_next_to(bad).len(), 1);
    }
}

#[test]
fn disabled_recall_or_interactive_families_do_not_poison_enabled_siblings() {
    let catalog = catalog_fixture();
    let id = &catalog.units[0].id;
    for (family, expected) in [
        (QuestionFamily::Recall, "basic"),
        (QuestionFamily::SingleChoice, "interactive"),
    ] {
        let policy = GenerationPolicy {
            allowed_types: vec![family],
            ..GenerationPolicy::default()
        };
        let cards = parse_knowledge_cards_with_policy(
            json!({"cards":[recall(id, "Recall?"), choice(id)]}),
            2,
            &catalog,
            &policy,
        )
        .unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0]["type"], expected);
    }
    let invalid_policy = GenerationPolicy {
        allowed_types: vec![],
        ..GenerationPolicy::default()
    };
    assert!(parse_knowledge_cards_with_policy(
        json!({"cards":[recall(id, "Recall?")]}),
        2,
        &catalog,
        &invalid_policy,
    )
    .is_err());
}

#[test]
fn rejected_envelopes_cannot_shift_source_associations_or_reorder_card_content() {
    let mut catalog = catalog_fixture();
    let mut second = catalog.units[0].clone();
    second.id = "b".repeat(32);
    catalog.units.push(second);
    let cards = parse_knowledge_cards(
        json!({"cards":[
            recall(&"a".repeat(32), "First retained?"),
            {"assessment":{"components":"wrong type"}},
            recall(&"b".repeat(32), "Second retained?")
        ]}),
        4,
        &catalog,
    )
    .unwrap();
    assert_eq!(cards.len(), 2);
    for (front, unit) in [("First retained?", "a"), ("Second retained?", "b")] {
        let card = cards.iter().find(|card| card["front"] == front).unwrap();
        assert_eq!(
            authoring::card_support(card).unwrap()[0].unit_id,
            unit.repeat(32)
        );
        assert_eq!(card["back"], "Three sides.");
    }
}

#[test]
fn response_bounds_and_all_invalid_are_not_misreported_as_empty_success() {
    let catalog = catalog_fixture();
    for value in [
        json!(null),
        json!([]),
        json!({}),
        json!({"cards":null}),
        json!({"cards":[],"extra":true}),
        json!({"cards":vec![Value::Null;129]}),
    ] {
        assert!(parse_knowledge_cards(value, 16, &catalog).is_err());
    }
    let error = parse_knowledge_cards(
        json!({"cards":[
            {"card":{"type":"PRIVATE_SENTINEL"}, "knowledgeUnitIds":["a".repeat(32)]},
            null
        ]}),
        2,
        &catalog,
    )
    .unwrap_err();
    assert!(retryable_contract_error(&error));
    assert!(!error.message.contains("PRIVATE_SENTINEL"));
    assert!(parse_knowledge_cards(json!({"cards":[]}), 2, &catalog)
        .unwrap()
        .is_empty());
}

#[test]
fn mixed_provider_output_publishes_only_retained_cards_with_exact_evidence() {
    let (database, source, catalog, plan) = authoring::tests::fixture();
    let cards = parse_knowledge_cards(
        json!({"cards":[
            null, recall(&catalog.units[0].id, "Number of triangle sides?"),
            {"card":{"type":"basic","front":"","back":""}},
            choice(&catalog.units[0].id)
        ]}),
        4,
        &catalog,
    )
    .unwrap();
    assert_eq!(cards.len(), 2);
    let grounded = cards
        .iter()
        .map(|card| SourceGroundedGeneratedCard {
            card: without_card_support(card.clone()),
            source_locator: source.locator.clone().unwrap(),
            knowledge_support: authoring::card_support(card).unwrap(),
            assessment: assessment::record_from_card(card).unwrap(),
        })
        .collect::<Vec<_>>();
    let payload = json!({"format":"ostudy-flashcards","formatVersion":2,
        "deck":{"mode":"create","name":"Admission regression"},
        "cards":grounded.iter().map(|card| &card.card).collect::<Vec<_>>()});
    let result = import_generated_flashcards_bytes_with_commit_context(
        &database,
        payload.to_string().as_bytes(),
        |connection, _, committed, now| {
            assert_eq!(committed.created_notes.len(), 2);
            link_imported_notes_to_topics(
                connection,
                &source.owner_id,
                &"3".repeat(32),
                &plan,
                &grounded,
                committed,
                now,
            )
        },
    )
    .unwrap();
    assert_eq!(result.created_note_count, 2);
    assert_eq!(result.created_card_count, 2);
    database
        .with_connection(|connection| {
            let counts: (i64, i64, i64) = connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM question_generation_evidence),
                            (SELECT COUNT(*) FROM card_schedules),
                            (SELECT COUNT(*) FROM review_events)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(counts, (1, 2, 0));
            Ok(())
        })
        .unwrap();
}

#[test]
fn public_file_import_remains_whole_file_atomic() {
    let database = crate::database::Database::open_in_memory().unwrap();
    let payload = json!({"format":"ostudy-flashcards","formatVersion":2,
    "deck":{"mode":"create","name":"Must not publish"},"cards":[
        {"type":"basic","front":"Valid?","back":"Valid answer."},
        {"type":"basic","front":"![bad](https://example.invalid/x)","back":"No."}
    ]});
    let result = import_generated_flashcards_bytes_with_commit_context(
        &database,
        payload.to_string().as_bytes(),
        |_, _, _, _| Ok(()),
    );
    assert!(result.is_err());
    database
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn cached_cards_cannot_bypass_native_content_preparation() {
    let catalog = catalog_fixture();
    let policy = GenerationPolicy::default();
    let cards = parse_knowledge_cards(
        json!({"cards":[recall(&catalog.units[0].id, "Sides?"), choice(&catalog.units[0].id)]}),
        4,
        &catalog,
    )
    .unwrap();
    for card in &cards {
        assert!(assessment::validates_checkpoint(card, &catalog, &policy));
    }
    let mut broken = cards
        .iter()
        .find(|card| card["type"] == "basic")
        .unwrap()
        .clone();
    broken["front"] = json!("![bad](https://example.invalid/cached)");
    assert!(!assessment::validates_checkpoint(
        &broken, &catalog, &policy
    ));
}

#[path = "family_tests.rs"]
mod families;
