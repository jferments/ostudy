//! Generic long-running learning-artifact jobs for documents.
//!
//! Source ingestion, outline construction, flashcard authoring, and study-guide
//! authoring are distinct lifecycle dimensions. This module gives guide/card
//! work an artifact-scoped identity instead of overloading the source document's
//! lifecycle or treating a generated deck as proof that a document is ready.

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const MAX_SAFE_ERROR_CODE: usize = 80;
const MAX_SAFE_ERROR_MESSAGE: usize = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentArtifactKind {
    StudyGuide,
    Flashcards,
}

impl DocumentArtifactKind {
    pub(crate) fn as_db(self) -> &'static str {
        match self {
            Self::StudyGuide => "study_guide",
            Self::Flashcards => "flashcards",
        }
    }

    fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "study_guide" => Ok(Self::StudyGuide),
            "flashcards" => Ok(Self::Flashcards),
            _ => Err(CommandError::new(
                "invalid_document_artifact_kind",
                "A persisted document artifact kind was invalid.",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentArtifactJobState {
    Queued,
    Preparing,
    Generating,
    Committing,
    Succeeded,
    Failed,
    Interrupted,
    Canceled,
}

impl DocumentArtifactJobState {
    fn as_db(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Preparing => "preparing",
            Self::Generating => "generating",
            Self::Committing => "committing",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Canceled => "canceled",
        }
    }

    fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "queued" => Ok(Self::Queued),
            "preparing" => Ok(Self::Preparing),
            "generating" => Ok(Self::Generating),
            "committing" => Ok(Self::Committing),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "interrupted" => Ok(Self::Interrupted),
            "canceled" => Ok(Self::Canceled),
            _ => Err(CommandError::new(
                "invalid_document_artifact_job_state",
                "A persisted document artifact job state was invalid.",
            )),
        }
    }

    fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Preparing | Self::Generating | Self::Committing
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DocumentArtifactProviderSnapshot {
    pub profile_id: Option<String>,
    pub provider_kind: String,
    pub model_id: String,
    pub profile_revision: i64,
    pub credential_epoch: i64,
    pub network_scope: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentArtifactJob {
    pub id: String,
    pub document_id: String,
    pub plan_id: Option<String>,
    pub artifact_kind: DocumentArtifactKind,
    pub state: DocumentArtifactJobState,
    pub total_units: usize,
    pub completed_units: usize,
    pub safe_error_code: String,
    pub safe_error_message: String,
    pub api_token_count: Option<i64>,
    pub api_token_usage_complete: bool,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
}

pub(crate) fn create_document_artifact_job(
    database: &Database,
    document_id: &str,
    plan_id: Option<&str>,
    artifact_kind: DocumentArtifactKind,
    provider: &DocumentArtifactProviderSnapshot,
    total_units: usize,
) -> Result<DocumentArtifactJob, CommandError> {
    validate_entity_id(document_id, "document")?;
    if let Some(plan_id) = plan_id {
        validate_entity_id(plan_id, "learning plan")?;
    }
    if provider.profile_revision < 1 || provider.credential_epoch < 0 {
        return Err(CommandError::new(
            "invalid_document_artifact_provider",
            "The selected AI profile snapshot was invalid.",
        ));
    }
    if !matches!(provider.network_scope.as_str(), "local" | "lan" | "cloud") {
        return Err(CommandError::new(
            "invalid_document_artifact_provider",
            "The selected AI profile has an invalid network scope.",
        ));
    }

    let now = unix_time_millis()?;
    let id = database.with_connection(|connection| generate_id(connection))?;
    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;

        let active: Option<(String, String)> = transaction
            .query_row(
                "SELECT id, artifact_kind FROM document_artifact_jobs
                 WHERE document_id = ?1
                   AND state IN ('queued', 'preparing', 'generating', 'committing')
                 ORDER BY created_at, id LIMIT 1",
                [document_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        if let Some((active_id, active_kind)) = active {
            return Err(CommandError::new(
                "document_artifact_job_already_running",
                format!(
                    "This document already has an active {active_kind} job ({active_id}). Stop or finish it before starting another learning artifact."
                ),
            ));
        }

        if let Some(plan_id) = plan_id {
            let plan_matches = transaction
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM document_learning_plans
                         WHERE id = ?1 AND document_id = ?2
                     )",
                    params![plan_id, document_id],
                    |row| row.get::<_, i64>(0),
                )
                .map(|value| value != 0)
                .map_err(CommandError::database)?;
            if !plan_matches {
                return Err(CommandError::new(
                    "document_learning_plan_not_found",
                    "The selected learning plan does not belong to this document.",
                ));
            }
        }

        transaction
            .execute(
                "INSERT INTO document_artifact_jobs (
                     id, document_id, plan_id, artifact_kind, profile_id,
                     provider_kind, model_id, profile_revision, credential_epoch,
                     network_scope, state, total_units, completed_units,
                     created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                           'queued', ?11, 0, ?12, ?12)",
                params![
                    id,
                    document_id,
                    plan_id,
                    artifact_kind.as_db(),
                    provider.profile_id,
                    provider.provider_kind,
                    provider.model_id,
                    provider.profile_revision,
                    provider.credential_epoch,
                    provider.network_scope,
                    to_i64(total_units),
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)
    })?;
    load_document_artifact_job(database, &id)
}

