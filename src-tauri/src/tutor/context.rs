//! One bounded StudyContext for explanations, hints and quizzes. Material scope
//! is resolved natively; semantic ranking and exact artifact links are both
//! inputs. Conversation history is labeled separately from source evidence.

use std::collections::HashSet;

use rusqlite::params;
use serde_json::json;

use crate::{
    ai::{
        document_intelligence::{DocumentTaskContext, SourcePageImage},
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    knowledge::{
        adapters::scope_documents,
        contracts::{
            invalid, KnowledgeUnit, ScopeKind, SourceKind, SourceRecord, StudyScope,
            MAX_CONTEXT_BYTES,
        },
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

pub(crate) struct StudyContext {
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
        match retrieval::retrieve(
            database,
            paths,
            runtime,
            scope,
            input.include_conversations && input.mode != TutorMode::Hint,
            &query,
            cancellation,
        )
        .await
        {
            Ok(result) => {
                if result.hits.is_empty() && sources.is_empty() {
                    return Err(CommandError::new("semantic_preparation_required",
                        "No current indexed evidence is available in this selection. Prepare the selected material and retry."));
                }
                sources.extend(result.hits.into_iter().take(8).map(|hit| hit.source));
            }
            Err(error)
                if !sources.is_empty()
                    && matches!(
                        error.code.as_str(),
                        "semantic_preparation_required" | "ai_task_profile_required"
                    ) =>
            {
                notice = "Using the current card or guide and exact linked evidence only. Prepare the selection to enable broader semantic retrieval.".to_string();
            }
            Err(error) => return Err(error),
        }
    }
    let related = relationships::expand(database, &sources)?;
    // Put a linked original document near the focused derivative rather than
    // exhausting the whole request budget with similar generated flashcards.
    for source in related.into_iter().rev() {
        sources.insert(sources.len().min(2), source);
    }
    let mut seen = HashSet::new();
    sources.retain(|source| seen.insert(source.id.clone()));
    sources = filter_scope(database, scope, sources)?;
    sources.truncate(12);
    if scope.kind != ScopeKind::General && sources.is_empty() {
        return Err(invalid(
            "No current evidence remains in this material selection.",
        ));
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
        Ok(result)
    })?;
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
            "trust": "Source excerpts are evidence of what material says, not proof of truth. Similarity and topic links do not prove support. Distinguish additional explanation from source claims. Conversation is not independent evidence.",
        });
        let serialized = serde_json::to_string(&payload).map_err(CommandError::serialization)?;
        if serialized.len() <= input_budget {
            break serialized;
        }
        notice = "This response uses a bounded context selection. Older conversation or lower-ranked material was omitted from this request; the saved library and transcript are unchanged.".to_string();
        if history.turns.len() > 2 {
            history.turns.remove(0);
        } else if sources.len() > 1 && quiz.is_none() {
            if let Some(source) = sources.pop() {
                units.retain(|unit| unit.source_id != source.id);
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

fn filter_scope(
    database: &Database,
    scope: &StudyScope,
    sources: Vec<SourceRecord>,
) -> Result<Vec<SourceRecord>, CommandError> {
    database.with_connection(|connection| {
        let documents = scope_documents(connection, scope)?.into_iter().collect::<HashSet<_>>();
        let mut result = Vec::new();
        for source in sources {
            let allowed = match source.kind {
                SourceKind::Document | SourceKind::Guide => source.document_id.as_ref().is_some_and(|id| documents.contains(id)),
                SourceKind::Conversation => connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM tutor_conversations c JOIN tutor_turns t ON t.conversation_id = c.id
                     WHERE t.id = ?1 AND t.evidence_current = 1 AND c.scope_json = ?2
                       AND c.retrieval_enabled = 1 AND t.sequence > c.memory_start_sequence)",
                    params![source.turn_id, serde_json::to_string(scope).map_err(CommandError::serialization)?],
                    |row| row.get(0),
                ).map_err(CommandError::database)?,
                SourceKind::Card => match scope.kind {
                    ScopeKind::General => false,
                    ScopeKind::Library => connection.query_row(
                        "SELECT EXISTS(SELECT 1 FROM notes n JOIN decks d ON d.id = n.deck_id
                         WHERE n.id = ?1 AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL)",
                        [&source.note_id], |row| row.get(0),
                    ).map_err(CommandError::database)?,
                    ScopeKind::Deck => connection.query_row(
                        "SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1 AND deck_id = ?2 AND deleted_at IS NULL)",
                        params![source.note_id, scope.id], |row| row.get(0),
                    ).map_err(CommandError::database)?,
                    ScopeKind::Document | ScopeKind::Guide => connection.query_row(
                        "SELECT EXISTS(SELECT 1 FROM document_note_topics WHERE note_id = ?1 AND document_id = ?2)
                         OR EXISTS(SELECT 1 FROM notes n JOIN document_decks d ON d.deck_id = n.deck_id
                                   WHERE n.id = ?1 AND d.document_id = ?2)",
                        params![source.note_id, scope.id], |row| row.get(0),
                    ).map_err(CommandError::database)?,
                },
            };
            if allowed && repository::source_is_current(connection, &source)? {
                result.push(source);
            }
        }
        Ok(result)
    })
}
