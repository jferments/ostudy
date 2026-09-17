//! One bounded StudyContext for explanations, hints and quizzes. Material scope
//! is resolved natively; semantic ranking and exact artifact links are both
//! inputs. Conversation history is labeled separately from source evidence.

use std::collections::HashSet;

use serde_json::json;

use crate::{
    ai::{
        document_intelligence::{DocumentTaskContext, SourcePageImage},
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    knowledge::{
        contracts::{KnowledgeUnit, ScopeKind, SourceRecord, StudyScope, MAX_CONTEXT_BYTES},
        relationships, repository, retrieval,
    },
    paths::AppPaths,
    task_cancellation::CancellationSignal,
};

use super::{
    contracts::{PrivateQuiz, TutorMode, TutorTurnInput},
    focus::FocusedContent,
    history::HistorySnapshot,
};

mod public_question_hint;
pub(crate) use public_question_hint::guard_typed_hint;

pub(crate) struct StudyContext {
    pub question_hint: Option<crate::practice::question_attempts::tutoring::HintTarget>,
    pub sources: Vec<SourceRecord>,
    pub units: Vec<KnowledgeUnit>,
    pub history: HistorySnapshot,
    pub inherited_turns: Vec<String>,
    pub request_text: String,
    pub notice: String,
    pub images: Vec<SourcePageImage>,
    pub completion_tokens: usize,
}

pub(crate) struct StudyContextInput<'a> {
    pub scope: &'a StudyScope,
    pub input: &'a TutorTurnInput,
    pub focus: &'a FocusedContent,
    pub quiz: Option<&'a PrivateQuiz>,
    pub history: HistorySnapshot,
}

