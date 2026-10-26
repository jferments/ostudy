//! Scoped semantic retrieval over the shared material representation. The SQL
//! scope filter runs before vector ranking; no keyword match is required for a
//! passage to become a candidate. Source revisions are checked again afterward.

use std::collections::{BTreeMap, HashSet};

use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::{
    ai::{
        embeddings::{self, EmbeddingDiagnostics, Purpose},
        generation, AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    paths::AppPaths,
    task_cancellation::CancellationSignal,
};

use super::{
    adapters::scope_documents,
    collection_scope,
    contracts::{bounded, invalid, ScopeKind, SourceCitation, SourceRecord, StudyScope},
    index::{IndexKey, VectorHit},
    preparation::open_index,
    repository,
    runtime::check_canceled,
};

const MAX_ELIGIBLE_ENTRIES: usize = 250_000;

pub(crate) struct RetrievedSource {
    pub source: SourceRecord,
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

pub(crate) struct RetrievalRequest<'a> {
    pub scope: &'a StudyScope,
    pub include_conversations: bool,
    pub query: &'a str,
    pub cancellation: &'a CancellationSignal,
    pub diagnostics: Option<&'a generation::Context>,
}

pub(crate) async fn retrieve(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    request: RetrievalRequest<'_>,
) -> Result<RetrievalResult, CommandError> {
    let RetrievalRequest {
        scope,
        include_conversations,
        query,
        cancellation,
        diagnostics,
    } = request;
    scope.validate()?;
    bounded(query, 8_000, "semantic search query", false)?;
    check_canceled(cancellation)?;
    super::indexing::ensure_scope(
        database,
        paths,
        runtime,
        super::indexing::IndexSelection {
            scope,
            include_conversations,
            signal: cancellation,
            expected_revision: None,
            expected_embedding: None,
            diagnostics: diagnostics.cloned(),
        },
    )
    .await?;
    let context = embeddings::load_default_embedding_context(database)?;
    let mut index = open_index(database, paths)?;
    if index.dimensions(&context.space_id)?.is_none() {
        return Err(CommandError::new(
            "semantic_preparation_required",
            "No searchable text is available in this selection. Check the saved material and embedding model in AI settings.",
        ));
    }
    let vectors = embeddings::embed(
        database,
        runtime,
        &context,
        Purpose::Query,
        &[query.to_string()],
        cancellation,
        diagnostics.map(|context| EmbeddingDiagnostics {
            context,
            stage: "embedding_query",
        }),
    )
    .await?;
    let eligible = eligible_entries(database, scope, include_conversations)?;
    let eligible_count = eligible.len();
    let space = context.space_id.clone();
    let signal = cancellation.clone();
    let query_vector = vectors
        .into_iter()
        .next()
        .ok_or_else(|| invalid("No query embedding was returned."))?;
    let hits = tauri::async_runtime::spawn_blocking(move || {
        index.search(&space, &query_vector, &eligible, 48, &signal)
    })
    .await
    .map_err(|_| {
        CommandError::new(
            "semantic_search_interrupted",
            "Semantic retrieval was interrupted.",
        )
    })??;
    check_canceled(cancellation)?;
    embeddings::ensure_context_current(database, &context)?;
    let hits = database.with_connection(|connection| {
        current_hits(connection, scope, include_conversations, hits)
    })?;
    Ok(RetrievalResult {
        hits,
        eligible_entries: eligible_count,
        space_id: context.space_id,
    })
}

/// Final authorization after ranking; an index result alone never grants access.
pub(crate) fn current_hits(
    connection: &rusqlite::Connection,
    scope: &StudyScope,
    include_conversations: bool,
    hits: Vec<VectorHit>,
) -> Result<Vec<RetrievedSource>, CommandError> {
    // Permission can be revoked while vector ranking runs off-thread.
    // Recheck the whole allowed key set, including multi-source dependencies,
    // before exposing hits through either Tutor or the retrieval-preview IPC.
    let allowed = eligible_entries_in(connection, scope, include_conversations)?
        .into_iter()
        .map(|key| (key.id, key.content_hash))
        .collect::<std::collections::HashMap<_, _>>();
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for hit in hits {
        if !hit.distance.is_finite()
            || seen.contains(&hit.key.source_id)
            || allowed.get(&hit.key.id) != Some(&hit.key.content_hash)
        {
            continue;
        }
        let Some(source) = repository::load_source(connection, &hit.key.source_id)? else {
            continue;
        };
        let current = if let Some(unit_id) = hit.key.id.strip_prefix("unit:") {
            connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM knowledge_units
                     WHERE id = ?1 AND source_id = ?2 AND source_hash = ?3 AND content_hash = ?4)",
                    params![
                        unit_id,
                        source.id,
                        source.content_hash,
                        hit.key.content_hash
                    ],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(CommandError::database)?
        } else {
            source.content_hash == hit.key.content_hash
        };
        if !current {
            continue;
        }
        seen.insert(source.id.clone());
        result.push(RetrievedSource { source });
    }
    Ok(result)
}

pub(crate) fn eligible_entries(
    database: &Database,
    scope: &StudyScope,
    include_conversations: bool,
) -> Result<Vec<IndexKey>, CommandError> {
    scope.validate()?;
    if scope.kind == ScopeKind::General {
        return Ok(Vec::new());
    }
    database
        .with_connection(|connection| eligible_entries_in(connection, scope, include_conversations))
}

