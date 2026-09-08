//! Explicit, resumable semantic preparation. A preview binds the selected scope,
//! source revisions and providers; only a matching user-confirmed request sends
//! material to a provider. Native saves and ordinary study never enter this path.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ai::{
        document_intelligence::{load_document_task_context, DocumentTaskContext},
        document_provider::load_document_provider_capabilities,
        embeddings::{self, EmbeddingContext, Purpose},
        AiRuntimeState,
    },
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::validate_entity_id,
};

use super::{
    adapters::{enumerate_sources, materialize_sources},
    contracts::{
        hash, invalid, is_hash, KnowledgeUnit, SourceKind, SourceRecord, StudyScope,
        MAX_PREPARATION_SOURCES,
    },
    extraction::{batch_ranges, ensure_units_batch, extraction_contract},
    index::{IndexKey, IndexWrite, VectorIndex},
    preparation_store::{self, PreparationStart, PreparationStatus},
    repository,
    runtime::{self, TaskGuard},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparationScope {
    pub scope: StudyScope,
    #[serde(default)]
    pub include_conversations: bool,
}

impl PreparationScope {
    pub(crate) fn key(&self) -> Result<String, CommandError> {
        self.scope.validate()?;
        Ok(hash(
            &serde_json::to_string(self).map_err(CommandError::serialization)?,
        ))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareMaterialInput {
    pub operation_id: String,
    pub selection: PreparationScope,
    pub consent_key: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparationPreview {
    pub selection: PreparationScope,
    pub consent_key: String,
    pub source_count: usize,
    pub source_characters: usize,
    pub image_count: usize,
    pub extraction_provider: String,
    pub extraction_model: String,
    pub embedding_provider: String,
    pub embedding_model: String,
    pub embedding_dimensions: Option<usize>,
}

struct SourceVersion {
    id: String,
    hash: String,
}

struct PreparationManifest {
    preview: PreparationPreview,
    sources: Vec<SourceVersion>,
    task: DocumentTaskContext,
    embeddings: EmbeddingContext,
    extraction_contract: String,
}

pub(crate) fn preview(
    database: &Database,
    selection: PreparationScope,
) -> Result<PreparationPreview, CommandError> {
    Ok(manifest(database, selection)?.preview)
}

fn manifest(
    database: &Database,
    selection: PreparationScope,
) -> Result<PreparationManifest, CommandError> {
    selection.scope.validate()?;
    let mut task = load_document_task_context(database)?;
    let capabilities = load_document_provider_capabilities(
        database,
        &task.profile_id,
        &task.provider_kind,
        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    let embeddings = embeddings::load_default_embedding_context(database)?;
    let contract = extraction_contract(&task)?;
    let mut seeds = enumerate_sources(database, &selection.scope, selection.include_conversations)?;
    // Original document evidence precedes generated derivatives. The sort is
    // stable, retaining the adapters' canonical order within a source family.
    seeds.sort_by_key(|seed| match seed.kind {
        SourceKind::Document => 0,
        SourceKind::Guide => 1,
        SourceKind::Card => 2,
        SourceKind::Conversation => 3,
    });
    let mut sources = Vec::new();
    let mut source_characters = 0_usize;
    let mut image_count = 0_usize;
    let mut digest = Sha256::new();
    digest.update(selection.key()?.as_bytes());
    digest.update(contract.as_bytes());
    digest.update(format!(
        "{}:{}:{}",
        task.profile_id, task.profile_revision, task.credential_epoch
    ));
    digest.update(embeddings.consent_identity().as_bytes());
    for batch in seeds.chunks(32) {
        for source in materialize_sources(database, batch)? {
            source_characters = source_characters.saturating_add(source.text.chars().count());
            image_count = image_count.saturating_add(source.images.len());
            digest.update(source.id.as_bytes());
            digest.update(source.content_hash.as_bytes());
            sources.push(SourceVersion {
                id: source.id,
                hash: source.content_hash,
            });
            if sources.len() > MAX_PREPARATION_SOURCES {
                return Err(invalid("This selection exceeds the semantic preparation limit. Select a smaller deck or document."));
            }
        }
    }
    if sources.is_empty() {
        return Err(invalid(
            "This selection does not contain material that can be prepared.",
        ));
    }
    let preview = PreparationPreview {
        selection,
        consent_key: format!("{:x}", digest.finalize()),
        source_count: sources.len(),
        source_characters,
        image_count,
        extraction_provider: task.provider_kind.clone(),
        extraction_model: task.model_id.clone(),
        embedding_provider: embeddings
            .request
            .profile
            .provider_kind
            .as_str()
            .to_string(),
        embedding_model: embeddings.request.profile.model_id.clone(),
        embedding_dimensions: embeddings.options.dimensions,
    };
    Ok(PreparationManifest {
        preview,
        sources,
        task,
        embeddings,
        extraction_contract: contract,
    })
}

pub(crate) async fn prepare(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    input: PrepareMaterialInput,
) -> Result<PreparationStatus, CommandError> {
    validate_entity_id(&input.operation_id, "preparation operation")?;
    if !is_hash(&input.consent_key) {
        return Err(invalid(
            "Review the preparation preview before sending material to a provider.",
        ));
    }
    let scope_key = input.selection.key()?;
    if let Some(previous) = preparation_store::find_operation(database, &input.operation_id)? {
        if previous.scope_key != scope_key || previous.consent_key != input.consent_key {
            return Err(invalid(
                "This operation identity was already used for different material.",
            ));
        }
        return Ok(previous);
    }
    let guard = TaskGuard::begin(&input.operation_id, &format!("prepare:{scope_key}"), true)?;
    guard.progress("checking source revisions", 0, 0);
    let manifest = manifest(database, input.selection)?;
    if manifest.preview.consent_key != input.consent_key {
        return Err(CommandError::new(
            "semantic_preparation_changed",
            "The selected material or provider changed after the preview. Review the updated preparation before continuing.",
        ));
    }
    preparation_store::begin(
        database,
        PreparationStart {
            scope_key: &scope_key,
            scope: &manifest.preview.selection.scope,
            operation_id: &input.operation_id,
            consent_key: &input.consent_key,
            embedding_space: &manifest.embeddings.space_id,
            extraction_contract: &manifest.extraction_contract,
            total_sources: manifest.sources.len(),
        },
    )?;
    let outcome = run_preparation(
        database,
        paths,
        runtime,
        &manifest,
        &input.operation_id,
        &guard,
    )
    .await;
    preparation_store::finish(database, &input.operation_id, &outcome)?;
    // The returned durable status reports failure without encouraging an
    // ambiguous replay to submit the same provider work again.
    preparation_store::find_operation(database, &input.operation_id)?
        .ok_or_else(|| invalid("The preparation result is no longer available."))
}

async fn run_preparation(
    database: &Database,
    paths: &AppPaths,
    runtime: &AiRuntimeState,
    manifest: &PreparationManifest,
    operation_id: &str,
    guard: &TaskGuard,
) -> Result<(), CommandError> {
    let mut index = open_index(database, paths)?;
    let mut completed = 0;
    for references in manifest.sources.chunks(12) {
        guard.check()?;
        let sources = database.with_connection(|connection| {
            references
                .iter()
                .map(|reference| {
                    let source = repository::load_source(connection, &reference.id)?
                        .filter(|source| source.content_hash == reference.hash)
                        .ok_or_else(repository::stale_source)?;
                    Ok(source)
                })
                .collect::<Result<Vec<_>, CommandError>>()
        })?;
        for range in batch_ranges(&sources) {
            let batch = &sources[range];
            guard.progress("understanding material", completed, manifest.sources.len());
            let units = ensure_units_batch(
                database,
                paths,
                runtime,
                &manifest.task,
                batch,
                &guard.signal,
            )
            .await?;
            let mut pending = Vec::new();
            let mut seen = HashSet::new();
            for source in batch {
                let entry = source_entry(source);
                if !index.contains(&manifest.embeddings.space_id, &entry.0)?
                    && seen.insert(entry.0.id.clone())
                {
                    pending.push(entry);
                }
                if let Some(source_units) = units.get(&source.id) {
                    for unit in source_units {
                        let entry = unit_entry(unit)?;
                        if !index.contains(&manifest.embeddings.space_id, &entry.0)?
                            && seen.insert(entry.0.id.clone())
                        {
                            pending.push(entry);
                        }
                    }
                }
            }
            guard.progress(
                "indexing semantic content",
                completed,
                manifest.sources.len(),
            );
            for entries in pending.chunks(32) {
                let texts = entries
                    .iter()
                    .map(|entry| entry.1.clone())
                    .collect::<Vec<_>>();
                let vectors = embeddings::embed(
                    database,
                    runtime,
                    &manifest.embeddings,
                    Purpose::Document,
                    &texts,
                    &guard.signal,
                )
                .await?;
                database.with_connection(|connection| {
                    for source in batch {
                        if !repository::source_is_current(connection, source)? {
                            return Err(repository::stale_source());
                        }
                    }
                    Ok(())
                })?;
                let writes = entries
                    .iter()
                    .zip(vectors)
                    .map(|(entry, vector)| IndexWrite {
                        key: entry.0.clone(),
                        vector,
                    })
                    .collect::<Vec<_>>();
                index.write(&manifest.embeddings.space_id, &writes)?;
            }
            completed += batch.len();
            preparation_store::checkpoint(database, operation_id, completed)?;
            guard.progress("prepared material", completed, manifest.sources.len());
            tokio::task::yield_now().await;
        }
    }
    guard.check()?;
    manifest.task.ensure_current(database)?;
    embeddings::ensure_context_current(database, &manifest.embeddings)?;
    database.with_connection(|connection| {
        for reference in &manifest.sources {
            let current: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM knowledge_sources WHERE id = ?1 AND active = 1 AND content_hash = ?2)",
                rusqlite::params![reference.id, reference.hash], |row| row.get(0),
            ).map_err(CommandError::database)?;
            if !current {
                return Err(repository::stale_source());
            }
        }
        Ok(())
    })
}

pub(crate) fn source_entry(source: &SourceRecord) -> (IndexKey, String) {
    let text = if source.text.trim().is_empty() {
        format!(
            "{}\nImage evidence; associated knowledge units contain its interpretation.",
            source.title
        )
    } else {
        format!("{}\n{}", source.title, source.text)
    };
    (
        IndexKey {
            id: format!("source:{}", source.id),
            source_id: source.id.clone(),
            content_hash: source.content_hash.clone(),
        },
        text,
    )
}

pub(crate) fn unit_entry(unit: &KnowledgeUnit) -> Result<(IndexKey, String), CommandError> {
    let content_hash =
        hash(&serde_json::to_string(&unit.content).map_err(CommandError::serialization)?);
    let text = format!(
        "{}\n{}\n{}\n{}",
        unit.content.title,
        unit.content.topic,
        unit.content.statement,
        unit.content.qualifications.join("\n")
    );
    Ok((
        IndexKey {
            id: format!("unit:{}", unit.id),
            source_id: unit.source_id.clone(),
            content_hash,
        },
        text,
    ))
}

pub(crate) fn open_index(
    database: &Database,
    paths: &AppPaths,
) -> Result<VectorIndex, CommandError> {
    let mut index = VectorIndex::open(&paths.app_data_dir().join("cache/semantic-v1.sqlite3"))?;
    index.evict_invalidated(database)?;
    Ok(index)
}

pub(crate) fn clear_index(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
    let _maintenance = runtime::MaintenanceGuard::begin()?;
    // Remove a corrupt or incompatible cache without first needing to open it.
    // These files are derived data, not the user's library or an embedding model.
    for suffix in ["", "-wal", "-shm"] {
        let path = paths
            .app_data_dir()
            .join(format!("cache/semantic-v1.sqlite3{suffix}"));
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(CommandError::io(error)),
        }
    }
    preparation_store::index_cleared(database)
}
