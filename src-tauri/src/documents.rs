use std::fs;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    ai::AiRuntimeState,
    database::Database,
    document_initial_generation::process_and_generate_initial_flashcards,
    error::CommandError,
    paths::AppPaths,
    pdf_render_backend::PdfRenderRuntime,
    util::{unix_time_millis, validate_entity_id},
};

const MAX_DOCUMENTS_RETURNED: i64 = 500;
const INTERRUPTED_ERROR_CODE: &str = "document_generation_interrupted";
const INTERRUPTED_ERROR_MESSAGE: &str =
    "Document processing stopped when ostudy closed. Retry generation to continue.";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryPdfGenerationInput {
    pub document_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetDocumentArchivedInput {
    pub document_id: String,
    pub revision: i64,
    pub archived: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDto {
    pub id: String,
    pub title: String,
    pub original_filename: String,
    pub byte_size: i64,
    pub sha256: String,
    pub lifecycle_state: String,
    pub source_format: String,
    pub source_unit_kind: String,
    pub source_unit_count: i64,
    pub page_count: i64,
    pub extracted_page_count: i64,
    pub rendered_page_count: i64,
    pub extraction_mode: String,
    pub ocr_page_count: i64,
    pub ocr_completed_page_count: i64,
    pub ocr_mean_confidence: Option<f64>,
    pub ocr_engine: String,
    pub scale_mode: String,
    pub estimated_source_tokens: i64,
    pub outline_window_count: i64,
    pub outline_completed_window_count: i64,
    pub generation_window_count: i64,
    pub generation_completed_window_count: i64,
    pub generated_note_count: i64,
    pub generated_card_count: i64,
    pub generated_deck_id: Option<String>,
    pub safe_error_code: String,
    pub safe_error_message: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub revision: i64,
    pub processing_duration_ms: Option<i64>,
    pub api_token_count: Option<i64>,
    pub api_token_usage_complete: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentGenerationResultDto {
    pub document: DocumentDto,
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}

#[derive(Clone, Debug)]
struct StoredDocument {
    dto: DocumentDto,
    source_relative_path: String,
}

#[tauri::command]
pub fn list_documents(
    database: State<'_, Database>,
    include_archived: bool,
) -> Result<Vec<DocumentDto>, CommandError> {
    database.with_connection(|connection| list_document_rows(connection, include_archived))
}

#[tauri::command]
pub fn get_document_source_path(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    document_id: String,
) -> Result<String, CommandError> {
    validate_entity_id(&document_id, "document")?;
    let stored = database.with_connection(|connection| load_document(connection, &document_id))?;
    let source = paths.resolve_relative(&stored.source_relative_path);
    if !source.is_file() {
        return Err(CommandError::new(
            "document_source_missing",
            "The managed source document is missing. Restore the library from backup or import it again.",
        ));
    }
    Ok(source.to_string_lossy().into_owned())
}

pub async fn retry_pdf_flashcard_generation(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    render_runtime: State<'_, PdfRenderRuntime>,
    input: RetryPdfGenerationInput,
) -> Result<DocumentGenerationResultDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let document =
        database.with_connection(|connection| load_document(connection, &input.document_id))?;
    if document.dto.archived_at.is_some() {
        return Err(CommandError::new(
            "document_archived",
            "Restore this document before generating flashcards from it.",
        ));
    }
    if document.dto.generated_deck_id.is_some() {
        return Err(CommandError::new(
            "document_already_generated",
            "This document already has a generated deck. Delete that deck before generating it again.",
        ));
    }

    let result = process_and_generate_initial_flashcards(
        database.inner(),
        paths.inner(),
        runtime.inner(),
        render_runtime.inner().clone(),
        &input.document_id,
    )
    .await;
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            record_document_failure(database.inner(), &input.document_id, &error);
            return Err(error);
        }
    };
    let document = database
        .with_connection(|connection| load_document(connection, &input.document_id))?
        .dto;
    Ok(DocumentGenerationResultDto {
        document,
        deck_id: result.deck_id,
        created_note_count: result.created_note_count,
        created_card_count: result.created_card_count,
        skipped_duplicate_count: result.skipped_duplicate_count,
    })
}

