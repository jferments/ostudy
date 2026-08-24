use rusqlite::params;

use crate::{database::Database, error::CommandError, util::unix_time_millis};

use super::contracts::IngestionProgress;

pub(crate) fn update_ingestion_progress(
    database: &Database,
    document_id: &str,
    progress: IngestionProgress,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = match progress {
            IngestionProgress::Rendering {
                completed_units,
                total_units,
            } => {
                let total = to_i64(total_units);
                let completed = to_i64(completed_units).min(total);
                connection
                    .execute(
                        "UPDATE documents
                         SET page_count = ?1, extracted_page_count = ?1,
                             rendered_page_count = ?2,
                             extraction_mode = 'native', ocr_page_count = 0,
                             ocr_completed_page_count = 0, ocr_mean_confidence = NULL,
                             ocr_engine = '', updated_at = ?3
                         WHERE id = ?4 AND lifecycle_state = 'processing'",
                        params![total, completed, now, document_id],
                    )
                    .map_err(|error| {
                        CommandError::database_at(
                            "document_ingestion_progress_failed",
                            format!(
                                "document={document_id} stage=render completed={completed} total={total}"
                            ),
                            error,
                        )
                    })?
            }
            IngestionProgress::Ocr {
                completed_units,
                total_units,
            } => {
                let total = to_i64(total_units);
                let completed = to_i64(completed_units).min(total);
                connection
                    .execute(
                        "UPDATE documents
                         SET extraction_mode = 'ocr_fallback', ocr_page_count = ?1,
                             ocr_completed_page_count = ?2, ocr_mean_confidence = NULL,
                             ocr_engine = 'tesseract-cli', updated_at = ?3
                         WHERE id = ?4 AND lifecycle_state = 'processing'",
                        params![total, completed, now, document_id],
                    )
                    .map_err(|error| {
                        CommandError::database_at(
                            "document_ocr_progress_failed",
                            format!(
                                "document={document_id} completed={completed} total={total}"
                            ),
                            error,
                        )
                    })?
            }
        };
        if changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "The document changed while ingestion progress was being recorded.",
            ));
        }
        Ok(())
    })
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