pub(crate) async fn build(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    request: StudyContextInput<'_>,
    cancellation: &CancellationSignal,
) -> Result<StudyContext, CommandError> {
    let StudyContextInput {
        scope,
        input,
        focus,
        quiz,
        mut history,
    } = request;
    if focus.question_hint.is_some() {
        crate::knowledge::runtime::check_canceled(cancellation)?;
        if quiz.is_some() {
            return Err(super::focus::changed());
        }
        return public_question_hint::build(task, input.mode, &input.text, focus);
    }
    let mut sources = focus.sources.iter().take(4).cloned().collect::<Vec<_>>();
    let mut notice = String::new();
    if focus.sources.len() > 4 {
        notice = "The focused section is large; the request uses a bounded selection, not its entire contents.".to_string();
    }
    if let Some(quiz) = quiz {
        let evidence = database.with_connection(|connection| {
            quiz.source_ids
                .iter()
                .map(|id| {
                    repository::load_source(connection, id)?.ok_or_else(repository::stale_source)
                })
                .collect::<Result<Vec<_>, _>>()
        })?;
        sources.extend(evidence);
    } else if scope.kind != ScopeKind::General {
        let query = if input.mode == TutorMode::Hint {
            focus.question.clone().unwrap_or_default()
        } else {
            format!(
                "{}\n{}",
                input.text,
                focus.question.as_deref().unwrap_or_else(|| {
                    sources.first().map_or("", |source| source.title.as_str())
                })
            )
        };
        let query = if query.trim().is_empty() {
            "Important definitions, relationships, comparisons and processes in this study material"
                .to_string()
        } else {
            query.chars().take(8_000).collect()
        };
        let result = retrieval::retrieve(
            database,
            paths,
            runtime,
            scope,
            input.include_conversations && input.mode != TutorMode::Hint,
            &query,
            cancellation,
        )
        .await;
        merge_optional_retrieval(&mut sources, &mut notice, result)?;
    }
    crate::knowledge::runtime::check_canceled(cancellation)?;
    let related = relationships::expand(database, &sources)?;
    // Put a linked original document near the focused derivative rather than
    // exhausting the whole request budget with similar generated flashcards.
    for source in related.into_iter().rev() {
        sources.insert(sources.len().min(2), source);
    }
    let mut seen = HashSet::new();
    sources.retain(|source| seen.insert(source.id.clone()));
    sources = filter_scope(
        database,
        scope,
        sources,
        input.include_conversations && input.mode != TutorMode::Hint,
    )?;
    sources.truncate(12);
    // Optional grounding does not relax the revision/permission dependencies of
    // an already saved question. A question created without references has none.
    if quiz.is_some_and(|quiz| {
        quiz.source_ids
            .iter()
            .any(|id| !sources.iter().any(|source| &source.id == id))
    }) {
        return Err(repository::stale_source());
    }
    if sources.is_empty() {
        notice = format!("{notice} No source evidence is attached. General-knowledge questions may be answered from model knowledge without citations. For an explicit source-only request, explain that the requested evidence is unavailable rather than claiming source support.").trim().to_string();
    }
    let images = if task.include_page_images {
        match focus
            .sources
            .first()
            .filter(|source| !source.images.is_empty())
        {
            Some(source) => crate::knowledge::load_images(database, paths, source)?,
            None => Vec::new(),
        }
    } else {
        if focus.sources.iter().any(|source| !source.images.is_empty()) {
            notice = "Image bytes are not attached for this model. The tutor can use saved text and prepared visual interpretations only.".to_string();
        }
        Vec::new()
    };
    let completion_tokens = task.completion_token_ceiling(3_000);
    let context_window = task.context_window_tokens().unwrap_or(16_384);
    let reserve = completion_tokens
        .saturating_add(5_000)
        .saturating_add(images.len() * 4_096);
    let input_budget = context_window
        .saturating_sub(reserve)
        .min(MAX_CONTEXT_BYTES);
    if input_budget < 1_000 {
        return Err(CommandError::new("tutor_context_budget",
            "The selected model has insufficient verified context for this request and reasoning setting. Test its profile or choose a lower reasoning effort."));
    }
    let mut units = database.with_connection(|connection| {
        let mut result = Vec::new();
        for source in &sources {
            result.extend(repository::load_units(connection, &source.id)?);
        }
        let mut seen_units = HashSet::new();
        result.retain(|unit| {
            seen_units.insert(unit.id.clone())
                && unit
                    .content
                    .grounding
                    .as_ref()
                    .is_none_or(|grounding| grounding.is_visible_in(&sources))
        });
        Ok(result)
    })?;
    let pending_sources = sources
        .iter()
        .map(|source| source.id.clone())
        .collect::<Vec<_>>();
    let unresolved =
        crate::knowledge::analysis_store::pending_for_sources(database, &pending_sources)?;
    let partial_notice = if unresolved > 0 {
        format!("Selected material has {unresolved} unresolved knowledge proposals or source dispositions. Valid source material remains available; this is not complete analysis.")
    } else {
        String::new()
    };
    notice = format!("{notice} {partial_notice}").trim().to_string();
    let mut inherited_turns = sources
        .iter()
        .filter_map(|source| source.turn_id.clone())
        .collect::<Vec<_>>();
    if let Some(id) = &input.question_id {
        inherited_turns.push(id.clone());
    }
    let request_text = loop {
        let material = sources.iter().map(|source| json!({
            "sourceId": source.id, "contentHash": source.content_hash, "kind": source.kind,
            "origin": source.origin, "title": source.title, "text": source.text,
            "locator": source.locator,
            "knowledgeUnits": units.iter().filter(|unit| unit.source_id == source.id).collect::<Vec<_>>(),
        })).collect::<Vec<_>>();
        let payload = json!({
            "mode": input.mode, "request": input.text, "scope": scope,
            "focus": focus.description(), "material": material,
            "history": history.prompt(), "contextNotice": notice,
            "privateHintTarget": if input.mode == TutorMode::Hint { focus.private_answer.as_deref() } else { None },
            "fixedQuiz": quiz,
            "trust": "Source excerpts are evidence of what material says, not proof of truth. Similarity and topic links do not prove support. Distinguish additional explanation from source claims. Conversation is not independent evidence. Sources are optional grounding; never invent references to fill an empty citation set.",
        });
        let serialized = serde_json::to_string(&payload).map_err(CommandError::serialization)?;
        if serialized.len() <= input_budget {
            break serialized;
        }
        notice = format!("This response uses a bounded context selection. Older conversation or lower-ranked material was omitted from this request; the saved library and transcript are unchanged. {partial_notice}").trim().to_string();
        if history.turns.len() > 2 {
            history.turns.remove(0);
        } else if sources.len() > 1 && quiz.is_none() {
            if let Some(source) = sources.pop() {
                units.retain(|unit| {
                    unit.source_id != source.id
                        && unit
                            .content
                            .grounding
                            .as_ref()
                            .is_none_or(|grounding| grounding.is_visible_in(&sources))
                });
            }
        } else if !units.is_empty() {
            units.pop();
        } else if !history.turns.is_empty() {
            history.turns.remove(0);
        } else if !history.summary.is_empty() {
            history.summary.clear();
        } else {
            return Err(CommandError::new("tutor_context_budget",
                "The current question and required evidence exceed this model's context budget. Select a smaller section or shorten the message."));
        }
    };
    Ok(StudyContext {
        question_hint: None,
        sources,
        units,
        history,
        inherited_turns,
        request_text,
        notice,
        images,
        completion_tokens,
    })
}

