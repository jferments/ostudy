//! Derived indexing of SAVED evidence and accepted knowledge. This module never
//! calls an interpretation model. Navigation/startup are local-only; document
//! processing, sending chat and explicit maintenance authorize scoped catch-up.
use super::{
    adapters::{enumerate_sources, materialize_sources},
    contracts::{hash, invalid, KnowledgeUnit, ScopeKind, StudyScope},
    index::{IndexKey, IndexWrite},
    preparation::{open_index, source_entry, unit_entry},
    repository, retrieval,
    runtime::check_canceled,
};
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
use serde_json::json;
use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;

// Only missing-vector catch-up is serialized, not chat/interpretation. No
// library transaction crosses an await; overlapping actions reuse saved vectors.
static INDEX_WORK: Mutex<()> = Mutex::const_new(());
const BATCH_ENTRIES: usize = 32;
const BATCH_CHARS: usize = 64_000;

pub(crate) struct IndexSelection<'a> {
    pub scope: &'a StudyScope,
    pub include_conversations: bool,
    pub signal: &'a CancellationSignal,
    pub expected_revision: Option<&'a str>,
    pub expected_embedding: Option<&'a str>,
    pub diagnostics: Option<generation::Context>,
}

pub(crate) fn scope_revision(entries: &[IndexKey]) -> Result<String, CommandError> {
    let mut keys = entries
        .iter()
        .map(|k| (&k.id, &k.source_id, &k.content_hash))
        .collect::<Vec<_>>();
    keys.sort();
    Ok(hash(
        &serde_json::to_string(&keys).map_err(CommandError::serialization)?,
    ))
}

pub(crate) async fn ensure_scope(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    selection: IndexSelection<'_>,
) -> Result<(), CommandError> {
    let IndexSelection {
        scope,
        include_conversations,
        signal,
        expected_revision,
        expected_embedding,
        diagnostics,
    } = selection;
    scope.validate()?;
    if scope.kind == ScopeKind::General {
        return Ok(());
    }
    let context = embeddings::load_default_embedding_context(database)?;
    if expected_embedding.is_some_and(|expected| expected != context.consent_identity()) {
        return Err(CommandError::new(
            "semantic_preparation_changed",
            "The embedding provider changed. Review the updated index preview.",
        ));
    }
    let _lease = tokio::select! {
        _ = signal.canceled() => { check_canceled(signal)?; return Ok(()); },
        lease = INDEX_WORK.lock() => lease,
    };
    check_canceled(signal)?;
    let seeds = enumerate_sources(database, scope, include_conversations)?;
    for batch in seeds.chunks(32) {
        check_canceled(signal)?;
        materialize_sources(database, batch)?;
        tokio::task::yield_now().await;
    }
    let entries = retrieval::eligible_entries(database, scope, include_conversations)?;
    if let Some(expected) = expected_revision {
        if scope_revision(&entries)? != expected {
            return Err(CommandError::new(
                "semantic_preparation_changed",
                "The selected source revisions changed. Review the updated index preview.",
            ));
        }
    }
    let mut index = open_index(database, paths)?;
    let total = entries.len();
    let mut missing = Vec::new();
    for entry in &entries {
        if !index.contains(&context.space_id, entry)? {
            missing.push(entry.clone());
        }
    }
    crate::logging::debug_target(
        "ai.knowledge.index",
        "knowledge_index_started",
        "Indexing saved evidence and knowledge; no interpretation requests occur in this path.",
        json!({"entries":total,"missingEntries":missing.len(),"cachedEntries":total-missing.len()}),
    );
    let mut cursor = 0;
    while cursor < missing.len() {
        check_canceled(signal)?;
        ensure_default_current(database, &context)?;
        let allowed = permitted(database, scope, include_conversations)?;
        let mut keys = Vec::new();
        let mut texts = Vec::new();
        let mut chars = 0;
        while cursor + keys.len() < missing.len() && keys.len() < BATCH_ENTRIES {
            let key = &missing[cursor + keys.len()];
            require_allowed(key, &allowed)?;
            let text = entry_text(database, key)?;
            let length = text.chars().count();
            if !keys.is_empty() && chars + length > BATCH_CHARS {
                break;
            }
            chars += length;
            keys.push(key.clone());
            texts.push(text);
        }
        crate::task_cancellation::set_current_task_phase(
            "indexing saved material",
            None,
            cursor,
            missing.len(),
            None,
            None,
            None,
        );
        let vectors = embeddings::embed(
            database,
            runtime,
            &context,
            Purpose::Document,
            &texts,
            signal,
            diagnostics.as_ref().map(|context| EmbeddingDiagnostics {
                context,
                stage: "embedding_indexing",
            }),
        )
        .await?;
        check_canceled(signal)?;
        ensure_default_current(database, &context)?;
        let allowed = permitted(database, scope, include_conversations)?;
        let writes = validated_writes(database, &keys, vectors, &allowed)?;
        index.write(&context.space_id, &writes)?;
        cursor += keys.len();
        crate::logging::debug_target(
            "ai.knowledge.index",
            "knowledge_index_batch_completed",
            "Checkpointed search vectors without changing source interpretations.",
            json!({"completed":cursor,"total":missing.len()}),
        );
        tokio::task::yield_now().await;
    }
    // Permissions/revisions can change even in an all-cached run.
    check_canceled(signal)?;
    ensure_default_current(database, &context)?;
    let allowed = permitted(database, scope, include_conversations)?;
    for entry in &entries {
        require_allowed(entry, &allowed)?;
    }
    crate::logging::info_target(
        "ai.knowledge.index",
        "knowledge_index_ready",
        "Selected material is searchable using saved knowledge.",
        json!({"newEntries":missing.len(),"cachedEntries":total-missing.len()}),
    );
    Ok(())
}

