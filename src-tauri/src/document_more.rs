//! Additional flashcard generation for an already-generated managed document.
//!
//! A second pass never creates a replacement deck. It reuses the durable source
//! semantics and outline, summarizes knowledge already tested by the exact
//! generated deck, asks the configured model for materially different coverage,
//! and appends through the canonical generated-import boundary.

use std::{collections::HashSet, fs};

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::{
    ai::{document_intelligence, AiRuntimeState},
    authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit,
    database::Database,
    document_semantics::{build_semantic_windows, SemanticChunk, SemanticWindow},
    error::CommandError,
    paths::AppPaths,
    task_cancellation::run_document_task,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const GENERATION_WINDOW_MAX_TOKENS: usize = 20_000;
const GENERATION_WINDOW_MAX_PAGES: usize = 40;
const GENERATION_WINDOW_OVERLAP_TOKENS: usize = 2_000;
const MAX_IMAGES_PER_GENERATION_WINDOW: usize = 6;
const MAX_RENDER_BYTES: usize = 32 * 1024 * 1024;
const MAX_GENERATED_CARDS: usize = 10_000;
const MAX_EXISTING_COVERAGE_CHARS: usize = 36_000;
const CANCELED_MESSAGE: &str =
    "Additional flashcard generation was stopped. Existing cards were not changed.";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateMorePdfFlashcardsInput {
    pub document_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateMorePdfFlashcardsResultDto {
    pub document_id: String,
    pub deck_id: String,
    pub created_note_count: i64,
    pub created_card_count: i64,
    pub skipped_duplicate_count: i64,
}

#[derive(Clone, Debug)]
struct AdditionalDocumentContext {
    title: String,
    deck_id: String,
    deck_name: String,
    page_count: i64,
}

#[derive(Clone, Debug)]
struct PageRenderRecord {
    page_number: i64,
    render_relative_path: String,
    render_byte_size: i64,
    render_sha256: String,
}

#[tauri::command]
pub async fn generate_more_pdf_flashcards(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    input: GenerateMorePdfFlashcardsInput,
) -> Result<GenerateMorePdfFlashcardsResultDto, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    let document_id = input.document_id.clone();
    let database_after = database.clone();
    let result = run_document_task(
        &document_id,
        generate_more_impl(database, paths, runtime, input),
    )
    .await;

    match result {
        Ok(value) => Ok(value),
        Err(error) if error.code == "ai_request_canceled" => {
            record_additional_failure(
                database_after.inner(),
                &document_id,
                "document_generation_canceled",
                CANCELED_MESSAGE,
            );
            Err(CommandError::new(
                "document_generation_canceled",
                CANCELED_MESSAGE,
            ))
        }
        Err(error) => {
            record_additional_failure(
                database_after.inner(),
                &document_id,
                &error.code,
                &error.message,
            );
            Err(error)
        }
    }
}

async fn generate_more_impl(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    runtime: State<'_, AiRuntimeState>,
    input: GenerateMorePdfFlashcardsInput,
) -> Result<GenerateMorePdfFlashcardsResultDto, CommandError> {
    let context = load_context(database.inner(), &input.document_id)?;
    ensure_exact_append_destination(database.inner(), &context.deck_id, &context.deck_name)?;
    mark_additional_processing(database.inner(), &input.document_id)?;

    let outline = load_outline(database.inner(), &input.document_id)?;
    let existing_coverage =
        load_existing_coverage(database.inner(), &context.deck_id, MAX_EXISTING_COVERAGE_CHARS)?;
    let second_pass_outline = second_pass_outline(outline, &existing_coverage);
    let chunks = load_semantic_chunks(database.inner(), &input.document_id)?;
    if chunks.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The retained PDF has no semantic source chunks available for another generation pass.",
        ));
    }
    let windows = build_semantic_windows(
        &chunks,
        GENERATION_WINDOW_MAX_TOKENS,
        GENERATION_WINDOW_MAX_PAGES,
        GENERATION_WINDOW_OVERLAP_TOKENS,
    );
    if windows.is_empty() {
        return Err(CommandError::new(
            "document_has_no_semantic_content",
            "The retained PDF has no source windows available for another generation pass.",
        ));
    }

    let task = document_intelligence::load_document_task_context(database.inner())?;
    let pages = if task.include_page_images {
        load_page_renders(database.inner(), &input.document_id)?
    } else {
        Vec::new()
    };
    let job_id = create_additional_job(
        database.inner(),
        &input.document_id,
        &task,
        context.page_count,
        &windows,
    )?;

    let mut generated = Vec::<Value>::new();
    for window in &windows {
        mark_window_running(database.inner(), &job_id, window.window_index)?;
        let images = if task.include_page_images {
            load_window_images(paths.inner(), &pages, window)?
        } else {
            Vec::new()
        };
        let cards = document_intelligence::generate_window_flashcards(
            runtime.inner(),
            &task,
            &context.title,
            &second_pass_outline,
            window,
            &images,
        )
        .await?;
        if generated.len().saturating_add(cards.len()) > MAX_GENERATED_CARDS {
            return Err(CommandError::new(
                "too_many_generated_cards",
                format!(
                    "The additional pass generated more than {MAX_GENERATED_CARDS} candidates."
                ),
            ));
        }
        mark_window_complete(database.inner(), &job_id, window, cards.len())?;
        generated.extend(cards);
    }

    if generated.is_empty() {
        return Err(CommandError::new(
            "document_no_additional_cards",
            "The second pass did not find important material that was not already tested by the existing deck.",
        ));
    }
    let generated = document_intelligence::curate_document_cards(
        runtime.inner(),
        &task,
        &second_pass_outline,
        generated,
    )
    .await?;
    if generated.is_empty() {
        return Err(CommandError::new(
            "document_no_additional_cards",
            "The second-pass relevance check found no additional cards worth adding to the existing deck.",
        ));
    }

    mark_job_committing(database.inner(), &job_id, generated.len())?;
    ensure_exact_append_destination(database.inner(), &context.deck_id, &context.deck_name)?;
    let payload = build_append_payload(&context.deck_name, generated)?;
    let result = import_generated_flashcards_bytes_with_commit(
        database.inner(),
        payload.as_bytes(),
        |connection, import_result, now| {
            if import_result.deck_id != context.deck_id {
                return Err(CommandError::new(
                    "document_generated_deck_changed",
                    "The generated deck changed before additional cards could be committed. No cards were added.",
                ));
            }
            connection
                .execute(
                    "UPDATE documents
                     SET lifecycle_state = 'ready',
                         generated_note_count = generated_note_count + ?1,
                         generated_card_count = generated_card_count + ?2,
                         safe_error_code = '', safe_error_message = '',
                         updated_at = ?3, completed_at = ?3, revision = revision + 1
                     WHERE id = ?4 AND generated_deck_id = ?5",
                    params![
                        import_result.created_note_count,
                        import_result.created_card_count,
                        now,
                        input.document_id,
                        context.deck_id,
                    ],
                )
                .map_err(CommandError::database)?;
            connection
                .execute(
                    "UPDATE document_generation_jobs
                     SET state = 'succeeded', generated_deck_id = ?1,
                         safe_error_code = '', safe_error_message = '',
                         updated_at = ?2, completed_at = ?2
                     WHERE id = ?3",
                    params![context.deck_id, now, job_id],
                )
                .map_err(CommandError::database)?;
            Ok(())
        },
    )?;

    Ok(GenerateMorePdfFlashcardsResultDto {
        document_id: input.document_id,
        deck_id: result.deck_id,
        created_note_count: result.created_note_count,
        created_card_count: result.created_card_count,
        skipped_duplicate_count: result.skipped_duplicate_count,
    })
}

