//! Durable document-AI state transitions and bounded projections.
//!
//! This module is the only place the generation executor needs to know the
//! current SQLite job/window schema. Domain planning is validated before any
//! job rows are written, and every lifecycle transition checks its affected-row
//! invariant so stale/canceled state cannot silently look successful.

use std::fs;

use rusqlite::{params, TransactionBehavior};

use crate::{
    ai::document_intelligence::{DocumentTaskContext, SourcePageImage},
    database::Database,
    document_domain::{DocumentSourceDescriptor, SourceLocator},
    document_semantics::{validate_semantic_windows, SemanticChunk, SemanticWindow},
    error::CommandError,
    paths::AppPaths,
    util::{generate_id, unix_time_millis},
};

const MAX_RENDER_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct SourceAssetRecord {
    pub asset_id: String,
    pub source_unit_number: Option<i64>,
    pub media_type: String,
    pub relative_path: String,
    pub byte_size: i64,
    pub sha256: String,
}

/// Temporary PDF-facing projection backed by canonical source assets.
///
/// The current Documents command only needs one canonical render path per PDF
/// page to verify derivative completeness before generation. Visual evidence
/// integrity is checked later from `SourceAssetRecord` when images are loaded.
#[derive(Clone, Debug)]
pub(crate) struct SourceRenderRecord {
    pub relative_path: String,
}

#[derive(Clone, Debug)]
pub(crate) struct GenerationJobIdentity {
    pub profile_id: String,
    pub provider_kind: String,
    pub model_id: String,
    pub profile_revision: i64,
    pub credential_epoch: i64,
    pub network_scope: String,
}

impl From<&DocumentTaskContext> for GenerationJobIdentity {
    fn from(task: &DocumentTaskContext) -> Self {
        Self {
            profile_id: task.profile_id.clone(),
            provider_kind: task.provider_kind.clone(),
            model_id: task.model_id.clone(),
            profile_revision: task.profile_revision,
            credential_epoch: task.credential_epoch,
            network_scope: task.network_scope.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DocumentAiProgressPlan {
    pub estimated_source_tokens: usize,
    pub outline_window_count: usize,
    pub outline_completed_window_count: usize,
    pub generation_window_count: usize,
    pub generation_completed_window_count: usize,
}

pub(crate) fn mark_document_processing(
    database: &Database,
    document_id: &str,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents
                 SET lifecycle_state = 'processing', safe_error_code = '', safe_error_message = '',
                     processing_started_at = ?1, processing_duration_ms = NULL,
                     api_token_count = NULL, api_token_usage_complete = 0,
                     updated_at = ?1, completed_at = NULL, revision = revision + 1
                 WHERE id = ?2 AND archived_at IS NULL",
                params![now, document_id],
            )
            .map_err(|error| db_error("document_mark_processing_failed", document_id, error))?;
        require_one(
            changed,
            "document_changed",
            "The document is no longer available for generation.",
        )
    })
}

pub(crate) fn mark_outline_building(
    database: &Database,
    document_id: &str,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents SET outline_state = 'building', updated_at = ?1
                 WHERE id = ?2 AND lifecycle_state = 'processing'",
                params![now, document_id],
            )
            .map_err(|error| db_error("document_outline_state_failed", document_id, error))?;
        require_one(
            changed,
            "document_changed",
            "The document changed before outline analysis could start.",
        )
    })
}

