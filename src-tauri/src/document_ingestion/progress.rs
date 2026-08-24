use rusqlite::params;

use crate::{
    database::Database, error::CommandError, pdf_document_ingestion, util::unix_time_millis,
};

use super::{
    contracts::IngestionProgress,
    support::{require_one, to_i64},
};

pub(crate) fn update_ingestion_progress(
    database: &Database,
    document_id: &str,
    progress: IngestionProgress,
) -> Result<(), CommandError> {
    match progress {
        IngestionProgress::Extracting {
            completed_units,
            total_units,
        } => {
            let total = to_i64(total_units);
            let completed = to_i64(completed_units).min(total);
            database.with_connection(|connection| {
                let changed = connection
                    .execute(
                        "UPDATE documents
                         SET page_count = ?1, extracted_page_count = ?2,
                             source_unit_count = ?1,
                             extracted_source_unit_count = ?2,
                             updated_at = ?3
                         WHERE id = ?4 AND lifecycle_state = 'processing'",
                        params![total, completed, unix_time_millis()?, document_id],
                    )
                    .map_err(|error| {
                        CommandError::database_at(
                            "canonical_document_progress_failed",
                            format!("document={document_id} completed={completed} total={total}"),
                            error,
                        )
                    })?;
                require_one(
                    changed,
                    "document_changed",
                    "The document changed while extraction progress was being recorded.",
                )
            })
        }
        IngestionProgress::Rendering {
            completed_units,
            total_units,
        } => {
            pdf_document_ingestion::update_ingestion_progress(
                database,
                document_id,
                pdf_document_ingestion::IngestionProgress::Rendering {
                    completed_units,
                    total_units,
                },
            )?;
            let total = to_i64(total_units);
            let completed = to_i64(completed_units).min(total);
            database.with_connection(|connection| {
                let changed = connection
                    .execute(
                        "UPDATE documents
                         SET source_unit_count = ?1,
                             extracted_source_unit_count = ?1,
                             visualized_source_unit_count = ?2
                         WHERE id = ?3 AND lifecycle_state = 'processing'",
                        params![total, completed, document_id],
                    )
                    .map_err(|error| {
                        CommandError::database_at(
                            "canonical_document_progress_failed",
                            format!("document={document_id} completed={completed} total={total}"),
                            error,
                        )
                    })?;
                require_one(
                    changed,
                    "document_changed",
                    "The document changed while rendering progress was being recorded.",
                )
            })
        }
        IngestionProgress::Ocr {
            completed_units,
            total_units,
        } => pdf_document_ingestion::update_ingestion_progress(
            database,
            document_id,
            pdf_document_ingestion::IngestionProgress::Ocr {
                completed_units,
                total_units,
            },
        ),
    }
}