fn load_context(
    database: &Database,
    document_id: &str,
) -> Result<AdditionalDocumentContext, CommandError> {
    database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT document.title, document.page_count,
                        document.generated_deck_id, document.archived_at,
                        deck.name, deck.deleted_at, deck.archived_at
                 FROM documents document
                 LEFT JOIN decks deck ON deck.id = document.generated_deck_id
                 WHERE document.id = ?1",
                [document_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_not_found",
                    "That PDF is no longer in the document library.",
                )
            })
            .and_then(
                |(title, page_count, deck_id, document_archived, deck_name, deck_deleted, deck_archived)| {
                    if document_archived.is_some() {
                        return Err(CommandError::new(
                            "document_archived",
                            "Restore this PDF before generating more flashcards from it.",
                        ));
                    }
                    let deck_id = deck_id.ok_or_else(|| {
                        CommandError::new(
                            "document_generated_deck_missing",
                            "This PDF does not currently have a generated deck to extend.",
                        )
                    })?;
                    if deck_deleted.is_some() || deck_archived.is_some() {
                        return Err(CommandError::new(
                            "document_generated_deck_unavailable",
                            "Restore the generated deck before adding more cards to it.",
                        ));
                    }
                    let deck_name = deck_name.ok_or_else(|| {
                        CommandError::new(
                            "document_generated_deck_missing",
                            "The generated deck is no longer available.",
                        )
                    })?;
                    Ok(AdditionalDocumentContext {
                        title,
                        deck_id,
                        deck_name,
                        page_count,
                    })
                },
            )
    })
}