pub(crate) fn initialize_ai_plan(
    database: &Database,
    document_id: &str,
    scale_mode: &str,
    progress: DocumentAiProgressPlan,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents
                 SET scale_mode = ?1, estimated_source_tokens = ?2,
                     outline_window_count = ?3, outline_completed_window_count = ?4,
                     generation_window_count = ?5, generation_completed_window_count = ?6,
                     updated_at = ?7
                 WHERE id = ?8 AND lifecycle_state = 'processing'",
                params![
                    scale_mode,
                    to_i64(progress.estimated_source_tokens),
                    to_i64(progress.outline_window_count),
                    to_i64(progress.outline_completed_window_count),
                    to_i64(progress.generation_window_count),
                    to_i64(progress.generation_completed_window_count),
                    now,
                    document_id,
                ],
            )
            .map_err(|error| db_error("document_ai_plan_write_failed", document_id, error))?;
        require_one(
            changed,
            "document_changed",
            "The document changed while its AI plan was being recorded.",
        )
    })
}

pub(crate) fn initialize_additional_progress(
    database: &Database,
    document_id: &str,
    scale_mode: &str,
    generation_window_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents
                 SET scale_mode = ?1, generation_window_count = ?2,
                     generation_completed_window_count = 0, updated_at = ?3
                 WHERE id = ?4 AND lifecycle_state = 'processing'",
                params![
                    scale_mode,
                    to_i64(generation_window_count),
                    now,
                    document_id
                ],
            )
            .map_err(|error| {
                db_error("document_additional_plan_write_failed", document_id, error)
            })?;
        require_one(
            changed,
            "document_changed",
            "The document changed before additional generation could start.",
        )
    })
}

pub(crate) fn update_outline_progress(
    database: &Database,
    document_id: &str,
    completed: usize,
) -> Result<(), CommandError> {
    update_progress(
        database,
        document_id,
        "outline_completed_window_count",
        completed,
    )
}

pub(crate) fn update_generation_progress(
    database: &Database,
    document_id: &str,
    completed: usize,
) -> Result<(), CommandError> {
    update_progress(
        database,
        document_id,
        "generation_completed_window_count",
        completed,
    )
}

fn update_progress(
    database: &Database,
    document_id: &str,
    column: &str,
    completed: usize,
) -> Result<(), CommandError> {
    debug_assert!(matches!(
        column,
        "outline_completed_window_count" | "generation_completed_window_count"
    ));
    let maximum_column = if column == "outline_completed_window_count" {
        "outline_window_count"
    } else {
        "generation_window_count"
    };
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                &format!(
                    "UPDATE documents
                     SET {column} = MIN({maximum_column}, MAX(0, ?1)), updated_at = ?2
                     WHERE id = ?3 AND lifecycle_state = 'processing'"
                ),
                params![to_i64(completed), now, document_id],
            )
            .map_err(|error| db_error("document_ai_progress_write_failed", document_id, error))?;
        require_one(
            changed,
            "document_changed",
            "The document changed while AI progress was being recorded.",
        )
    })
}

