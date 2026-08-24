use rusqlite::{params, Connection, OptionalExtension};

use crate::error::CommandError;

use super::contracts::DocumentDto;

const MAX_DOCUMENTS_RETURNED: i64 = 500;

#[derive(Clone, Debug)]
pub(crate) struct StoredDocument {
    pub(crate) dto: DocumentDto,
    pub(crate) source_relative_path: String,
}

pub(crate) fn list_document_rows(
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
    let rows = statement
        .query_map(
            params![
                if include_archived { 1_i64 } else { 0_i64 },
                MAX_DOCUMENTS_RETURNED
            ],
            document_row,
        )
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}

pub(crate) fn load_document(
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

pub(crate) fn active_generated_deck_exists(
    connection: &Connection,
    document_id: &str,
) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT
                 EXISTS(
                     SELECT 1
                     FROM document_decks provenance
                     JOIN decks deck ON deck.id = provenance.deck_id
                     WHERE provenance.document_id = ?1
                       AND deck.deleted_at IS NULL
                 )
                 OR EXISTS(
                     SELECT 1
                     FROM documents document
                     JOIN decks deck ON deck.id = document.generated_deck_id
                     WHERE document.id = ?1
                       AND deck.deleted_at IS NULL
                 )",
            [document_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(CommandError::database)
}

pub(crate) fn active_study_guide_exists(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_deck_counts_as_an_available_learning_artifact() {
        let database = crate::database::Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let document_id = "a".repeat(32);
                let deck_id = "b".repeat(32);
                let job_id = "d".repeat(32);
                connection
                    .execute(
                        "INSERT INTO documents (
                             id, title, original_filename, mime_type, byte_size, sha256,
                             source_relative_path, lifecycle_state, created_at, updated_at
                         ) VALUES (?1, 'Document', 'source.pdf', 'application/pdf', 1,
                                   ?2, 'documents/source/source.pdf', 'failed', 1, 1)",
                        rusqlite::params![document_id, "c".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO decks (
                             id, name, description, sort_order, created_at, updated_at,
                             revision
                         ) VALUES (?1, 'Generated', '', 1, 1, 1, 1)",
                        [deck_id.as_str()],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_generation_jobs (
                             id, document_id, provider_kind, model_id, profile_revision,
                             credential_epoch, network_scope, state, page_count,
                             processed_page_count, generated_candidate_count,
                             created_at, updated_at
                         ) VALUES (
                             ?1, ?2, 'openai_compatible', 'fixture', 1,
                             0, 'local', 'succeeded', 1, 1, 0, 1, 1
                         )",
                        rusqlite::params![job_id, document_id],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO document_decks (document_id, deck_id, job_id, created_at)
                         VALUES (?1, ?2, ?3, 1)",
                        rusqlite::params![document_id, deck_id, job_id],
                    )
                    .map_err(CommandError::database)?;
                assert!(active_generated_deck_exists(connection, &document_id)?);
                Ok(())
            })
            .expect("provenance lookup should succeed");
    }
}