#[tauri::command]
pub fn set_document_archived(
    database: State<'_, Database>,
    input: SetDocumentArchivedInput,
) -> Result<DocumentDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    if input.revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The document revision is invalid.",
        ));
    }

    database.with_connection(|connection| {
        let now = unix_time_millis()?;
        let has_learning_artifact = active_generated_deck_exists(connection, &input.document_id)?
            || active_study_guide_exists(connection, &input.document_id)?;
        let lifecycle = if input.archived {
            "archived"
        } else if has_learning_artifact {
            "ready"
        } else {
            "failed"
        };
        let changed = connection
            .execute(
                "UPDATE documents
                 SET lifecycle_state = ?1, archived_at = ?2, updated_at = ?3,
                     revision = revision + 1
                 WHERE id = ?4 AND revision = ?5",
                params![
                    lifecycle,
                    input.archived.then_some(now),
                    now,
                    input.document_id,
                    input.revision,
                ],
            )
            .map_err(|error| {
                CommandError::database_at(
                    "document_archive_write_failed",
                    format!("document={}", input.document_id),
                    error,
                )
            })?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "This document changed before the archive action completed. Reload it and try again.",
            ));
        }
        Ok(load_document(connection, &input.document_id)?.dto)
    })
}

pub(crate) fn reconcile_startup(database: &Database, paths: &AppPaths) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                CommandError::database_at("document_reconcile_begin_failed", "scope=startup", error)
            })?;
        transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = CASE
                         WHEN EXISTS (
                             SELECT 1 FROM decks deck
                             WHERE deck.id = documents.generated_deck_id
                               AND deck.deleted_at IS NULL
                         ) OR EXISTS (
                             SELECT 1 FROM document_study_guides guide
                             WHERE guide.document_id = documents.id
                               AND guide.state = 'active'
                         ) THEN 'ready'
                         ELSE 'failed'
                     END,
                     outline_state = CASE WHEN outline_state = 'building' THEN 'failed' ELSE outline_state END,
                     safe_error_code = ?1, safe_error_message = ?2,
                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?3 - processing_started_at)
                     END,
                     updated_at = ?3, revision = revision + 1
                 WHERE lifecycle_state = 'processing' AND archived_at IS NULL",
                params![INTERRUPTED_ERROR_CODE, INTERRUPTED_ERROR_MESSAGE, now],
            )
            .map_err(|error| {
                CommandError::database_at("document_reconcile_documents_failed", "scope=startup", error)
            })?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'interrupted', safe_error_code = ?1,
                     safe_error_message = ?2, updated_at = ?3
                 WHERE state IN ('queued', 'extracting', 'generating', 'committing')",
                params![INTERRUPTED_ERROR_CODE, INTERRUPTED_ERROR_MESSAGE, now],
            )
            .map_err(|error| {
                CommandError::database_at("document_reconcile_jobs_failed", "scope=startup", error)
            })?;
        transaction.commit().map_err(|error| {
            CommandError::database_at("document_reconcile_commit_failed", "scope=startup", error)
        })
    })?;

    if paths.document_staging_dir().is_dir() {
        for entry in fs::read_dir(paths.document_staging_dir()).map_err(CommandError::io)? {
            let entry = entry.map_err(CommandError::io)?;
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            } else {
                let _ = fs::remove_file(path);
            }
        }
    }

    let managed_sources = database.with_connection(|connection| {
        let mut statement = connection
            .prepare("SELECT id, source_relative_path FROM documents WHERE archived_at IS NULL")
            .map_err(|error| {
                CommandError::database_at(
                    "document_reconcile_source_query_failed",
                    "scope=startup",
                    error,
                )
            })?;
        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
    })?;
    for (document_id, relative_path) in managed_sources {
        if !paths.resolve_relative(&relative_path).is_file() {
            record_document_failure(
                database,
                &document_id,
                &CommandError::new(
                    "document_source_missing",
                    "The managed source document is missing. Restore the library from backup or import it again.",
                ),
            );
        }
    }
    Ok(())
}