pub(crate) fn create_generation_job(
    database: &Database,
    document_id: &str,
    identity: &GenerationJobIdentity,
    source: &DocumentSourceDescriptor,
    windows: &[SemanticWindow],
) -> Result<String, CommandError> {
    validate_semantic_windows(windows, source)?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                db_error("document_generation_job_begin_failed", document_id, error)
            })?;
        let job_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO document_generation_jobs (
                     id, document_id, profile_id, provider_kind, model_id,
                     profile_revision, credential_epoch, network_scope, state,
                     page_count, processed_page_count, source_unit_count,
                     processed_source_unit_count, generated_candidate_count,
                     created_at, started_at, updated_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'generating',
                     ?9, 0, ?9, 0, 0, ?10, ?10, ?10
                 )",
                params![
                    job_id,
                    document_id,
                    identity.profile_id,
                    identity.provider_kind,
                    identity.model_id,
                    identity.profile_revision,
                    identity.credential_epoch,
                    identity.network_scope,
                    to_i64(source.unit_count),
                    now,
                ],
            )
            .map_err(|error| {
                db_error_with_job(
                    "document_generation_job_insert_failed",
                    document_id,
                    &job_id,
                    error,
                )
            })?;
        for window in windows {
            let locator = window.source_locator();
            transaction
                .execute(
                    "INSERT INTO document_generation_windows (
                         job_id, window_index, first_chunk_index, last_chunk_index,
                         page_start, page_end, page_numbers_json,
                         source_locator_json, visual_asset_ids_json,
                         token_estimate, state, card_count, updated_at
                     ) VALUES (
                         ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                         ?10, 'pending', 0, ?11
                     )",
                    params![
                        job_id,
                        to_i64(window.window_index),
                        window.first_chunk_index,
                        window.last_chunk_index,
                        locator.first(),
                        locator.last(),
                        serde_json::to_string(&locator.unit_numbers)
                            .map_err(CommandError::serialization)?,
                        serde_json::to_string(&locator).map_err(CommandError::serialization)?,
                        serde_json::to_string(&locator.asset_ids)
                            .map_err(CommandError::serialization)?,
                        to_i64(window.token_estimate),
                        now,
                    ],
                )
                .map_err(|error| {
                    CommandError::database_at(
                        "document_generation_window_insert_failed",
                        format!(
                            "document={document_id} job={job_id} window={} partition={} locator={}",
                            window.window_index,
                            window.partition_index,
                            locator.stable_identity()
                        ),
                        error,
                    )
                })?;
        }
        transaction.commit().map_err(|error| {
            db_error_with_job(
                "document_generation_job_commit_failed",
                document_id,
                &job_id,
                error,
            )
        })?;
        Ok(job_id)
    })
}

pub(crate) fn mark_window_running(
    database: &Database,
    document_id: &str,
    job_id: &str,
    window_index: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE document_generation_windows
                 SET state = 'running', updated_at = ?1
                 WHERE job_id = ?2 AND window_index = ?3 AND state = 'pending'",
                params![now, job_id, to_i64(window_index)],
            )
            .map_err(|error| {
                db_error_with_window(
                    "document_generation_window_start_failed",
                    document_id,
                    job_id,
                    window_index,
                    error,
                )
            })?;
        require_one(
            changed,
            "document_window_changed",
            "The generation window changed before it could start.",
        )
    })
}

pub(crate) fn mark_window_complete(
    database: &Database,
    document_id: &str,
    job_id: &str,
    window: &SemanticWindow,
    card_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                db_error_with_window(
                    "document_generation_window_complete_begin_failed",
                    document_id,
                    job_id,
                    window.window_index,
                    error,
                )
            })?;
        let changed = transaction
            .execute(
                "UPDATE document_generation_windows
                 SET state = ?1, card_count = ?2, safe_error_code = '',
                     safe_error_message = '', updated_at = ?3
                 WHERE job_id = ?4 AND window_index = ?5 AND state IN ('pending', 'running')",
                params![
                    if card_count == 0 {
                        "skipped"
                    } else {
                        "succeeded"
                    },
                    to_i64(card_count),
                    now,
                    job_id,
                    to_i64(window.window_index),
                ],
            )
            .map_err(|error| {
                db_error_with_window(
                    "document_generation_window_complete_failed",
                    document_id,
                    job_id,
                    window.window_index,
                    error,
                )
            })?;
        require_one(
            changed,
            "document_window_changed",
            "The generation window changed before completion could be recorded.",
        )?;
        let locator = window.source_locator();
        let job_changed = transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET processed_page_count = MIN(page_count, MAX(processed_page_count, ?1)),
                     processed_source_unit_count = MIN(
                         source_unit_count,
                         MAX(processed_source_unit_count, ?1)
                     ),
                     generated_candidate_count = (
                         SELECT COALESCE(SUM(card_count), 0)
                         FROM document_generation_windows WHERE job_id = ?2
                     ),
                     updated_at = ?3
                 WHERE id = ?2 AND state = 'generating'",
                params![locator.last(), job_id, now],
            )
            .map_err(|error| {
                db_error_with_job(
                    "document_generation_job_progress_failed",
                    document_id,
                    job_id,
                    error,
                )
            })?;
        require_one(
            job_changed,
            "document_job_changed",
            "The generation job changed before window completion could be recorded.",
        )?;
        transaction.commit().map_err(|error| {
            db_error_with_job(
                "document_generation_window_complete_commit_failed",
                document_id,
                job_id,
                error,
            )
        })
    })
}

