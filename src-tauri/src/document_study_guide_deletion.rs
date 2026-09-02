//! Revision-checked deletion for generated study guides.
//!
//! Generated revisions and revision-owned state are deleted together while
//! document/topic notes are detached and preserved. The guide identity makes a
//! retry after an ambiguous response harmless: once the requested guide is gone
//! and no replacement is active, the command reports a replay rather than
//! repeating side effects.

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DeleteDocumentStudyGuideInput {
    pub document_id: String,
    pub guide_id: String,
    pub expected_revision: i64,
    pub operation_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteDocumentStudyGuideResult {
    pub document_id: String,
    pub guide_id: String,
    pub deleted_guide_revision: i64,
    pub removed_revision_count: i64,
    pub removed_override_count: i64,
    pub removed_checkpoint_count: i64,
    pub removed_export_count: i64,
    pub removed_job_count: i64,
    pub replayed: bool,
}

#[tauri::command]
pub(crate) fn delete_document_study_guide(
    database: State<'_, Database>,
    input: DeleteDocumentStudyGuideInput,
) -> Result<DeleteDocumentStudyGuideResult, CommandError> {
    delete_study_guide(database.inner(), input)
}

fn delete_study_guide(
    database: &Database,
    input: DeleteDocumentStudyGuideInput,
) -> Result<DeleteDocumentStudyGuideResult, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    validate_entity_id(&input.guide_id, "study guide")?;
    validate_entity_id(&input.operation_id, "study-guide deletion operation")?;
    if input.expected_revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The study-guide revision was invalid.",
        ));
    }

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let document_exists = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM documents WHERE id = ?1)",
                [&input.document_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(CommandError::database)?;
        if !document_exists {
            return Err(CommandError::new(
                "document_not_found",
                "That source document is no longer in the library.",
            ));
        }

        let active = transaction
            .query_row(
                "SELECT id, revision FROM document_study_guides
                 WHERE document_id = ?1 AND state = 'active'",
                [&input.document_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some((active_id, active_revision)) = active else {
            let any_remaining = transaction
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM document_study_guides WHERE document_id = ?1
                     )",
                    [&input.document_id],
                    |row| row.get::<_, i64>(0),
                )
                .map(|value| value != 0)
                .map_err(CommandError::database)?;
            if any_remaining {
                return Err(CommandError::new(
                    "document_study_guide_changed",
                    "The study-guide state changed. Refresh Study Guides and try again.",
                ));
            }
            return Ok(DeleteDocumentStudyGuideResult {
                document_id: input.document_id,
                guide_id: input.guide_id,
                deleted_guide_revision: input.expected_revision,
                removed_revision_count: 0,
                removed_override_count: 0,
                removed_checkpoint_count: 0,
                removed_export_count: 0,
                removed_job_count: 0,
                replayed: true,
            });
        };
        if active_id != input.guide_id || active_revision != input.expected_revision {
            return Err(CommandError::new(
                "document_study_guide_changed",
                "This study guide changed before it could be deleted. Reload it and try again.",
            ));
        }

        let busy = transaction
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM document_artifact_jobs
                     WHERE document_id = ?1
                       AND artifact_kind = 'study_guide'
                       AND state IN ('queued', 'preparing', 'generating', 'committing')
                 )",
                [&input.document_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(CommandError::database)?;
        if busy {
            return Err(CommandError::new(
                "document_study_guide_busy",
                "Stop the current study-guide generation before deleting this guide.",
            ));
        }

        let now = unix_time_millis()?;
        transaction
            .execute(
                "UPDATE document_study_guide_overlays
                 SET guide_id = NULL, revision = revision + 1, updated_at = ?1
                 WHERE document_id = ?2
                   AND guide_id IS NOT NULL
                   AND overlay_kind IN ('guide_note', 'topic_note')",
                params![now, input.document_id],
            )
            .map_err(CommandError::database)?;

        let removed_revision_count = count(
            &transaction,
            "SELECT COUNT(*) FROM document_study_guides WHERE document_id = ?1",
            &input.document_id,
        )?;
        let removed_override_count = count(
            &transaction,
            "SELECT COUNT(*) FROM document_study_guide_overlays
             WHERE document_id = ?1 AND guide_id IS NOT NULL",
            &input.document_id,
        )?;
        let removed_checkpoint_count = count(
            &transaction,
            "SELECT COUNT(*) FROM document_study_guide_checkpoints WHERE document_id = ?1",
            &input.document_id,
        )?;
        let removed_job_count = count(
            &transaction,
            "SELECT COUNT(*) FROM document_artifact_jobs
             WHERE document_id = ?1 AND artifact_kind = 'study_guide'",
            &input.document_id,
        )?;

        transaction
            .execute(
                "DELETE FROM document_study_guide_checkpoints WHERE document_id = ?1",
                [&input.document_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "DELETE FROM document_study_guides WHERE document_id = ?1",
                [&input.document_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "DELETE FROM document_artifact_jobs
                 WHERE document_id = ?1 AND artifact_kind = 'study_guide'",
                [&input.document_id],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)?;

        Ok(DeleteDocumentStudyGuideResult {
            document_id: input.document_id,
            guide_id: input.guide_id,
            deleted_guide_revision: input.expected_revision,
            removed_revision_count,
            removed_override_count,
            removed_checkpoint_count,
            removed_export_count: 0,
            removed_job_count,
            replayed: false,
        })
    })
}

fn count(
    connection: &rusqlite::Connection,
    sql: &str,
    document_id: &str,
) -> Result<i64, CommandError> {
    connection
        .query_row(sql, [document_id], |row| row.get(0))
        .map_err(CommandError::database)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_revision_is_rejected_before_database_work() {
        let database = Database::open_in_memory().expect("database should open");
        let error = delete_study_guide(
            &database,
            DeleteDocumentStudyGuideInput {
                document_id: "a".repeat(32),
                guide_id: "b".repeat(32),
                expected_revision: 0,
                operation_id: "c".repeat(32),
            },
        )
        .expect_err("zero revision must fail");
        assert_eq!(error.code, "invalid_revision");
    }
}
