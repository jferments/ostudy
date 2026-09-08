//! Scoped semantic retrieval over the shared material representation. The SQL
//! scope filter runs before vector ranking; no keyword match is required for a
//! passage to become a candidate. Source revisions are checked again afterward.

use std::collections::HashSet;

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::{
    ai::{embeddings::{self, Purpose}, AiRuntimeState},
    database::Database,
    error::CommandError,
    paths::AppPaths,
    task_cancellation::CancellationSignal,
};

use super::{
    adapters::scope_documents,
    contracts::{bounded, invalid, KnowledgeUnit, ScopeKind, SourceCitation, SourceRecord, StudyScope},
    index::IndexKey,
    preparation::open_index,
    repository,
    runtime::check_canceled,
};

const MAX_ELIGIBLE_ENTRIES: usize = 250_000;

pub(crate) struct RetrievedSource {
    pub source: SourceRecord,
    pub units: Vec<KnowledgeUnit>,
    pub distance: f64,
}

pub(crate) struct RetrievalResult {
    pub hits: Vec<RetrievedSource>,
    pub eligible_entries: usize,
    pub space_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalPreview {
    pub sources: Vec<SourceCitation>,
    pub eligible_entries: usize,
    pub embedding_space: String,
}

pub(crate) async fn retrieve(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    scope: &StudyScope,
    include_conversations: bool,
    query: &str,
    cancellation: &CancellationSignal,
) -> Result<RetrievalResult, CommandError> {
    scope.validate()?;
    bounded(query, 8_000, "semantic search query", false)?;
    check_canceled(cancellation)?;
    let context = embeddings::load_default_embedding_context(database)?;
    let mut index = open_index(paths)?;
    if index.dimensions(&context.space_id)?.is_none() {
        return Err(CommandError::new(
            "semantic_preparation_required",
            "Prepare this material with the selected embedding model before using semantic retrieval.",
        ));
    }
    let vectors = embeddings::embed(
        database,
        runtime,
        &context,
        Purpose::Query,
        &[query.to_string()],
        cancellation,
    )
    .await?;
    let eligible = eligible_entries(database, scope, include_conversations)?;
    let eligible_count = eligible.len();
    let space = context.space_id.clone();
    let signal = cancellation.clone();
    let query_vector = vectors.into_iter().next().ok_or_else(|| invalid("No query embedding was returned."))?;
    let hits = tauri::async_runtime::spawn_blocking(move || {
        index.search(&space, &query_vector, &eligible, 48, &signal)
    })
    .await
    .map_err(|_| CommandError::new("semantic_search_interrupted", "Semantic retrieval was interrupted."))??;
    check_canceled(cancellation)?;
    embeddings::ensure_context_current(database, &context)?;
    let hits = database.with_connection(|connection| {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        for hit in hits {
            if !hit.distance.is_finite() || seen.contains(&hit.key.source_id) {
                continue;
            }
            let Some(source) = repository::load_source(connection, &hit.key.source_id)? else {
                continue;
            };
            let current = if let Some(unit_id) = hit.key.id.strip_prefix("unit:") {
                connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM knowledge_units
                     WHERE id = ?1 AND source_id = ?2 AND source_hash = ?3 AND content_hash = ?4)",
                    params![unit_id, source.id, source.content_hash, hit.key.content_hash],
                    |row| row.get::<_, bool>(0),
                ).map_err(CommandError::database)?
            } else {
                source.content_hash == hit.key.content_hash
            };
            if !current {
                continue;
            }
            let mut units = repository::load_units(connection, &source.id)?;
            units.extend(repository::linked_units(connection, &source)?);
            units.sort_by(|left, right| left.id.cmp(&right.id));
            units.dedup_by(|left, right| left.id == right.id);
            units.truncate(32);
            seen.insert(source.id.clone());
            result.push(RetrievedSource { source, units, distance: hit.distance });
        }
        Ok(result)
    })?;
    Ok(RetrievalResult { hits, eligible_entries: eligible_count, space_id: context.space_id })
}