pub(crate) fn mark_job_committing(
    database: &Database,
    document_id: &str,
    job_id: &str,
    card_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'committing', generated_candidate_count = ?1, updated_at = ?2
                 WHERE id = ?3 AND document_id = ?4 AND state = 'generating'",
                params![to_i64(card_count), now, job_id, document_id],
            )
            .map_err(|error| {
                db_error_with_job(
                    "document_generation_job_committing_failed",
                    document_id,
                    job_id,
                    error,
                )
            })?;
        require_one(
            changed,
            "document_job_changed",
            "The generation job changed before commit could begin.",
        )
    })
}

pub(crate) fn load_semantic_chunks(
    database: &Database,
    document_id: &str,
) -> Result<Vec<SemanticChunk>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT chunk_index, text, full_text, source_locator_json,
                        element_types_json, heading_context, token_estimate
                 FROM document_semantic_chunk_sources
                 WHERE document_id = ?1 ORDER BY chunk_index",
            )
            .map_err(|error| {
                db_error(
                    "document_semantic_chunks_prepare_failed",
                    document_id,
                    error,
                )
            })?;
        let chunks = statement
            .query_map([document_id], |row| {
                let locator_json: String = row.get(3)?;
                let types_json: String = row.get(4)?;
                let token_estimate: i64 = row.get(6)?;
                let source_locator: SourceLocator =
                    serde_json::from_str(&locator_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            locator_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                let element_types = serde_json::from_str(&types_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        types_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(SemanticChunk {
                    chunk_index: row.get(0)?,
                    text: row.get(1)?,
                    full_text: row.get(2)?,
                    source_locator,
                    element_types,
                    heading_context: row.get(5)?,
                    token_estimate: usize::try_from(token_estimate).unwrap_or(usize::MAX),
                })
            })
            .map_err(|error| db_error("document_semantic_chunks_query_failed", document_id, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                db_error("document_semantic_chunks_decode_failed", document_id, error)
            })?;
        Ok(chunks)
    })
}

pub(crate) fn load_source_assets(
    database: &Database,
    document_id: &str,
) -> Result<Vec<SourceAssetRecord>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT asset_id, source_unit_number, media_type,
                        relative_path, byte_size, sha256
                 FROM document_source_assets
                 WHERE document_id = ?1
                 ORDER BY COALESCE(source_unit_number, 0), asset_id",
            )
            .map_err(|error| {
                db_error("document_source_assets_prepare_failed", document_id, error)
            })?;
        let assets = statement
            .query_map([document_id], |row| {
                Ok(SourceAssetRecord {
                    asset_id: row.get(0)?,
                    source_unit_number: row.get(1)?,
                    media_type: row.get(2)?,
                    relative_path: row.get(3)?,
                    byte_size: row.get(4)?,
                    sha256: row.get(5)?,
                })
            })
            .map_err(|error| db_error("document_source_assets_query_failed", document_id, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                db_error("document_source_assets_decode_failed", document_id, error)
            })?;
        Ok(assets)
    })
}

pub(crate) fn load_source_renders(
    database: &Database,
    document_id: &str,
) -> Result<Vec<SourceRenderRecord>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT relative_path
                 FROM document_source_assets
                 WHERE document_id = ?1
                   AND role = 'unit_render'
                   AND source_unit_number IS NOT NULL
                 ORDER BY source_unit_number",
            )
            .map_err(|error| {
                db_error("document_source_renders_prepare_failed", document_id, error)
            })?;
        let renders = statement
            .query_map([document_id], |row| {
                Ok(SourceRenderRecord {
                    relative_path: row.get(0)?,
                })
            })
            .map_err(|error| db_error("document_source_renders_query_failed", document_id, error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                db_error("document_source_renders_decode_failed", document_id, error)
            })?;
        Ok(renders)
    })
}

