use std::collections::HashSet;

use serde_json::{json, Value};

use crate::{document_scale::CURATION_VISIBLE_COMPLETION_TOKENS, error::CommandError};

use super::super::AiRuntimeState;
use super::{
    context::DocumentTaskContext,
    contracts::{parse_curation, DocumentOutline, MAX_CURATE_CARDS_PER_BATCH},
    outline::outline_context_for_curation,
    schemas::curation_schema,
    support::{bounded_diagnostic, collapse_contract_error, retryable_contract_error},
    transport::{request_json, JsonTaskRequest},
};

const CURATION_PROMPT_VERSION: i64 = 3;
const CURATION_SCHEMA_VERSION: i64 = 1;

pub(crate) async fn curate_document_cards(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    outline: &DocumentOutline,
    cards: Vec<Value>,
) -> Result<Vec<Value>, CommandError> {
    if cards.is_empty() {
        return Ok(cards);
    }

    let outline_json = outline_context_for_curation(outline)?;
    let batches = into_curation_batches(cards);
    let parallelism = crate::ai::concurrency::document_request_parallelism(&task.network_scope)
        .clamp(1, batches.len().max(1));
    crate::logging::debug_target(
        "ai.document.curation",
        "document_curation_execution_plan",
        "Prepared bounded card-curation batches without logging generated card content.",
        json!({
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "batches": batches.len(),
            "parallelism": parallelism,
            "candidates": batches.iter().map(Vec::len).sum::<usize>(),
        }),
    );

    let mut retained = Vec::new();
    let mut cursor = 0_usize;
    while cursor < batches.len() {
        let end = (cursor + parallelism).min(batches.len());
        let indices = (cursor..end).collect::<Vec<_>>();
        let p0 = indices.first().copied();
        let p1 = indices.get(1).copied();
        let p2 = indices.get(2).copied();
        let p3 = indices.get(3).copied();
        let (r0, r1, r2, r3) = tokio::join!(
            curation_slot(runtime, task, &outline_json, &batches, p0),
            curation_slot(runtime, task, &outline_json, &batches, p1),
            curation_slot(runtime, task, &outline_json, &batches, p2),
            curation_slot(runtime, task, &outline_json, &batches, p3),
        );

        for (batch_index, result) in [r0, r1, r2, r3].into_iter().flatten() {
            let batch = &batches[batch_index];
            match result {
                Ok(keep) => retained.extend(
                    keep.into_iter()
                        .filter_map(|index| batch.get(index).cloned()),
                ),
                Err(error) => {
                    crate::logging::warn_target(
                        "ai.document.curation",
                        "document_curation_batch_skipped",
                        "A bounded curation batch failed; validated candidates from that batch were retained rather than discarded.",
                        json!({
                            "batch": batch_index + 1,
                            "candidates": batch.len(),
                            "code": error.code.as_str(),
                            "detail": bounded_diagnostic(&error.message, 400),
                        }),
                    );
                    retained.extend(batch.iter().cloned());
                }
            }
        }
        cursor = end;
    }

    let mut seen = HashSet::new();
    retained.retain(|card| seen.insert(card_identity(card)));
    Ok(retained)
}

fn into_curation_batches(cards: Vec<Value>) -> Vec<Vec<Value>> {
    let mut cards = cards.into_iter();
    let mut batches = Vec::new();
    loop {
        let batch = cards
            .by_ref()
            .take(MAX_CURATE_CARDS_PER_BATCH)
            .collect::<Vec<_>>();
        if batch.is_empty() {
            break;
        }
        batches.push(batch);
    }
    batches
}

async fn curation_slot(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    outline_json: &str,
    batches: &[Vec<Value>],
    batch_index: Option<usize>,
) -> Option<(usize, Result<Vec<usize>, CommandError>)> {
    let batch_index = batch_index?;
    let batch = batches.get(batch_index)?;
    let summaries = batch
        .iter()
        .enumerate()
        .map(|(index, card)| compact_card_summary(index, card))
        .collect::<Vec<_>>();
    let user = format!(
        "Global document outline:\n<ostudy-global-outline>\n{outline_json}\n</ostudy-global-outline>\n\nCandidate flashcards:\n<ostudy-candidates>\n{}\n</ostudy-candidates>\n\nReturn the zero-based indices of candidates that should remain. Keep strong coverage of major learning objectives and a useful mix of recall and application. QUALITY GATE: reject a candidate unless its answer directly and completely answers its question and is supported by the document context. Reject question/answer mismatches, incomplete answers to multi-part questions, answers that are only headings or topic labels, unsupported numerical claims, reversed question/answer direction, vague prompts whose answer is not uniquely determined, and cards that require access to the original source to make sense. For definitions, keep only genuine term/definition pairs. Also remove globally minor or hyperspecific trivia, bibliography/citation artifacts, near-duplicates, and redundant cards that test the same knowledge in essentially the same way. When uncertain whether a candidate is semantically correct, drop it. Do not rewrite cards and do not invent replacements.",
        summaries.join("\n")
    );
    Some((
        batch_index,
        request_curation_with_retry(runtime, task, &user, batch.len()).await,
    ))
}