fn load_outline(
    database: &Database,
    document_id: &str,
) -> Result<document_intelligence::DocumentOutline, CommandError> {
    let json = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT outline_json FROM document_outlines WHERE document_id = ?1",
                [document_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_outline_missing",
                    "The retained document outline is missing. Retry normal generation to rebuild the document before requesting more cards.",
                )
            })
    })?;
    serde_json::from_str(&json).map_err(|_| {
        CommandError::new(
            "document_outline_invalid",
            "The retained document outline could not be read. Rebuild the document before requesting more cards.",
        )
    })
}

fn load_semantic_chunks(
    database: &Database,
    document_id: &str,
) -> Result<Vec<SemanticChunk>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT chunk_index, text, full_text, page_numbers_json,
                        element_types_json, heading_context, token_estimate
                 FROM document_semantic_chunks
                 WHERE document_id = ?1 ORDER BY chunk_index",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([document_id], |row| {
                let pages_json: String = row.get(3)?;
                let types_json: String = row.get(4)?;
                let token_estimate: i64 = row.get(6)?;
                let page_numbers = serde_json::from_str(&pages_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        pages_json.len(),
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
                    page_numbers,
                    element_types,
                    heading_context: row.get(5)?,
                    token_estimate: usize::try_from(token_estimate).unwrap_or(usize::MAX),
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
    })
}

fn load_existing_coverage(
    database: &Database,
    deck_id: &str,
    maximum_chars: usize,
) -> Result<String, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT front_text, back_text
                 FROM notes
                 WHERE deck_id = ?1 AND deleted_at IS NULL
                 ORDER BY created_at, id",
            )
            .map_err(CommandError::database)?;
        let mut summary = String::new();
        let rows = statement
            .query_map([deck_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (front, back) = row.map_err(CommandError::database)?;
            let entry = format!(
                "- Already tested: {} => {}\n",
                compact_text(&front, 360),
                compact_text(&back, 500)
            );
            if summary.chars().count().saturating_add(entry.chars().count()) > maximum_chars {
                summary.push_str("- Additional existing cards omitted from this bounded coverage summary.\n");
                break;
            }
            summary.push_str(&entry);
        }
        if summary.is_empty() {
            summary.push_str("- No readable existing card coverage was found.\n");
        }
        Ok(summary)
    })
}

fn second_pass_outline(
    mut outline: document_intelligence::DocumentOutline,
    existing_coverage: &str,
) -> document_intelligence::DocumentOutline {
    let original_summary = outline.summary.clone();
    outline.summary = format!(
        "SECOND-PASS FLASHCARD CONSTRAINT: The existing generated deck already tests the knowledge listed below. Generate additional cards only for important concepts, mechanisms, distinctions, applications, or relationships that are not already meaningfully tested. Do not create paraphrases of existing questions merely to increase card count.\n\n<ostudy-existing-card-coverage>\n{existing_coverage}</ostudy-existing-card-coverage>\n\nOriginal document summary: {original_summary}"
    );
    outline.learning_objectives.insert(
        0,
        "Second-pass objective: fill important learning-objective coverage gaps that are not already tested by the existing deck; avoid paraphrased duplicates.".to_string(),
    );
    outline
}

fn load_page_renders(
    database: &Database,
    document_id: &str,
) -> Result<Vec<PageRenderRecord>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT page_number, render_relative_path, render_byte_size, render_sha256
                 FROM document_pages WHERE document_id = ?1 ORDER BY page_number",
            )
            .map_err(CommandError::database)?;
        statement
            .query_map([document_id], |row| {
                Ok(PageRenderRecord {
                    page_number: row.get(0)?,
                    render_relative_path: row.get(1)?,
                    render_byte_size: row.get(2)?,
                    render_sha256: row.get(3)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)
    })
}