pub(crate) fn load_window_images(
    paths: &AppPaths,
    assets: &[SourceAssetRecord],
    window: &SemanticWindow,
    maximum: usize,
) -> Result<Vec<SourcePageImage>, CommandError> {
    if window.source_locator.asset_ids.is_empty() || maximum == 0 {
        return Ok(Vec::new());
    }
    let selected = select_source_assets(&window.source_locator.asset_ids, maximum);
    let mut images = Vec::with_capacity(selected.len());
    for asset_id in selected {
        let asset = assets
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .ok_or_else(|| {
                CommandError::new(
                    "document_source_asset_missing",
                    "A selected canonical source asset is missing.",
                )
            })?;
        if !asset.media_type.starts_with("image/") {
            return Err(CommandError::new(
                "document_source_asset_not_image",
                "A selected multimodal source asset is not an image.",
            ));
        }
        let bytes =
            fs::read(paths.resolve_relative(&asset.relative_path)).map_err(CommandError::io)?;
        if bytes.is_empty()
            || bytes.len() > MAX_RENDER_BYTES
            || to_i64(bytes.len()) != asset.byte_size
            || sha256_hex(&bytes) != asset.sha256
        {
            return Err(CommandError::new(
                "document_source_asset_invalid",
                "A canonical source asset failed its size or integrity check. Retry generation to rebuild it.",
            ));
        }
        images.push(SourcePageImage {
            page_number: asset
                .source_unit_number
                .unwrap_or_else(|| window.source_locator.first()),
            bytes,
        });
    }
    Ok(images)
}

pub(crate) fn select_source_assets(asset_ids: &[String], maximum: usize) -> Vec<String> {
    if maximum == 0 {
        return Vec::new();
    }
    let mut unique = asset_ids.to_vec();
    unique.sort();
    unique.dedup();
    if unique.len() <= maximum {
        return unique;
    }
    if maximum == 1 {
        return vec![unique[unique.len() / 2].clone()];
    }
    let last = unique.len() - 1;
    (0..maximum)
        .map(|index| unique[index * last / (maximum - 1)].clone())
        .collect()
}

fn require_one(changed: usize, code: &str, message: &str) -> Result<(), CommandError> {
    if changed == 1 {
        Ok(())
    } else {
        Err(CommandError::new(code, message))
    }
}

fn db_error(event: &str, document_id: &str, error: impl std::fmt::Display) -> CommandError {
    CommandError::database_at(event, format!("document={document_id}"), error)
}

fn db_error_with_job(
    event: &str,
    document_id: &str,
    job_id: &str,
    error: impl std::fmt::Display,
) -> CommandError {
    CommandError::database_at(event, format!("document={document_id} job={job_id}"), error)
}