pub(crate) fn eligible_entries_in(
    connection: &rusqlite::Connection,
    scope: &StudyScope,
    include_conversations: bool,
) -> Result<Vec<IndexKey>, CommandError> {
    scope.validate()?;
    if scope.kind == ScopeKind::General {
        return Ok(Vec::new());
    }
    if collection_scope::has_collection(scope) {
        let mut entries = BTreeMap::new();
        for entry in collection_scope::eligible_entries(connection, scope)? {
            entries.insert(entry.id.clone(), entry);
        }
        for member in scope
            .material_scopes()
            .into_iter()
            .filter(|member| member.kind != ScopeKind::Collection)
        {
            for entry in eligible_entries_in(connection, &member, false)? {
                entries.insert(entry.id.clone(), entry);
            }
        }
        if entries.len() > MAX_ELIGIBLE_ENTRIES {
            return Err(invalid(
                "This semantic search scope is too large. Select fewer sources.",
            ));
        }
        // Historical conversation text is never reused implicitly for a dynamic
        // collection scope. Current collection evidence remains available.
        let _ = include_conversations;
        return Ok(entries.into_values().collect());
    }
    let documents = scope_documents(connection, scope)?;
    let members = scope.material_scopes();
    let decks = members
        .iter()
        .filter(|s| s.kind == ScopeKind::Deck)
        .filter_map(|s| s.id.as_ref())
        .collect::<Vec<_>>();
    let direct_documents = members
        .iter()
        .filter(|s| matches!(s.kind, ScopeKind::Document | ScopeKind::Guide))
        .filter_map(|s| s.id.as_ref())
        .collect::<Vec<_>>();
    let documents_json = serde_json::to_string(&documents).map_err(CommandError::serialization)?;
    let decks_json = serde_json::to_string(&decks).map_err(CommandError::serialization)?;
    let direct_json =
        serde_json::to_string(&direct_documents).map_err(CommandError::serialization)?;
    let scope_json = serde_json::to_string(scope).map_err(CommandError::serialization)?;
    let mut statement = connection.prepare(
        "WITH allowed_sources AS (
             SELECT s.id, s.content_hash, s.extraction_contract FROM knowledge_sources s
             WHERE s.active = 1 AND (
                 (s.kind = 'document' AND s.document_id IN (SELECT value FROM json_each(?1)))
                 OR (s.kind = 'guide' AND s.document_id IN (SELECT value FROM json_each(?1))
                     AND EXISTS(SELECT 1 FROM document_study_guides g WHERE g.id = s.guide_id AND g.state = 'active'))
                 OR (s.kind = 'card' AND EXISTS(
                     SELECT 1 FROM cards c JOIN notes n ON n.id = c.note_id JOIN decks d ON d.id = n.deck_id
                     WHERE c.id = s.card_id AND c.deleted_at IS NULL AND n.deleted_at IS NULL AND d.deleted_at IS NULL
                       AND ((?2 = 1 AND d.archived_at IS NULL)
                         OR n.deck_id IN (SELECT value FROM json_each(?3))
                         OR EXISTS(SELECT 1 FROM document_note_topics nt WHERE nt.note_id = n.id
                             AND nt.document_id IN (SELECT value FROM json_each(?4)))
                         OR EXISTS(SELECT 1 FROM document_decks dd WHERE dd.deck_id = n.deck_id
                             AND dd.document_id IN (SELECT value FROM json_each(?4))))))
                 OR (?5 = 1 AND s.kind = 'conversation' AND EXISTS(
                     SELECT 1 FROM tutor_turns t JOIN tutor_conversations tc ON tc.id = t.conversation_id
                     WHERE t.id = s.turn_id AND t.state = 'complete' AND t.evidence_current = 1
                       AND t.id NOT IN (SELECT turn_id FROM tutor_turn_replacements)
                       AND COALESCE(t.captured_scope_json, json_extract(t.request_json, '$.capturedScope'), tc.scope_json) = ?6
                       AND tc.retrieval_enabled = 1 AND t.sequence > tc.memory_start_sequence))
             )
         )
         SELECT 'source:' || s.id, s.id, s.content_hash FROM allowed_sources s
         UNION ALL
         SELECT 'unit:' || u.id, u.source_id, u.content_hash FROM knowledge_units u
         JOIN allowed_sources s ON s.id = u.source_id AND s.content_hash = u.source_hash
             AND s.extraction_contract = u.extraction_contract
         WHERE NOT EXISTS (
             SELECT 1 FROM knowledge_unit_evidence e
             LEFT JOIN allowed_sources dependency ON dependency.id = e.source_id AND dependency.content_hash = e.source_hash
             WHERE e.unit_id = u.id AND dependency.id IS NULL)
         LIMIT 250001",
    ).map_err(CommandError::database)?;
    let entries = statement
        .query_map(
            params![
                documents_json,
                scope.kind == ScopeKind::Library,
                decks_json,
                direct_json,
                include_conversations,
                scope_json
            ],
            |row| {
                Ok(IndexKey {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    content_hash: row.get(2)?,
                })
            },
        )
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    if entries.len() > MAX_ELIGIBLE_ENTRIES {
        return Err(invalid(
            "This semantic search scope is too large. Select fewer sources.",
        ));
    }
    Ok(entries)
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
                "SELECT 1 FROM tutor_turns WHERE id = ?1 AND state = 'complete' AND evidence_current = 1
                 AND id NOT IN (SELECT turn_id FROM tutor_turn_replacements)",
                [turn_id], |_| Ok(()),
            ).optional().map_err(CommandError::database)?;
            if current.is_none() {
                return Err(repository::stale_source());
            }
        }
        Ok(SourceCitation::from(&source))
    })
}
