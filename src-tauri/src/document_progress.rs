//! Process-local live progress for active document generation tasks.
//!
//! Durable checkpoints remain in SQLite. This command exposes only the current
//! process's high-frequency progress snapshot so the Documents UI can show the
//! active phase, source-window/page range, elapsed runtime, provider requests,
//! retries, and provider-reported token/cache/reasoning usage without writing a
//! database row for every streamed provider event.

use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    task_cancellation::{self, DocumentTaskProgress},
    util::validate_entity_id,
};

#[tauri::command]
pub fn get_document_generation_progress(
    database: State<'_, Database>,
    document_id: String,
) -> Result<Option<DocumentTaskProgress>, CommandError> {
    validate_entity_id(&document_id, "document")?;
    let exists = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM documents WHERE id = ?1)",
                [document_id.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(CommandError::database)
    })?;
    if !exists {
        return Err(CommandError::new(
            "document_not_found",
            "That document is no longer in the library.",
        ));
    }
    Ok(task_cancellation::document_task_progress(&document_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_document_has_no_process_local_progress() {
        let database = Database::open_in_memory().expect("database should open");
        let document_id = "0123456789abcdef0123456789abcdef";
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                            id, title, original_filename, source_relative_path,
                            byte_size, sha256, lifecycle_state, created_at,
                            updated_at, revision
                         ) VALUES (?1, 'Progress test', 'test.pdf',
                                   'documents/test/source.pdf', 1, ?2, 'failed', 1, 1, 1)",
                        [document_id, &"a".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("document should seed");
        assert!(task_cancellation::document_task_progress(document_id).is_none());
    }
}