async fn request_curation_with_retry(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    user: &str,
    candidate_count: usize,
) -> Result<Vec<usize>, CommandError> {
    let system = format!(
        "You are the final educational quality editor for ostudy. Select, but never rewrite, generated flashcards. A card is eligible only when its question and answer are mutually coherent, directly matched, complete for what was asked, and supported by the supplied document context. Reject answer headings or topic labels masquerading as answers, unsupported claims, incomplete multi-part answers, ambiguous prompts, reversed direction, and other semantic mismatches. Then optimize global learning-objective coverage and remove low-value or redundant material. Prefer dropping a questionable card over preserving a misleading one. Return only the required JSON. Curation prompt version: {CURATION_PROMPT_VERSION}. Curation schema version: {CURATION_SCHEMA_VERSION}."
    );
    let schema = curation_schema(candidate_count);
    let first = request_json(
        runtime,
        task,
        JsonTaskRequest {
            system_prompt: &system,
            user_text: user,
            schema: &schema,
            max_tokens: task.completion_token_ceiling(CURATION_VISIBLE_COMPLETION_TOKENS),
            images: &[],
        },
    )
    .await
    .and_then(|value| parse_curation(value, candidate_count));

    match first {
        Ok(indices) => Ok(indices),
        Err(error) if retryable_contract_error(&error) => {
            let repaired = format!(
                "{user}\n\nThe prior response was invalid. Return a fresh JSON object containing only valid candidate indices."
            );
            request_json(
                runtime,
                task,
                JsonTaskRequest {
                    system_prompt: &system,
                    user_text: &repaired,
                    schema: &schema,
                    max_tokens: task.completion_token_ceiling(
                        CURATION_VISIBLE_COMPLETION_TOKENS.saturating_mul(2),
                    ),
                    images: &[],
                },
            )
            .await
            .and_then(|value| parse_curation(value, candidate_count))
            .map_err(|retry_error| collapse_contract_error(retry_error, "card curation"))
        }
        Err(error) => Err(error),
    }
}

fn compact_card_summary(index: usize, card: &Value) -> String {
    let card_type = card
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let prompt = card
        .get("front")
        .or_else(|| card.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let answer = card.get("back").and_then(Value::as_str).unwrap_or("");
    let tags = card
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .take(8)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    format!(
        "{index}. type={card_type}; prompt={}; answer={}; tags={}",
        bounded_diagnostic(prompt, 800),
        bounded_diagnostic(answer, 800),
        bounded_diagnostic(&tags, 300),
    )
}

fn card_identity(card: &Value) -> String {
    let card_type = card.get("type").and_then(Value::as_str).unwrap_or("");
    let front = card
        .get("front")
        .or_else(|| card.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let back = card.get("back").and_then(Value::as_str).unwrap_or("");
    format!(
        "{}|{}|{}",
        card_type.trim().to_ascii_lowercase(),
        normalize_identity_text(front),
        normalize_identity_text(back),
    )
}

fn normalize_identity_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curation_batches_are_bounded_without_losing_candidates() {
        let cards = (0..70)
            .map(|index| {
                json!({
                    "type": "basic",
                    "front": format!("Question {index}"),
                    "back": format!("Answer {index}"),
                    "tags": []
                })
            })
            .collect::<Vec<_>>();
        let batches = into_curation_batches(cards);
        assert_eq!(
            batches.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![32, 32, 6]
        );
    }

    #[test]
    fn identity_normalizes_case_and_whitespace() {
        let first =
            json!({"type":"basic", "front":"  Cardiac   Output ", "back":"Flow per minute"});
        let second = json!({"type":"basic", "front":"cardiac output", "back":"flow per minute"});
        assert_eq!(card_identity(&first), card_identity(&second));
    }

    #[test]
    fn compact_summary_is_bounded() {
        let card = json!({
            "type": "basic",
            "front": "q".repeat(2_000),
            "back": "a".repeat(2_000),
            "tags": []
        });
        assert!(compact_card_summary(0, &card).len() < 1_700);
    }
}