fn load_window_images(
    paths: &AppPaths,
    pages: &[PageRenderRecord],
    window: &SemanticWindow,
) -> Result<Vec<document_intelligence::SourcePageImage>, CommandError> {
    let selected = select_image_pages(&window.page_numbers, MAX_IMAGES_PER_GENERATION_WINDOW);
    let mut images = Vec::with_capacity(selected.len());
    for page_number in selected {
        let page = pages
            .iter()
            .find(|page| page.page_number == page_number)
            .ok_or_else(|| {
                CommandError::new(
                    "document_render_missing",
                    "A rendered page required by the additional pass is missing.",
                )
            })?;
        let bytes = fs::read(paths.resolve_relative(&page.render_relative_path))
            .map_err(CommandError::io)?;
        if bytes.is_empty()
            || bytes.len() > MAX_RENDER_BYTES
            || i64::try_from(bytes.len()).unwrap_or(i64::MAX) != page.render_byte_size
            || sha256_hex(&bytes) != page.render_sha256
        {
            return Err(CommandError::new(
                "document_render_invalid",
                "A retained rendered PDF page failed its integrity check. Rebuild the document before generating more cards.",
            ));
        }
        images.push(document_intelligence::SourcePageImage { page_number, bytes });
    }
    Ok(images)
}

fn create_additional_job(
    database: &Database,
    document_id: &str,
    task: &document_intelligence::DocumentTaskContext,
    page_count: i64,
    windows: &[SemanticWindow],
) -> Result<String, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let job_id = generate_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO document_generation_jobs (
                     id, document_id, profile_id, provider_kind, model_id,
                     profile_revision, credential_epoch, network_scope, state,
                     page_count, processed_page_count, generated_candidate_count,
                     created_at, started_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'generating', ?9, 0, 0, ?10, ?10, ?10)",
                params![
                    job_id,
                    document_id,
                    task.profile_id,
                    task.provider_kind,
                    task.model_id,
                    task.profile_revision,
                    task.credential_epoch,
                    task.network_scope,
                    page_count,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        for window in windows {
            transaction
                .execute(
                    "INSERT INTO document_generation_windows (
                         job_id, window_index, first_chunk_index, last_chunk_index,
                         page_start, page_end, page_numbers_json, token_estimate,
                         state, card_count, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', 0, ?9)",
                    params![
                        job_id,
                        i64::try_from(window.window_index).unwrap_or(i64::MAX),
                        window.first_chunk_index,
                        window.last_chunk_index,
                        window.page_start,
                        window.page_end,
                        serde_json::to_string(&window.page_numbers)
                            .map_err(CommandError::serialization)?,
                        i64::try_from(window.token_estimate).unwrap_or(i64::MAX),
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }
        transaction.commit().map_err(CommandError::database)?;
        Ok(job_id)
    })
}

fn mark_additional_processing(database: &Database, document_id: &str) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE documents
                 SET lifecycle_state = 'processing', safe_error_code = '', safe_error_message = '',
                     updated_at = ?1, completed_at = NULL, revision = revision + 1
                 WHERE id = ?2 AND archived_at IS NULL AND generated_deck_id IS NOT NULL",
                params![now, document_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "The PDF changed before additional generation could begin. Reload Documents and try again.",
            ));
        }
        Ok(())
    })
}

fn mark_window_running(
    database: &Database,
    job_id: &str,
    window_index: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE document_generation_windows
                 SET state = 'running', updated_at = ?1
                 WHERE job_id = ?2 AND window_index = ?3",
                params![now, job_id, i64::try_from(window_index).unwrap_or(i64::MAX)],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn mark_window_complete(
    database: &Database,
    job_id: &str,
    window: &SemanticWindow,
    card_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_windows
                 SET state = ?1, card_count = ?2, safe_error_code = '', safe_error_message = '', updated_at = ?3
                 WHERE job_id = ?4 AND window_index = ?5",
                params![
                    if card_count == 0 { "skipped" } else { "succeeded" },
                    i64::try_from(card_count).unwrap_or(i64::MAX),
                    now,
                    job_id,
                    i64::try_from(window.window_index).unwrap_or(i64::MAX),
                ],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET processed_page_count = MAX(processed_page_count, ?1),
                     generated_candidate_count = (
                         SELECT COALESCE(SUM(card_count), 0)
                         FROM document_generation_windows WHERE job_id = ?2
                     ), updated_at = ?3
                 WHERE id = ?2",
                params![window.page_end, job_id, now],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })
}

