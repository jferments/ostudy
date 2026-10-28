//! Flashcards drafted from one Tutor answer. The learner asks for them, picks
//! and edits the ones to keep, and saves them as ordinary cards, so nothing
//! here writes to the library.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{ai::AiRuntimeState, error::CommandError, task_cancellation::CancellationSignal};

use super::{
    context::DocumentTaskContext,
    support::{bounded_diagnostic, output_error},
    transport::{request_json_cancellable, JsonTaskRequest},
};

pub(crate) const ANSWER_CARDS_MAX: usize = 6;
pub(crate) const ANSWER_CARD_QUESTION_MAX_CHARS: usize = 300;
pub(crate) const ANSWER_CARD_ANSWER_MAX_CHARS: usize = 600;
const ANSWER_CARDS_VISIBLE_COMPLETION_TOKENS: usize = 1_200;
const ANSWER_MAX_CHARS: usize = 8_000;

/// One drafted card: a question and the answer it expects.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnswerCardDraft {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AnswerCardsCandidate {
    cards: Vec<AnswerCardDraft>,
}

/// Ask for a few cards that hold what the answer teaches.
pub(crate) async fn request_answer_cards(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    answer: &str,
    cancellation: &CancellationSignal,
) -> Result<Vec<AnswerCardDraft>, CommandError> {
    let schema = answer_cards_schema();
    let system = answer_cards_system_prompt();
    let user = answer_cards_user_text(answer)?;
    request_json_cancellable(
        runtime,
        task,
        JsonTaskRequest {
            system_prompt: &system,
            user_text: &user,
            schema: &schema,
            max_tokens: task.completion_token_ceiling(ANSWER_CARDS_VISIBLE_COMPLETION_TOKENS),
            images: &[],
        },
        cancellation,
    )
    .await
    .and_then(decode_answer_cards)
}

fn answer_cards_system_prompt() -> String {
    format!(
        "You write flashcards for a learner from one explanation their tutor gave. Write between one and {ANSWER_CARDS_MAX} cards, fewer when the explanation holds less, each testing one idea the explanation actually teaches. A question asks about the idea itself, never about the explanation, the tutor or the learner, and does not give its answer away. An answer is short and complete on its own, using only what the explanation says. Write chemical formulas and math as plain text a card can show, such as HCO3- or x^2. Plain text only, no markup; a question at most {ANSWER_CARD_QUESTION_MAX_CHARS} characters and an answer at most {ANSWER_CARD_ANSWER_MAX_CHARS}. Return only the required structured result."
    )
}

fn answer_cards_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["cards"],
        "properties": {
            "cards": {
                "type": "array",
                "minItems": 1,
                "maxItems": ANSWER_CARDS_MAX,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["question", "answer"],
                    "properties": {
                        "question": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": ANSWER_CARD_QUESTION_MAX_CHARS
                        },
                        "answer": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": ANSWER_CARD_ANSWER_MAX_CHARS
                        }
                    }
                }
            }
        }
    })
}

fn answer_cards_user_text(answer: &str) -> Result<String, CommandError> {
    let explanation = serde_json::to_string(&json!({
        "explanation": answer.trim().chars().take(ANSWER_MAX_CHARS).collect::<String>(),
    }))
    .map_err(CommandError::serialization)?;
    Ok(format!(
        "<ostudy-tutor-answer>\n{explanation}\n</ostudy-tutor-answer>\n\nWrite the flashcards this explanation teaches."
    ))
}

fn decode_answer_cards(value: Value) -> Result<Vec<AnswerCardDraft>, CommandError> {
    let candidate: AnswerCardsCandidate = serde_json::from_value(value).map_err(|error| {
        output_error(format!(
            "The provider returned cards that did not match the required contract: {}",
            bounded_diagnostic(&error.to_string(), 320),
        ))
    })?;
    let tidy = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut cards = Vec::new();
    for card in candidate.cards.into_iter().take(ANSWER_CARDS_MAX) {
        let question = tidy(&card.question);
        let answer = tidy(&card.answer);
        if question.is_empty()
            || answer.is_empty()
            || question.chars().count() > ANSWER_CARD_QUESTION_MAX_CHARS
            || answer.chars().count() > ANSWER_CARD_ANSWER_MAX_CHARS
        {
            continue;
        }
        if cards
            .iter()
            .any(|kept: &AnswerCardDraft| kept.question == question)
        {
            continue;
        }
        cards.push(AnswerCardDraft { question, answer });
    }
    if cards.is_empty() {
        return Err(output_error("The provider returned no usable cards."));
    }
    Ok(cards)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_is_closed_and_bounded() {
        let schema = answer_cards_schema();
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(
            schema["properties"]["cards"]["maxItems"],
            json!(ANSWER_CARDS_MAX)
        );
        assert_eq!(
            schema["properties"]["cards"]["items"]["additionalProperties"],
            json!(false)
        );
    }

    #[test]
    fn cards_are_tidied_and_unusable_ones_dropped() {
        let cards = decode_answer_cards(json!({"cards": [
            {"question": " What does secretin\n act on? ", "answer": "The ductal cells."},
            {"question": "What does secretin act on?", "answer": "A repeat."},
            {"question": "   ", "answer": "Nothing asked."},
            {"question": "Which cells make enzymes?", "answer": "x".repeat(601)},
            {"question": "Which cells make enzymes?", "answer": "The acinar cells."}
        ]}))
        .unwrap();
        assert_eq!(
            cards,
            vec![
                AnswerCardDraft {
                    question: "What does secretin act on?".to_string(),
                    answer: "The ductal cells.".to_string(),
                },
                AnswerCardDraft {
                    question: "Which cells make enzymes?".to_string(),
                    answer: "The acinar cells.".to_string(),
                },
            ]
        );
        assert!(decode_answer_cards(json!({"cards": [{"question": " ", "answer": "a"}]})).is_err());
        assert!(decode_answer_cards(json!({"cards": [], "extra": 1})).is_err());
    }

    #[test]
    fn the_answer_is_sent_bounded_as_data() {
        let answer = "a".repeat(ANSWER_MAX_CHARS + 50);
        let text = answer_cards_user_text(&answer).unwrap();
        let body = text
            .strip_prefix("<ostudy-tutor-answer>\n")
            .and_then(|rest| rest.split_once("\n</ostudy-tutor-answer>"))
            .map(|(json, _)| json)
            .unwrap();
        let value: Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            value["explanation"].as_str().unwrap().chars().count(),
            ANSWER_MAX_CHARS
        );
    }
}