fn record_document_failure(database: &Database, document_id: &str, error: &CommandError) {
    let now = unix_time_millis().unwrap_or_default();
    let usage = crate::task_cancellation::current_usage_summary();
    let (api_token_count, api_token_usage_complete) = usage.unwrap_or((None, false));
    let _ = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = CASE
                         WHEN archived_at IS NOT NULL THEN 'archived'
                         WHEN EXISTS (
                             SELECT 1 FROM decks deck
                             WHERE deck.id = documents.generated_deck_id
                               AND deck.deleted_at IS NULL
                         ) OR EXISTS (
                             SELECT 1 FROM document_study_guides guide
                             WHERE guide.document_id = documents.id
                               AND guide.state = 'active'
                         ) THEN 'ready'
                         ELSE 'failed'
                     END,
                     outline_state = CASE WHEN outline_state = 'building' THEN 'failed' ELSE outline_state END,
                     safe_error_code = ?1, safe_error_message = ?2,
                     processing_duration_ms = CASE
                         WHEN processing_started_at IS NULL THEN processing_duration_ms
                         ELSE MAX(0, ?3 - processing_started_at)
                     END,
                     api_token_count = CASE WHEN ?4 IS NULL THEN api_token_count ELSE ?4 END,
                     api_token_usage_complete = CASE WHEN ?4 IS NULL THEN api_token_usage_complete ELSE ?5 END,
                     updated_at = ?3, revision = revision + 1
                 WHERE id = ?6",
                params![
                    bounded_error(&error.code, 80),
                    bounded_error(&error.message, 1000),
                    now,
                    api_token_count,
                    if api_token_usage_complete { 1_i64 } else { 0_i64 },
                    document_id,
                ],
            )
            .map_err(|db_error| {
                CommandError::database_at(
                    "document_failure_state_write_failed",
                    format!("document={document_id} original_code={}", error.code),
                    db_error,
                )
            })?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'failed', safe_error_code = ?1, safe_error_message = ?2,
                     updated_at = ?3, completed_at = ?3
                 WHERE document_id = ?4
                   AND state IN ('queued', 'extracting', 'generating', 'committing')",
                params![
                    bounded_error(&error.code, 80),
                    bounded_error(&error.message, 1000),
                    now,
                    document_id,
                ],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    });
}

fn list_document_rows(
    connection: &Connection,
    include_archived: bool,
) -> Result<Vec<DocumentDto>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, title, original_filename, byte_size, sha256, lifecycle_state,
                    page_count, extracted_page_count, rendered_page_count,
                    extraction_mode, ocr_page_count, ocr_completed_page_count,
                    ocr_mean_confidence, ocr_engine,
                    scale_mode, estimated_source_tokens,
                    outline_window_count, outline_completed_window_count,
                    generation_window_count, generation_completed_window_count,
                    generated_note_count, generated_card_count,
                    CASE WHEN EXISTS (
                        SELECT 1 FROM decks deck
                        WHERE deck.id = documents.generated_deck_id AND deck.deleted_at IS NULL
                    ) THEN documents.generated_deck_id ELSE NULL END,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision,
                    processing_duration_ms, api_token_count, api_token_usage_complete,
                    source_format, source_unit_kind, source_unit_count
             FROM documents
             WHERE (?1 = 1 OR archived_at IS NULL)
             ORDER BY updated_at DESC, id
             LIMIT ?2",
        )
        .map_err(CommandError::database)?;
    statement
        .query_map(
            params![
                if include_archived { 1_i64 } else { 0_i64 },
                MAX_DOCUMENTS_RETURNED
            ],
            document_row,
        )
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

fn load_document(
    connection: &Connection,
    document_id: &str,
) -> Result<StoredDocument, CommandError> {
    connection
        .query_row(
            "SELECT id, title, original_filename, byte_size, sha256, lifecycle_state,
                    page_count, extracted_page_count, rendered_page_count,
                    extraction_mode, ocr_page_count, ocr_completed_page_count,
                    ocr_mean_confidence, ocr_engine,
                    scale_mode, estimated_source_tokens,
                    outline_window_count, outline_completed_window_count,
                    generation_window_count, generation_completed_window_count,
                    generated_note_count, generated_card_count,
                    CASE WHEN EXISTS (
                        SELECT 1 FROM decks deck
                        WHERE deck.id = documents.generated_deck_id AND deck.deleted_at IS NULL
                    ) THEN documents.generated_deck_id ELSE NULL END,
                    safe_error_code, safe_error_message, created_at, updated_at,
                    completed_at, archived_at, revision,
                    processing_duration_ms, api_token_count, api_token_usage_complete,
                    source_format, source_unit_kind, source_unit_count,
                    source_relative_path
             FROM documents WHERE id = ?1",
            [document_id],
            |row| {
                Ok(StoredDocument {
                    dto: document_row(row)?,
                    source_relative_path: row.get(36)?,
                })
            },
        )
        .optional()
        .map_err(CommandError::database)?
        .ok_or_else(|| {
            CommandError::new(
                "document_not_found",
                "That document is no longer in the library.",
            )
        })
}