pub(crate) fn transition_document_artifact_job(
    database: &Database,
    job_id: &str,
    expected: DocumentArtifactJobState,
    next: DocumentArtifactJobState,
    completed_units: usize,
) -> Result<DocumentArtifactJob, CommandError> {
    validate_entity_id(job_id, "artifact job")?;
    if !valid_transition(expected, next) {
        return Err(CommandError::new(
            "invalid_document_artifact_job_transition",
            "That document artifact job transition is not allowed.",
        ));
    }
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE document_artifact_jobs
                 SET state = ?1,
                     completed_units = ?2,
                     started_at = CASE
                         WHEN started_at IS NULL AND ?1 IN ('preparing', 'generating', 'committing')
                         THEN ?3 ELSE started_at END,
                     completed_at = CASE
                         WHEN ?1 IN ('succeeded', 'failed', 'interrupted', 'canceled')
                         THEN ?3 ELSE NULL END,
                     updated_at = ?3
                 WHERE id = ?4 AND state = ?5 AND ?2 <= total_units",
                params![
                    next.as_db(),
                    to_i64(completed_units),
                    now,
                    job_id,
                    expected.as_db(),
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_artifact_job_changed",
                "The document artifact job changed before progress could be committed.",
            ));
        }
        Ok(())
    })?;
    load_document_artifact_job(database, job_id)
}

pub(crate) fn complete_document_artifact_job(
    database: &Database,
    job_id: &str,
    api_token_count: Option<i64>,
    api_token_usage_complete: bool,
) -> Result<DocumentArtifactJob, CommandError> {
    validate_entity_id(job_id, "artifact job")?;
    if api_token_count.is_some_and(|tokens| tokens < 0) {
        return Err(CommandError::new(
            "invalid_document_artifact_usage",
            "Document artifact token usage cannot be negative.",
        ));
    }
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE document_artifact_jobs
                 SET state = 'succeeded', completed_units = total_units,
                     api_token_count = ?1, api_token_usage_complete = ?2,
                     safe_error_code = '', safe_error_message = '',
                     updated_at = ?3, completed_at = ?3
                 WHERE id = ?4 AND state = 'committing'",
                params![
                    api_token_count,
                    if api_token_usage_complete { 1_i64 } else { 0_i64 },
                    now,
                    job_id,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_artifact_job_changed",
                "The document artifact job changed before completion could be committed.",
            ));
        }
        Ok(())
    })?;
    load_document_artifact_job(database, job_id)
}

pub(crate) fn fail_document_artifact_job(
    database: &Database,
    job_id: &str,
    error: &CommandError,
    api_token_count: Option<i64>,
    api_token_usage_complete: bool,
) -> Result<DocumentArtifactJob, CommandError> {
    validate_entity_id(job_id, "artifact job")?;
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE document_artifact_jobs
                 SET state = 'failed', safe_error_code = ?1,
                     safe_error_message = ?2, api_token_count = ?3,
                     api_token_usage_complete = ?4, updated_at = ?5,
                     completed_at = ?5
                 WHERE id = ?6
                   AND state IN ('queued', 'preparing', 'generating', 'committing')",
                params![
                    bounded(&error.code, MAX_SAFE_ERROR_CODE),
                    bounded(&error.message, MAX_SAFE_ERROR_MESSAGE),
                    api_token_count,
                    if api_token_usage_complete { 1_i64 } else { 0_i64 },
                    now,
                    job_id,
                ],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "document_artifact_job_changed",
                "The document artifact job was no longer active when failure was recorded.",
            ));
        }
        Ok(())
    })?;
    load_document_artifact_job(database, job_id)
}