pub(crate) fn eligible_entries(
    database: &Database,
    scope: &StudyScope,
    include_conversations: bool,
) -> Result<Vec<IndexKey>, CommandError> {
    database.with_connection(|connection| {
        let documents = scope_documents(connection, scope)?;
        let documents_json = serde_json::to_string(&documents).map_err(CommandError::serialization)?;
        let scope_json = serde_json::to_string(scope).map_err(CommandError::serialization)?;
        let kind = match scope.kind {
            ScopeKind::General => "general",
            ScopeKind::Deck => "deck",
            ScopeKind::Document => "document",
            ScopeKind::Guide => "guide",
            ScopeKind::Library => "library",
        };
        let mut statement = connection.prepare(
            "WITH allowed_sources AS (
                 SELECT s.id, s.content_hash FROM knowledge_sources s
                 WHERE s.active = 1 AND (
                     (s.kind = 'document' AND s.document_id IN (SELECT value FROM json_each(?1)))
                     OR (s.kind = 'guide' AND s.document_id IN (SELECT value FROM json_each(?1))
                         AND EXISTS(SELECT 1 FROM document_study_guides g WHERE g.id = s.guide_id AND g.state = 'active'))
                     OR (s.kind = 'card' AND EXISTS(
                         SELECT 1 FROM cards c JOIN notes n ON n.id = c.note_id JOIN decks d ON d.id = n.deck_id
                         WHERE c.id = s.card_id AND c.deleted_at IS NULL AND n.deleted_at IS NULL AND d.deleted_at IS NULL
                           AND ((?2 = 'library' AND d.archived_at IS NULL)
                             OR (?2 = 'deck' AND n.deck_id = ?3)
                             OR (?2 IN ('document', 'guide') AND (
                                 EXISTS(SELECT 1 FROM document_note_topics nt WHERE nt.note_id = n.id AND nt.document_id = ?3)
                                 OR EXISTS(SELECT 1 FROM document_decks dd WHERE dd.deck_id = n.deck_id AND dd.document_id = ?3)))
                           )))
                     OR (?4 = 1 AND s.kind = 'conversation' AND EXISTS(
                         SELECT 1 FROM tutor_turns t JOIN tutor_conversations tc ON tc.id = t.conversation_id
                         WHERE t.id = s.turn_id AND t.state = 'complete' AND t.evidence_current = 1
                           AND tc.scope_json = ?5))
                 )
             )
             SELECT 'source:' || s.id, s.id, s.content_hash FROM allowed_sources s
             UNION ALL
             SELECT 'unit:' || u.id, u.source_id, u.content_hash FROM knowledge_units u
             JOIN allowed_sources s ON s.id = u.source_id AND s.content_hash = u.source_hash
             LIMIT 250001",
        ).map_err(CommandError::database)?;
        let entries = statement.query_map(
            params![documents_json, kind, scope.id, include_conversations, scope_json],
            |row| Ok(IndexKey { id: row.get(0)?, source_id: row.get(1)?, content_hash: row.get(2)? }),
        ).map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        if entries.len() > MAX_ELIGIBLE_ENTRIES {
            return Err(invalid("This semantic search scope is too large. Select a deck or document rather than the full library."));
        }
        Ok(entries)
    })
}

pub(crate) fn resolve_citation(
    database: &Database,
    source_id: &str,
    content_hash: &str,
) -> Result<SourceCitation, CommandError> {
    crate::util::validate_entity_id(source_id, "source citation")?;
    if !super::contracts::is_hash(content_hash) {
        return Err(invalid("The source citation fingerprint is invalid."));
    }
    database.with_connection(|connection| {
        let source = repository::load_source(connection, source_id)?
            .filter(|source| source.content_hash == content_hash)
            .ok_or_else(|| CommandError::new(
                "tutor_source_unavailable",
                "This cited source was changed or removed. The historical answer is not current source evidence.",
            ))?;
        if let Some(turn_id) = &source.turn_id {
            let current = connection.query_row(
                "SELECT 1 FROM tutor_turns WHERE id = ?1 AND state = 'complete' AND evidence_current = 1",
                [turn_id], |_| Ok(()),
            ).optional().map_err(CommandError::database)?;
            if current.is_none() {
                return Err(repository::stale_source());
            }
        }
        Ok(SourceCitation::from(&source))
    })
}
