//! Replay model-shaped values through generation, evidence, cache and publication.
//! Fixture source IDs are synthetic; these tests make no entailment claim.

use super::*;

fn family_envelopes(unit_id: &str) -> Vec<Value> {
    let mut values: Vec<Value> = serde_json::from_str(include_str!("families.json")).unwrap();
    for value in &mut values {
        for component in value["assessment"]["components"].as_array_mut().unwrap() {
            component["knowledgeUnitIds"] = json!([unit_id]);
        }
    }
    values
}

#[test]
fn document_all_six_families_retain_valid_siblings_and_reject_missing_graded_evidence() {
    let catalog = catalog_fixture();
    let policy = GenerationPolicy::default();
    for good in family_envelopes(&catalog.units[0].id) {
        let mut rule = "unobserved";
        let expected = admit_knowledge_card(good.clone(), &catalog, &policy, &mut rule).unwrap();
        assert_eq!(rule, "assessment_admission");
        let mut missing = good.clone();
        missing["assessment"]["components"]
            .as_array_mut()
            .unwrap()
            .pop();
        let rejected = parse_knowledge_cards(json!({"cards":[missing.clone()]}), 4, &catalog);
        assert!(rejected.is_err());
        let cards =
            parse_knowledge_cards(json!({"cards":[missing, null, good.clone()]}), 4, &catalog)
                .unwrap();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0], expected);
        let record = assessment::record_from_card(&cards[0]).unwrap().unwrap();
        record.validate_card(&cards[0]).unwrap();
        assert!(assessment::validates_checkpoint(
            &cards[0], &catalog, &policy
        ));
        assert_eq!(
            authoring::card_support(&cards[0]).unwrap(),
            record.support().unwrap()
        );
    }
}

#[test]
fn document_all_six_families_publish_after_rejected_envelopes_without_shifted_evidence() {
    let (database, source, catalog, plan) = authoring::tests::fixture();
    let mut values = Vec::new();
    for value in family_envelopes(&catalog.units[0].id) {
        values.push(json!({"card":false,"assessment":{"components":42}}));
        values.push(value);
    }
    let cards = parse_knowledge_cards(json!({"cards":values}), 16, &catalog).unwrap();
    assert_eq!(cards.len(), 6);
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
        "deck":{"mode":"create","name":"Six family admission"},
        "cards":grounded.iter().map(|card| &card.card).collect::<Vec<_>>()});
    let result = import_generated_flashcards_bytes_with_commit_context(
        &database,
        payload.to_string().as_bytes(),
        |connection, _, committed, now| {
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
    assert_eq!(result.created_note_count, 6);
    assert_eq!(result.created_card_count, 6);
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
            assert_eq!(counts, (6, 6, 0));
            Ok(())
        })
        .unwrap();
}