fn db_error_with_window(
    event: &str,
    document_id: &str,
    job_id: &str,
    window_index: usize,
    error: impl std::fmt::Display,
) -> CommandError {
    CommandError::database_at(
        event,
        format!("document={document_id} job={job_id} window={window_index}"),
        error,
    )
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_domain::{SourceLocator, SourceUnitKind},
        document_scale::DocumentScalePlan,
        document_semantics::build_partitioned_semantic_windows,
    };

    fn chunk(index: i64, unit: i64) -> SemanticChunk {
        SemanticChunk {
            chunk_index: index,
            text: format!("text {index}"),
            full_text: format!("text {index}"),
            source_locator: SourceLocator::new(
                SourceUnitKind::Page,
                vec![unit],
                Some("One chapter".to_string()),
            )
            .unwrap(),
            element_types: vec!["paragraph".to_string()],
            heading_context: Some("One chapter".to_string()),
            token_estimate: 100,
        }
    }

    #[test]
    fn generation_job_persists_multiple_windows_from_one_partition_with_unique_ids() {
        let database = Database::open_in_memory().expect("database should open");
        let document_id = "0123456789abcdef0123456789abcdef";
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                         id, title, original_filename, mime_type, byte_size, sha256,
                         source_relative_path, lifecycle_state, created_at, updated_at
                     ) VALUES (?1, 'Test', 'test.pdf', 'application/pdf', 1,
                               ?2, 'documents/test/source.pdf', 'processing', 1, 1)",
                        params![document_id, "a".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("document fixture should insert");

        let chunks = (0..11)
            .map(|index| chunk(index, index + 1))
            .collect::<Vec<_>>();
        let plan = DocumentScalePlan::for_provider(11, "llama_cpp");
        let windows = build_partitioned_semantic_windows(&chunks, plan, 10_000, 10, 0);
        assert_eq!(windows.len(), 2);
        let identity = GenerationJobIdentity {
            profile_id: "11111111111111111111111111111111".to_string(),
            provider_kind: "llama_cpp".to_string(),
            model_id: "test-model".to_string(),
            profile_revision: 1,
            credential_epoch: 0,
            network_scope: "local".to_string(),
        };
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO ai_provider_profiles (
                         id, display_name, normalized_name, provider_kind, base_url,
                         model_id, auth_mode, enabled, allow_insecure_http,
                         request_timeout_ms, credential_storage, credential_epoch,
                         revision, created_at, updated_at
                     ) VALUES (?1, 'Test', 'test', 'llama_cpp', 'http://127.0.0.1:8080/',
                               'test-model', 'none', 1, 1, 60000, 'none', 0, 1, 1, 1)",
                        [identity.profile_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("profile fixture should insert");

        let job = create_generation_job(
            &database,
            document_id,
            &identity,
            &DocumentSourceDescriptor::pdf(11),
            &windows,
        )
        .expect("multi-window job should persist");
        let rows = database
            .with_connection(|connection| {
                let mut statement = connection
                    .prepare(
                        "SELECT window_index, source_locator_json
                         FROM document_generation_windows
                         WHERE job_id = ?1 ORDER BY window_index",
                    )
                    .map_err(CommandError::database)?;
                let rows = statement
                    .query_map([job], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })
                    .map_err(CommandError::database)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CommandError::database)?;
                Ok(rows)
            })
            .unwrap();
        assert_eq!(rows.iter().map(|row| row.0).collect::<Vec<_>>(), vec![0, 1]);
        let first_locator: SourceLocator = serde_json::from_str(&rows[0].1).unwrap();
        assert_eq!(first_locator.unit_kind, SourceUnitKind::Page);
    }

    #[test]
    fn invalid_window_identity_is_rejected_before_job_insert() {
        let window = SemanticWindow {
            window_index: 1,
            partition_index: 0,
            partition_title: "Chapter".to_string(),
            first_chunk_index: 0,
            last_chunk_index: 0,
            source_locator: SourceLocator::new(
                SourceUnitKind::Page,
                vec![1],
                Some("Chapter".to_string()),
            )
            .unwrap(),
            page_start: 1,
            page_end: 1,
            token_estimate: 10,
            source_text: "source".to_string(),
        };
        let error =
            validate_semantic_windows(&[window], &DocumentSourceDescriptor::pdf(1)).unwrap_err();
        assert_eq!(error.code, "invalid_document_window_identity");
    }

    #[test]
    fn source_asset_sampling_is_deterministic() {
        let assets = (1..=40)
            .map(|index| format!("asset-{index:03}"))
            .collect::<Vec<_>>();
        assert_eq!(
            select_source_assets(&assets, 3),
            vec![
                "asset-001".to_string(),
                "asset-020".to_string(),
                "asset-040".to_string()
            ]
        );
    }
}