fn document_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentDto> {
    Ok(DocumentDto {
        id: row.get(0)?,
        title: row.get(1)?,
        original_filename: row.get(2)?,
        byte_size: row.get(3)?,
        sha256: row.get(4)?,
        lifecycle_state: row.get(5)?,
        source_format: row.get(33)?,
        source_unit_kind: row.get(34)?,
        source_unit_count: row.get(35)?,
        page_count: row.get(6)?,
        extracted_page_count: row.get(7)?,
        rendered_page_count: row.get(8)?,
        extraction_mode: row.get(9)?,
        ocr_page_count: row.get(10)?,
        ocr_completed_page_count: row.get(11)?,
        ocr_mean_confidence: row.get(12)?,
        ocr_engine: row.get(13)?,
        scale_mode: row.get(14)?,
        estimated_source_tokens: row.get(15)?,
        outline_window_count: row.get(16)?,
        outline_completed_window_count: row.get(17)?,
        generation_window_count: row.get(18)?,
        generation_completed_window_count: row.get(19)?,
        generated_note_count: row.get(20)?,
        generated_card_count: row.get(21)?,
        generated_deck_id: row.get(22)?,
        safe_error_code: row.get(23)?,
        safe_error_message: row.get(24)?,
        created_at: row.get(25)?,
        updated_at: row.get(26)?,
        completed_at: row.get(27)?,
        archived_at: row.get(28)?,
        revision: row.get(29)?,
        processing_duration_ms: row.get(30)?,
        api_token_count: row.get(31)?,
        api_token_usage_complete: row.get::<_, i64>(32)? != 0,
    })
}

fn active_generated_deck_exists(
    connection: &Connection,
    document_id: &str,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM documents document
                 JOIN decks deck ON deck.id = document.generated_deck_id
                 WHERE document.id = ?1 AND deck.deleted_at IS NULL
             )",
            [document_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

fn active_study_guide_exists(
    connection: &Connection,
    document_id: &str,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM document_study_guides
                 WHERE document_id = ?1 AND state = 'active'
             )",
            [document_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

fn bounded_error(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_only_document_counts_as_ready_learning_artifact() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                         id, title, original_filename, mime_type, byte_size, sha256,
                         source_relative_path, lifecycle_state, created_at, updated_at
                     ) VALUES (?1, 'Guide only', 'guide.pdf', 'application/pdf', 1,
                               ?2, 'documents/guide/source.pdf', 'failed', 1, 1)",
                        params!["a".repeat(32), "b".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_learning_plans (
                         id, document_id, revision, compiler_version, outline_fingerprint,
                         plan_fingerprint, summary_text, learning_objectives_json,
                         key_concepts_json, state, created_at, updated_at
                     ) VALUES (?1, ?2, 1, 1, ?3, ?4, 'Summary', '[]', '[]', 'active', 1, 1)",
                        params![
                            "c".repeat(32),
                            "a".repeat(32),
                            "d".repeat(64),
                            "e".repeat(64)
                        ],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_study_guides (
                         id, document_id, plan_id, revision, schema_version, prompt_version,
                         assembly_version, overview_json, overview_text, state,
                         created_at, updated_at, published_at
                     ) VALUES (?1, ?2, ?3, 1, 1, 1, 1, '{}', 'Summary', 'active', 1, 1, 1)",
                        params!["f".repeat(32), "a".repeat(32), "c".repeat(32)],
                    )
                    .map_err(CommandError::database)?;
                assert!(active_study_guide_exists(connection, &"a".repeat(32))?);
                assert!(!active_generated_deck_exists(connection, &"a".repeat(32))?);
                Ok(())
            })
            .unwrap();
    }
}
