//! A question for a card made from a learner's mark. The learner asks for it
//! and edits it before saving, so nothing here writes to the library.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::{ai::AiRuntimeState, error::CommandError, task_cancellation::CancellationSignal};

use super::{
    context::DocumentTaskContext,
    support::{bounded_diagnostic, output_error},
    transport::{request_json_cancellable, JsonTaskRequest},
};

pub(crate) const MARK_QUESTION_MAX_CHARS: usize = 300;
const MARK_QUESTION_VISIBLE_COMPLETION_TOKENS: usize = 200;
const MARK_TITLE_MAX_CHARS: usize = 200;
const MARK_WORDS_MAX_CHARS: usize = 2_000;

/// The marked words a question is asked about, and where they were marked.
pub(crate) struct MarkQuestionSource<'a> {
    pub document_title: &'a str,
    pub page_number: i64,
    pub words: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MarkQuestionCandidate {
    question: String,
}

/// Ask for one question whose answer is the marked words.
pub(crate) async fn request_mark_question(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    source: &MarkQuestionSource<'_>,
    cancellation: &CancellationSignal,
) -> Result<String, CommandError> {
    let schema = mark_question_schema();
    let system = mark_question_system_prompt();
    let user = mark_question_user_text(source)?;
    request_json_cancellable(
        runtime,
        task,
        JsonTaskRequest {
            system_prompt: &system,
            user_text: &user,
            schema: &schema,
            max_tokens: task.completion_token_ceiling(MARK_QUESTION_VISIBLE_COMPLETION_TOKENS),
            images: &[],
        },
        cancellation,
    )
    .await
    .and_then(decode_mark_question)
}

fn mark_question_system_prompt() -> String {
    format!(
        "You write one flashcard question for a learner. While reading, the learner marked a passage, and the card's answer is exactly the marked words. Write a single clear question that those words answer, using only what they say. Ask about the idea itself, never about the document, the page, the mark or the learner. Do not repeat the answer's key terms in a way that gives the answer away. Plain text only, no markup, at most {MARK_QUESTION_MAX_CHARS} characters. Return only the required structured result."
    )
}

fn mark_question_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["question"],
        "properties": {
            "question": {
                "type": "string",
                "minLength": 1,
                "maxLength": MARK_QUESTION_MAX_CHARS
            }
        }
    })
}

fn mark_question_user_text(source: &MarkQuestionSource<'_>) -> Result<String, CommandError> {
    let mark = serde_json::to_string(&json!({
        "document": truncate_chars(source.document_title.trim(), MARK_TITLE_MAX_CHARS),
        "page": source.page_number,
        "markedWords": truncate_chars(source.words.trim(), MARK_WORDS_MAX_CHARS),
    }))
    .map_err(CommandError::serialization)?;
    Ok(format!(
        "<ostudy-learner-mark>\n{mark}\n</ostudy-learner-mark>\n\nWrite the question these marked words answer."
    ))
}

fn decode_mark_question(value: Value) -> Result<String, CommandError> {
    let candidate: MarkQuestionCandidate = serde_json::from_value(value).map_err(|error| {
        output_error(format!(
            "The provider returned a question that did not match the required contract: {}",
            bounded_diagnostic(&error.to_string(), 320),
        ))
    })?;
    let question = candidate
        .question
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if question.is_empty() || question.chars().count() > MARK_QUESTION_MAX_CHARS {
        return Err(output_error(
            "The provider returned an empty or overlong question.",
        ));
    }
    Ok(question)
}

fn truncate_chars(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_is_closed_and_bounded() {
        let schema = mark_question_schema();
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(
            schema["properties"]["question"]["maxLength"],
            json!(MARK_QUESTION_MAX_CHARS)
        );
    }

    #[test]
    fn the_question_is_tidied_and_must_say_something() {
        assert_eq!(
            decode_mark_question(json!({"question": "  What sets\n the heart rate? "})).unwrap(),
            "What sets the heart rate?"
        );
        assert!(decode_mark_question(json!({"question": "   "})).is_err());
        assert!(decode_mark_question(json!({"question": "x".repeat(301)})).is_err());
        assert!(decode_mark_question(json!({"question": "Why?", "answer": "No"})).is_err());
    }

    #[test]
    fn the_mark_is_sent_bounded_as_data() {
        let words = "w".repeat(MARK_WORDS_MAX_CHARS + 50);
        let text = mark_question_user_text(&MarkQuestionSource {
            document_title: " Cardiac physiology ",
            page_number: 4,
            words: &words,
        })
        .unwrap();
        let body = text
            .strip_prefix("<ostudy-learner-mark>\n")
            .and_then(|rest| rest.split_once("\n</ostudy-learner-mark>"))
            .map(|(json, _)| json)
            .unwrap();
        let mark: Value = serde_json::from_str(body).unwrap();
        assert_eq!(mark["document"], "Cardiac physiology");
        assert_eq!(mark["page"], 4);
        assert_eq!(
            mark["markedWords"].as_str().unwrap().chars().count(),
            MARK_WORDS_MAX_CHARS
        );
    }
}