fn mark_job_committing(
    database: &Database,
    job_id: &str,
    card_count: usize,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE document_generation_jobs
                 SET state = 'committing', generated_candidate_count = ?1, updated_at = ?2
                 WHERE id = ?3",
                params![i64::try_from(card_count).unwrap_or(i64::MAX), now, job_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

fn record_additional_failure(
    database: &Database,
    document_id: &str,
    code: &str,
    message: &str,
) {
    let now = unix_time_millis().unwrap_or_default();
    let _ = database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = CASE
                         WHEN archived_at IS NOT NULL THEN 'archived'
                         WHEN generated_deck_id IS NOT NULL THEN 'ready'
                         ELSE 'failed'
                     END,
                     safe_error_code = ?1, safe_error_message = ?2,
                     updated_at = ?3, revision = revision + 1
                 WHERE id = ?4",
                params![bounded(code, 80), bounded(message, 1000), now, document_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "UPDATE document_generation_jobs
                 SET state = CASE WHEN ?1 = 'document_generation_canceled' THEN 'canceled' ELSE 'failed' END,
                     safe_error_code = ?1, safe_error_message = ?2,
                     updated_at = ?3, completed_at = ?3
                 WHERE document_id = ?4
                   AND state IN ('queued', 'extracting', 'generating', 'committing')",
                params![bounded(code, 80), bounded(message, 1000), now, document_id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    });
}

fn ensure_exact_append_destination(
    database: &Database,
    expected_deck_id: &str,
    deck_name: &str,
) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id FROM decks
                 WHERE name = ?1 AND deleted_at IS NULL AND archived_at IS NULL
                 ORDER BY created_at, id",
            )
            .map_err(CommandError::database)?;
        let ids = statement
            .query_map([deck_name], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        match ids.as_slice() {
            [only] if only == expected_deck_id => Ok(()),
            [..] if ids.is_empty() => Err(CommandError::new(
                "document_generated_deck_missing",
                "The generated deck is no longer available for additional cards.",
            )),
            _ => Err(CommandError::new(
                "document_generated_deck_name_ambiguous",
                "More than one active deck has the generated deck's name. Rename one of those decks before generating more cards so ostudy cannot append to the wrong destination.",
            )),
        }
    })
}

fn build_append_payload(deck_name: &str, cards: Vec<Value>) -> Result<String, CommandError> {
    serde_json::to_string(&serde_json::json!({
        "format": "ostudy-flashcards",
        "formatVersion": 1,
        "deck": {
            "mode": "append",
            "name": deck_name
        },
        "defaultTags": ["AI generated", "PDF"],
        "cards": cards
    }))
    .map_err(CommandError::serialization)
}

fn select_image_pages(page_numbers: &[i64], maximum: usize) -> Vec<i64> {
    if maximum == 0 {
        return Vec::new();
    }
    let mut unique = page_numbers.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() <= maximum {
        return unique;
    }
    if maximum == 1 {
        return vec![unique[unique.len() / 2]];
    }
    let last = unique.len() - 1;
    (0..maximum)
        .map(|index| unique[index * last / (maximum - 1)])
        .collect()
}

fn compact_text(value: &str, maximum: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(maximum)
        .collect()
}

fn bounded(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outline() -> document_intelligence::DocumentOutline {
        document_intelligence::DocumentOutline {
            summary: "Original summary".to_string(),
            learning_objectives: vec!["Understand pulmonary perfusion".to_string()],
            key_concepts: vec!["V/Q matching".to_string()],
            sections: Vec::new(),
        }
    }

    #[test]
    fn second_pass_planning_context_names_existing_coverage_as_a_constraint() {
        let augmented = second_pass_outline(
            outline(),
            "- Already tested: What is V/Q matching? => Relationship of ventilation to perfusion.\n",
        );
        assert!(augmented.summary.contains("SECOND-PASS FLASHCARD CONSTRAINT"));
        assert!(augmented.summary.contains("Already tested"));
        assert!(augmented.learning_objectives[0].contains("coverage gaps"));
    }

    #[test]
    fn image_sampling_covers_a_large_window_without_duplicate_pages() {
        let pages = (1..=40).collect::<Vec<_>>();
        let selected = select_image_pages(&pages, 6);
        assert_eq!(selected.len(), 6);
        assert_eq!(selected.first(), Some(&1));
        assert_eq!(selected.last(), Some(&40));
        assert_eq!(selected.iter().collect::<HashSet<_>>().len(), 6);
    }
}