pub(crate) fn reconcile_interrupted_document_artifact_jobs(
    database: &Database,
) -> Result<usize, CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        connection
            .execute(
                "UPDATE document_artifact_jobs
                 SET state = 'interrupted',
                     safe_error_code = 'document_artifact_interrupted',
                     safe_error_message = 'Document learning-artifact generation stopped when ostudy closed. Retry to continue from validated checkpoints.',
                     updated_at = ?1, completed_at = ?1
                 WHERE state IN ('queued', 'preparing', 'generating', 'committing')",
                [now],
            )
            .map_err(CommandError::database)
    })
}

pub(crate) fn load_document_artifact_job(
    database: &Database,
    job_id: &str,
) -> Result<DocumentArtifactJob, CommandError> {
    validate_entity_id(job_id, "artifact job")?;
    database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT document_id, plan_id, artifact_kind, state,
                        total_units, completed_units, safe_error_code,
                        safe_error_message, api_token_count,
                        api_token_usage_complete, created_at, started_at,
                        updated_at, completed_at
                 FROM document_artifact_jobs WHERE id = ?1",
                [job_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, Option<i64>>(11)?,
                        row.get::<_, i64>(12)?,
                        row.get::<_, Option<i64>>(13)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_artifact_job_not_found",
                    "That document artifact job is no longer available.",
                )
            })
            .and_then(|row| {
                Ok(DocumentArtifactJob {
                    id: job_id.to_string(),
                    document_id: row.0,
                    plan_id: row.1,
                    artifact_kind: DocumentArtifactKind::from_db(&row.2)?,
                    state: DocumentArtifactJobState::from_db(&row.3)?,
                    total_units: to_usize(row.4)?,
                    completed_units: to_usize(row.5)?,
                    safe_error_code: row.6,
                    safe_error_message: row.7,
                    api_token_count: row.8,
                    api_token_usage_complete: row.9 != 0,
                    created_at: row.10,
                    started_at: row.11,
                    updated_at: row.12,
                    completed_at: row.13,
                })
            })
    })
}

fn valid_transition(
    expected: DocumentArtifactJobState,
    next: DocumentArtifactJobState,
) -> bool {
    matches!(
        (expected, next),
        (DocumentArtifactJobState::Queued, DocumentArtifactJobState::Preparing)
            | (
                DocumentArtifactJobState::Preparing,
                DocumentArtifactJobState::Generating
            )
            | (
                DocumentArtifactJobState::Generating,
                DocumentArtifactJobState::Generating
            )
            | (
                DocumentArtifactJobState::Generating,
                DocumentArtifactJobState::Committing
            )
            | (
                DocumentArtifactJobState::Queued
                    | DocumentArtifactJobState::Preparing
                    | DocumentArtifactJobState::Generating
                    | DocumentArtifactJobState::Committing,
                DocumentArtifactJobState::Canceled
                    | DocumentArtifactJobState::Interrupted
                    | DocumentArtifactJobState::Failed
            )
    ) && expected.is_active()
}

fn bounded(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn to_usize(value: i64) -> Result<usize, CommandError> {
    usize::try_from(value).map_err(|_| {
        CommandError::new(
            "invalid_document_artifact_job",
            "A persisted document artifact job count was invalid.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_job_state_machine_rejects_skipping_generation() {
        assert!(!valid_transition(
            DocumentArtifactJobState::Queued,
            DocumentArtifactJobState::Committing
        ));
        assert!(valid_transition(
            DocumentArtifactJobState::Queued,
            DocumentArtifactJobState::Preparing
        ));
        assert!(valid_transition(
            DocumentArtifactJobState::Generating,
            DocumentArtifactJobState::Committing
        ));
    }

    #[test]
    fn terminal_states_are_not_active() {
        for state in [
            DocumentArtifactJobState::Succeeded,
            DocumentArtifactJobState::Failed,
            DocumentArtifactJobState::Interrupted,
            DocumentArtifactJobState::Canceled,
        ] {
            assert!(!state.is_active());
        }
    }
}