pub(crate) async fn index_document(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    document_id: &str,
    signal: &CancellationSignal,
    diagnostics: Option<generation::Context>,
) -> Result<(), CommandError> {
    let scope = StudyScope {
        members: Vec::new(),
        kind: ScopeKind::Document,
        id: Some(document_id.to_string()),
    };
    ensure_scope(
        database,
        paths,
        runtime,
        IndexSelection {
            scope: &scope,
            include_conversations: false,
            signal,
            expected_revision: None,
            expected_embedding: None,
            diagnostics,
        },
    )
    .await
}
fn ensure_default_current(
    database: &Database,
    context: &embeddings::EmbeddingContext,
) -> Result<(), CommandError> {
    embeddings::ensure_context_current(database, context)?;
    let current = embeddings::load_default_embedding_context(database)?;
    if current.consent_identity() != context.consent_identity() {
        return Err(CommandError::new("embedding_profile_changed",
            "The selected embedding provider or model changed during indexing. Retry with the new selection."));
    }
    Ok(())
}
fn permitted(
    database: &Database,
    scope: &StudyScope,
    include: bool,
) -> Result<HashMap<String, String>, CommandError> {
    Ok(retrieval::eligible_entries(database, scope, include)?
        .into_iter()
        .map(|key| (key.id, key.content_hash))
        .collect())
}
fn require_allowed(key: &IndexKey, allowed: &HashMap<String, String>) -> Result<(), CommandError> {
    if allowed.get(&key.id) != Some(&key.content_hash) {
        return Err(repository::stale_source());
    }
    Ok(())
}
fn validated_writes(
    database: &Database,
    keys: &[IndexKey],
    vectors: Vec<Vec<f32>>,
    allowed: &HashMap<String, String>,
) -> Result<Vec<IndexWrite>, CommandError> {
    if vectors.len() != keys.len() {
        return Err(invalid("The embedding response omitted index entries."));
    }
    let mut seen = HashSet::new();
    for key in keys {
        if !seen.insert(&key.id) {
            return Err(invalid("Duplicate index entry."));
        }
        require_allowed(key, allowed)?;
        entry_text(database, key)?;
    }
    Ok(keys
        .iter()
        .cloned()
        .zip(vectors)
        .map(|(key, vector)| IndexWrite { key, vector })
        .collect())
}
fn entry_text(database: &Database, key: &IndexKey) -> Result<String, CommandError> {
    database.with_connection(|connection| {
        let source = repository::load_source(connection, &key.source_id)?
            .ok_or_else(repository::stale_source)?;
        let (current, text) = if let Some(id) = key.id.strip_prefix("unit:") {
            let serialized: String = connection
                .query_row(
                    "SELECT record_json FROM knowledge_units WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let unit: KnowledgeUnit =
                serde_json::from_str(&serialized).map_err(CommandError::serialization)?;
            if unit.source_id != source.id || unit.source_hash != source.content_hash {
                return Err(repository::stale_source());
            }
            unit.content.validate(&source)?;
            repository::validate_grounding(connection, &unit.content)?;
            unit_entry(&unit)?
        } else {
            source_entry(&source)
        };
        if current.id != key.id || current.content_hash != key.content_hash {
            return Err(repository::stale_source());
        }
        Ok(text)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexing_saved_units_is_read_only_and_revision_bound() {
        let (db, source, catalog, _) = crate::knowledge::authoring::tests::fixture();
        let before = db
            .with_connection(|c| {
                super::super::snapshot::DocumentSnapshot::capture(c, &source.owner_id)
            })
            .unwrap();
        let key = unit_entry(&catalog.units[0]).unwrap().0;
        let allowed = HashMap::from([(key.id.clone(), key.content_hash.clone())]);
        assert!(!entry_text(&db, &key).unwrap().is_empty());
        validated_writes(
            &db,
            std::slice::from_ref(&key),
            vec![vec![1., 0.]],
            &allowed,
        )
        .unwrap();
        db.with_connection(|c| before.ensure_current(c)).unwrap();
        assert!(validated_writes(
            &db,
            std::slice::from_ref(&key),
            vec![vec![1., 0.]],
            &HashMap::new()
        )
        .is_err());
        let mut changed = source.clone();
        changed.text.push_str(" edited");
        changed.refresh_identity().unwrap();
        db.with_connection(|c| repository::publish_source(c, &changed, "0", 4))
            .unwrap();
        assert!(validated_writes(&db, &[key], vec![vec![1., 0.]], &allowed).is_err());
    }
    #[test]
    fn index_revision_is_order_independent_and_includes_interpretations() {
        let (_, source, catalog, _) = crate::knowledge::authoring::tests::fixture();
        let keys = vec![
            source_entry(&source).0,
            unit_entry(&catalog.units[0]).unwrap().0,
        ];
        assert_eq!(
            scope_revision(&keys).unwrap(),
            scope_revision(&[keys[1].clone(), keys[0].clone()]).unwrap()
        );
        assert_ne!(
            scope_revision(&keys).unwrap(),
            scope_revision(&keys[..1]).unwrap()
        );
    }
}