fn merge_optional_retrieval(
    sources: &mut Vec<SourceRecord>,
    notice: &mut String,
    result: Result<retrieval::RetrievalResult, CommandError>,
) -> Result<(), CommandError> {
    match result {
        Ok(result) => sources.extend(result.hits.into_iter().take(8).map(|hit| hit.source)),
        Err(error) if optional_retrieval_failure(&error.code) => {
            crate::logging::debug_target(
                "ai.tutor.context",
                "tutor_optional_grounding_unavailable",
                "Optional semantic lookup was unavailable; retained exact evidence and model knowledge remain usable.",
                json!({"code":error.code,"retainedSources":sources.len()}),
            );
            *notice = "Semantic lookup was unavailable. Only attached source evidence was consulted. Do not claim access to other selected material; distinguish general model knowledge and disclose limitations for source-specific requests.".to_string();
        }
        Err(error) => return Err(error),
    }
    Ok(())
}

/// Only lookup availability failures are optional. Never swallow cancellation,
/// a permission/revision failure, corrupt storage, or an unknown native error.
fn optional_retrieval_failure(code: &str) -> bool {
    matches!(
        code,
        "semantic_preparation_required"
            | "ai_task_profile_required"
            | "ai_http_unauthorized"
            | "ai_authentication_failed"
            | "ai_http_rate_limited"
            | "ai_rate_limited"
            | "ai_insufficient_credits"
            | "ai_connection_failed"
            | "ai_network_error"
            | "ai_initial_response_timeout"
            | "ai_request_timeout"
            | "ai_provider_timeout"
            | "ai_provider_overloaded"
            | "ai_provider_unavailable"
            | "ai_server_unavailable"
            | "ai_stream_inactive"
    )
}

fn filter_scope(
    database: &Database,
    scope: &StudyScope,
    sources: Vec<SourceRecord>,
    include_conversations: bool,
) -> Result<Vec<SourceRecord>, CommandError> {
    database.with_connection(|connection| {
        let permitted = retrieval::eligible_entries_in(connection, scope, include_conversations)?
            .into_iter()
            .filter_map(|entry| entry.id.strip_prefix("source:").map(str::to_string))
            .collect::<HashSet<_>>();
        let mut result = Vec::new();
        for source in sources {
            if permitted.contains(&source.id) && repository::source_is_current(connection, &source)?
            {
                result.push(source);
            }
        }
        Ok(result)
    })
}

#[cfg(test)]
mod optional_retrieval_tests {
    use super::*;

    #[test]
    fn empty_search_and_missing_embedding_profile_do_not_block_chat() {
        let mut sources = Vec::new();
        let mut notice = String::new();
        merge_optional_retrieval(
            &mut sources,
            &mut notice,
            Ok(retrieval::RetrievalResult {
                hits: Vec::new(),
                eligible_entries: 0,
                space_id: String::new(),
            }),
        )
        .unwrap();
        for code in ["semantic_preparation_required", "ai_task_profile_required"] {
            merge_optional_retrieval(
                &mut sources,
                &mut notice,
                Err(CommandError::new(code, "not configured")),
            )
            .unwrap();
        }
        assert!(sources.is_empty());
        assert!(notice.contains("Do not claim access"));
    }

    #[test]
    fn cancellation_revision_and_storage_errors_are_not_fallbacks() {
        for code in [
            "ai_request_canceled",
            "semantic_source_stale",
            "ai_profile_changed",
            "database_error",
            "invalid_semantic_content",
            "unknown_failure",
        ] {
            let error = merge_optional_retrieval(
                &mut Vec::new(),
                &mut String::new(),
                Err(CommandError::new(code, "test failure")),
            )
            .unwrap_err();
            assert_eq!(error.code, code);
        }
    }
}
