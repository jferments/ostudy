use rusqlite::{params, Connection};
use serde_json::{json, Value};

use crate::{cards::questions::*, scheduler::insert_new_schedule};

pub(super) fn id(value: u32) -> String {
    format!("{value:032x}")
}

pub(super) fn rich(text: &str) -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]})
}

fn item(value: u32) -> RichItem {
    RichItem {
        id: id(value),
        content: rich(&format!("Synthetic item {value}")),
    }
}

fn choices() -> Vec<ChoiceOption> {
    (1..=2)
        .map(|value| ChoiceOption {
            id: id(value),
            content: rich(&format!("Synthetic choice {value}")),
            rationale: Some(rich("Retain the original rationale.")),
        })
        .collect()
}

fn definitions(asset: &str) -> Vec<(&'static str, QuestionBody)> {
    vec![
        ("single_choice", QuestionBody::SingleChoice {
            prompt: rich("Choose the first item."),
            options: choices(),
            correct_option_id: id(1),
            explanation: Some(rich("The criterion is explicit.")),
        }),
        ("select_all", QuestionBody::SelectAll {
            prompt: rich("Choose both items."),
            options: choices(),
            correct_option_ids: vec![id(1), id(2)],
            explanation: None,
        }),
        ("true_false", QuestionBody::TrueFalse {
            statement: rich("Two is less than one."),
            correct: false,
            explanation: Some(rich("Preserve the inequality and negation.")),
            corrected_statement: Some(rich("Two is not less than one.")),
        }),
        ("matching", QuestionBody::Matching {
            prompt: rich("Match the stated partners."),
            pairs: (1..=2).map(|value| MatchingPair {
                left: item(value),
                right: item(value + 2),
                rationale: None,
            }).collect(),
            explanation: None,
        }),
        ("grouping", QuestionBody::Grouping {
            prompt: rich("Place each item in its named group."),
            categories: vec![item(3), item(4)],
            items: (1..=2).map(|value| GroupedItem {
                item: item(value),
                category_id: id(value + 2),
                rationale: None,
            }).collect(),
            explanation: None,
        }),
        ("ordering", QuestionBody::Ordering {
            prompt: rich("Use ascending item numbers."),
            items: (1..=3).map(item).collect(),
            explanation: None,
        }),
        ("image_occlusion", QuestionBody::ImageOcclusion {
            image_asset_id: asset.to_string(),
            masks: vec![ImageMask {
                id: id(1),
                rect: NormalizedRect {
                    x: 0.1, y: 0.1, width: 0.2, height: 0.2,
                },
            }],
            questions: vec![OcclusionQuestion {
                id: id(10),
                prompt: rich("Identify the synthetic marked region."),
                answer: rich("The authored target."),
                explanation: None,
                mask_ids: vec![id(1)],
                mode: OcclusionMode::HideAll,
            }],
        }),
    ]
}

fn order(body: &QuestionBody) -> DisplayOrder {
    match body {
        QuestionBody::SingleChoice { options, .. }
        | QuestionBody::SelectAll { options, .. } => DisplayOrder {
            item_ids: options.iter().map(|item| item.id.clone()).collect(),
            target_ids: vec![],
        },
        QuestionBody::Matching { pairs, .. } => DisplayOrder {
            item_ids: pairs.iter().map(|pair| pair.left.id.clone()).collect(),
            target_ids: pairs.iter().map(|pair| pair.right.id.clone()).collect(),
        },
        QuestionBody::Grouping { items, categories, .. } => DisplayOrder {
            item_ids: items.iter().map(|item| item.item.id.clone()).collect(),
            target_ids: categories.iter().map(|item| item.id.clone()).collect(),
        },
        QuestionBody::Ordering { items, .. } => DisplayOrder {
            item_ids: items.iter().map(|item| item.id.clone()).collect(),
            target_ids: vec![],
        },
        _ => DisplayOrder::default(),
    }
}

pub(super) fn seed(connection: &Connection, deck: &str, asset: &str) {
    for (index, (kind, body)) in definitions(asset).into_iter().enumerate() {
        let note = id(100 + index as u32);
        let card = id(200 + index as u32);
        let instance_id = id(300 + index as u32);
        let display_order = order(&body);
        let question = ValidatedQuestion::validate(QuestionDefinition {
            schema_version: QUESTION_SCHEMA_VERSION,
            question: body,
        }).unwrap();
        let definition = question.to_json().unwrap();
        let search = question.search_text().unwrap();
        let key = question.presentation_keys()[0].clone();
        connection.execute(
            "INSERT INTO notes (id, deck_id, note_type, content_format,
                content_schema_version, question_definition, front_text, back_text,
                duplicate_hash, created_at, updated_at, revision)
             VALUES (?1, ?2, 'interactive', 'question-json', 1, ?3, ?4, ?5, ?6, 10, 10, 1)",
            params![note, deck, definition, search.prompt, search.answer, "a".repeat(64)],
        ).unwrap();
        connection.execute(
            "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal,
                created_at, updated_at) VALUES (?1, ?2, ?3, 'forward', 0, 10, 10)",
            params![card, note, deck],
        ).unwrap();
        connection.execute(
            "INSERT INTO card_question_presentations VALUES (?1, ?2, ?3)",
            params![card, note, key],
        ).unwrap();
        insert_new_schedule(connection, &card, 10).unwrap();
        let instance = QuestionInstance::capture(
            InstanceIdentity {
                instance_id: instance_id.clone(),
                note_id: note.clone(),
                card_id: card.clone(),
                note_revision: 1,
            }, question, key, display_order,
        ).unwrap();
        let mut attempt = QuestionAttempt::start(instance).unwrap();
        if index == 0 {
            attempt.mark_assisted().unwrap();
            attempt.check_answer(CheckAnswerRequest {
                check_id: id(400),
                expected_revision: attempt.revision(),
                answer: LearnerAnswer {
                    schema_version: QUESTION_SCHEMA_VERSION,
                    instance_id: instance_id.clone(),
                    response: QuestionResponse::SingleChoice {
                        selected_option_id: Some(id(1)),
                    },
                },
                response_time_ms: 1234,
            }, 30).unwrap();
        }
        connection.execute(
            "INSERT INTO question_attempts (instance_id, card_id, note_id,
                question_kind, instance_hash, revision, phase, assisted,
                checkpoint_json, first_checked_at, verdict, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 20, 30)",
            params![instance_id, card, note, kind, attempt.instance().fingerprint(),
                attempt.revision(), if index == 0 { "checked" } else { "answering" },
                index == 0, attempt.to_json().unwrap(),
                (index == 0).then_some(30), (index == 0).then_some("correct")],
        ).unwrap();
        if kind == "image_occlusion" {
            connection.execute(
                "INSERT INTO note_assets VALUES (?1, ?2, 'content')",
                params![note, asset],
            ).unwrap();
            connection.execute(
                "INSERT INTO question_attempt_assets VALUES (?1, ?2)",
                params![instance_id, asset],
            ).unwrap();
        }
    }
}
