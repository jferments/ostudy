//! Durable StudyTarget ownership for document and collection study bundles.
//!
//! This layer owns target identity, exact source snapshots, retry receipts,
//! sibling artifact state, restart reconciliation, and explicit deletion
//! suppression. It deliberately does not author flashcards or replace the
//! existing document-generation pipeline. Collection composition must consume
//! already-admitted document artifacts so collection synthesis cannot lower the
//! quality bar established by individual-document generation.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::State;

pub(crate) mod composition;

use crate::{
    collections::{self, SelectedDocumentInput, SourceSelectionInput, SourceSetSnapshotDto},
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StudyTargetKind {
    Document,
    Collection,
}

impl StudyTargetKind {
    fn as_db(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Collection => "collection",
        }
    }

    fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "document" => Ok(Self::Document),
            "collection" => Ok(Self::Collection),
            _ => Err(invalid("A saved study target has an invalid kind.")),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StudyTargetArtifactKind {
    Flashcards,
    StudyGuide,
}

impl StudyTargetArtifactKind {
    fn as_db(self) -> &'static str {
        match self {
            Self::Flashcards => "flashcards",
            Self::StudyGuide => "study_guide",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyTargetRef {
    pub kind: StudyTargetKind,
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyTargetOutputPolicy {
    #[serde(default = "default_true")]
    pub flashcards: bool,
    #[serde(default = "default_true")]
    pub study_guide: bool,
}

impl Default for StudyTargetOutputPolicy {
    fn default() -> Self {
        Self {
            flashcards: true,
            study_guide: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BeginStudyTargetBundleInput {
    pub operation_id: String,
    pub target: StudyTargetRef,
    #[serde(default)]
    pub output_policy: StudyTargetOutputPolicy,
    #[serde(default)]
    pub recreate_artifacts: Vec<StudyTargetArtifactKind>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetStudyTargetBundleInput {
    pub target: StudyTargetRef,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudyTargetArtifactStatus {
    pub kind: StudyTargetArtifactKind,
    pub state: String,
    pub artifact_id: Option<String>,
    pub total_units: usize,
    pub completed_units: usize,
    pub safe_error_code: String,
    pub safe_error_message: String,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudyTargetBundleStatus {
    pub target_id: String,
    pub target: StudyTargetRef,
    pub title: String,
    pub revision: i64,
    pub source_snapshot_id: String,
    pub deck_id: Option<String>,
    pub guide_id: Option<String>,
    pub guide_revision: Option<i64>,
    pub latest_operation_id: Option<String>,
    pub latest_batch_state: Option<String>,
    pub flashcards: StudyTargetArtifactStatus,
    pub study_guide: StudyTargetArtifactStatus,
}

#[derive(Clone)]
struct StoredTarget {
    id: String,
    target: StudyTargetRef,
    title: String,
    revision: i64,
    deck_id: Option<String>,
    snapshot_id: String,
}

#[tauri::command]
pub(crate) fn get_study_target_bundle(
    database: State<'_, Database>,
    input: GetStudyTargetBundleInput,
) -> Result<Option<StudyTargetBundleStatus>, CommandError> {
    validate_target_ref(&input.target)?;
    load_bundle_status(database.inner(), &input.target)
}

/// Capture a real document/collection target and its exact source snapshot.
///
/// This boundary is intentionally provider-free. A later composition step must
/// explicitly consume the receipt. Merely persisting it never authorizes cloud
/// work and never changes source-document flashcard/guide generation behavior.
#[tauri::command]
pub(crate) fn begin_study_target_bundle(
    database: State<'_, Database>,
    input: BeginStudyTargetBundleInput,
) -> Result<StudyTargetBundleStatus, CommandError> {
    begin_bundle(database.inner(), input)
}

pub(crate) fn reconcile_startup(database: &Database) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        tx.execute(
            "UPDATE study_target_artifact_jobs
             SET state='interrupted',
                 safe_error_code='study_target_interrupted',
                 safe_error_message='Study-material work was interrupted. Saved sibling artifacts remain available; retry explicitly.',
                 updated_at=?1,completed_at=?1
             WHERE state IN ('pending','running','committing')",
            [now],
        )
        .map_err(CommandError::database)?;
        tx.execute(
            "UPDATE study_target_batches
             SET state='interrupted',
                 safe_error_code='study_target_interrupted',
                 safe_error_message='Study-material work was interrupted. No provider request will be restarted automatically.',
                 updated_at=?1,completed_at=?1
             WHERE state IN ('queued','preparing','composing','committing')",
            [now],
        )
        .map_err(CommandError::database)?;
        tx.commit().map_err(CommandError::database)
    })
}

/// Work/retry receipts are device-local. Target ownership and terminal artifact
/// links transfer only through their selected Documents/Flashcards/Study Guides
/// graph.
pub(crate) fn sanitize_for_device_transfer(connection: &Connection) -> Result<(), CommandError> {
    connection
        .execute_batch(
            "DELETE FROM study_target_artifact_jobs;
             DELETE FROM study_target_batches;",
        )
        .map_err(CommandError::database)
}

/// Called by ordinary deck deletion in the same transaction as the tombstone.
pub(crate) fn record_deck_deletion_suppression(
    tx: &Transaction<'_>,
    deck_id: &str,
    operation_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    tx.execute(
        "INSERT INTO study_target_suppressions(
             target_id,artifact_kind,artifact_id,operation_id,reason,created_at
         )
         SELECT id,'flashcards',?1,?2,'user_deleted',?3
         FROM study_targets WHERE deck_id=?1
         ON CONFLICT(target_id,artifact_kind) DO UPDATE SET
             artifact_id=excluded.artifact_id,
             operation_id=excluded.operation_id,
             reason=excluded.reason,
             created_at=excluded.created_at",
        params![deck_id, operation_id, now],
    )
    .map_err(CommandError::database)?;
    tx.execute(
        "UPDATE study_target_artifact_jobs
         SET state='suppressed',artifact_id=?1,updated_at=?2,completed_at=?2
         WHERE artifact_kind='flashcards'
           AND state IN ('pending','running','committing')
           AND batch_id IN (
               SELECT operation_id FROM study_target_batches
               WHERE target_id IN (SELECT id FROM study_targets WHERE deck_id=?1)
           )",
        params![deck_id, now],
    )
    .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn clear_deck_deletion_suppression(
    tx: &Transaction<'_>,
    deck_id: &str,
    operation_id: &str,
) -> Result<(), CommandError> {
    tx.execute(
        "DELETE FROM study_target_suppressions
         WHERE artifact_kind='flashcards' AND artifact_id=?1 AND operation_id=?2",
        params![deck_id, operation_id],
    )
    .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn record_document_guide_deletion_suppression(
    tx: &Transaction<'_>,
    document_id: &str,
    guide_id: &str,
    operation_id: &str,
    now: i64,
) -> Result<(), CommandError> {
    tx.execute(
        "INSERT INTO study_target_suppressions(
             target_id,artifact_kind,artifact_id,operation_id,reason,created_at
         )
         SELECT id,'study_guide',?2,?3,'user_deleted',?4
         FROM study_targets
         WHERE target_kind='document' AND document_id=?1
         ON CONFLICT(target_id,artifact_kind) DO UPDATE SET
             artifact_id=excluded.artifact_id,
             operation_id=excluded.operation_id,
             reason=excluded.reason,
             created_at=excluded.created_at",
        params![document_id, guide_id, operation_id, now],
    )
    .map_err(CommandError::database)?;
    Ok(())
}

fn begin_bundle(
    database: &Database,
    mut input: BeginStudyTargetBundleInput,
) -> Result<StudyTargetBundleStatus, CommandError> {
    validate_entity_id(&input.operation_id, "study target operation")?;
    validate_target_ref(&input.target)?;
    input.recreate_artifacts.sort();
    input.recreate_artifacts.dedup();

    if !input.output_policy.flashcards && !input.output_policy.study_guide {
        return Err(invalid("Request at least one study-material output."));
    }
    if input.recreate_artifacts.iter().any(|kind| {
        (*kind == StudyTargetArtifactKind::Flashcards && !input.output_policy.flashcards)
            || (*kind == StudyTargetArtifactKind::StudyGuide && !input.output_policy.study_guide)
    }) {
        return Err(invalid(
            "A suppressed artifact can only be recreated when that output is requested.",
        ));
    }

    let snapshot = database
        .with_connection(|connection| resolve_target_snapshot(connection, &input.target))?;
    let snapshot_json = serde_json::to_string(&snapshot).map_err(CommandError::serialization)?;
    let policy_json =
        serde_json::to_string(&input.output_policy).map_err(CommandError::serialization)?;
    let request_hash = hash_bytes(
        &serde_json::to_vec(&json!({
            "target": &input.target,
            "snapshotId": &snapshot.snapshot_id,
            "outputPolicy": &input.output_policy,
            "recreateArtifacts": &input.recreate_artifacts,
        }))
        .map_err(CommandError::serialization)?,
    );

    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        collections::ensure_snapshot_current(&tx, &snapshot)?;
        let target = ensure_target(&tx, &input.target, &policy_json)?;

        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT target_id,request_sha256 FROM study_target_batches
                 WHERE operation_id=?1",
                [input.operation_id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        if let Some((target_id, stored_hash)) = existing {
            if target_id != target.id || stored_hash != request_hash {
                return Err(CommandError::new(
                    "operation_id_conflict",
                    "That study-material operation identifier was already used for a different request.",
                ));
            }
            tx.commit().map_err(CommandError::database)?;
            return Ok(());
        }

        let now = unix_time_millis()?;
        for artifact in &input.recreate_artifacts {
            tx.execute(
                "DELETE FROM study_target_suppressions
                 WHERE target_id=?1 AND artifact_kind=?2",
                params![target.id, artifact.as_db()],
            )
            .map_err(CommandError::database)?;
        }

        tx.execute(
            "INSERT INTO study_target_batches(
                 operation_id,target_id,request_sha256,source_snapshot_id,source_snapshot_json,
                 output_policy_json,provider_authorized,state,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,?5,?6,0,'queued',?7,?7)",
            params![
                input.operation_id,
                target.id,
                request_hash,
                snapshot.snapshot_id,
                snapshot_json,
                policy_json,
                now
            ],
        )
        .map_err(CommandError::database)?;

        for (requested, kind) in [
            (
                input.output_policy.flashcards,
                StudyTargetArtifactKind::Flashcards,
            ),
            (
                input.output_policy.study_guide,
                StudyTargetArtifactKind::StudyGuide,
            ),
        ] {
            if !requested {
                continue;
            }
            let suppressed_artifact: Option<Option<String>> = tx
                .query_row(
                    "SELECT artifact_id FROM study_target_suppressions
                     WHERE target_id=?1 AND artifact_kind=?2",
                    params![target.id, kind.as_db()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(CommandError::database)?;
            let state = if suppressed_artifact.is_some() {
                "suppressed"
            } else {
                "pending"
            };
            tx.execute(
                "INSERT INTO study_target_artifact_jobs(
                     batch_id,artifact_kind,state,artifact_id,total_units,completed_units,
                     created_at,updated_at,completed_at
                 ) VALUES(?1,?2,?3,?4,0,0,?5,?5,?6)",
                params![
                    input.operation_id,
                    kind.as_db(),
                    state,
                    suppressed_artifact.flatten(),
                    now,
                    if state == "suppressed" {
                        Some(now)
                    } else {
                        None
                    }
                ],
            )
            .map_err(CommandError::database)?;
        }

        tx.execute(
            "UPDATE study_targets
             SET active_snapshot_id=?1,active_snapshot_json=?2,output_policy_json=?3,
                 updated_at=?4,revision=revision+1
             WHERE id=?5",
            params![
                snapshot.snapshot_id,
                snapshot_json,
                policy_json,
                now,
                target.id
            ],
        )
        .map_err(CommandError::database)?;
        tx.commit().map_err(CommandError::database)
    })?;

    load_bundle_status(database, &input.target)?.ok_or_else(target_not_found)
}

fn ensure_target(
    tx: &Transaction<'_>,
    target: &StudyTargetRef,
    policy_json: &str,
) -> Result<StoredTarget, CommandError> {
    if let Some(stored) = load_target(tx, target)? {
        return Ok(stored);
    }
    let title = target_title(tx, target)?;
    let id = generate_id(tx)?;
    let now = unix_time_millis()?;
    let (document_id, collection_id) = match target.kind {
        StudyTargetKind::Document => (Some(target.id.as_str()), None),
        StudyTargetKind::Collection => (None, Some(target.id.as_str())),
    };
    tx.execute(
        "INSERT INTO study_targets(
             id,target_kind,document_id,collection_id,output_policy_json,
             active_snapshot_id,active_snapshot_json,deck_id,revision,created_at,updated_at
         ) VALUES(?1,?2,?3,?4,?5,'','{}',NULL,1,?6,?6)",
        params![
            id,
            target.kind.as_db(),
            document_id,
            collection_id,
            policy_json,
            now
        ],
    )
    .map_err(CommandError::database)?;
    Ok(StoredTarget {
        id,
        target: target.clone(),
        title,
        revision: 1,
        deck_id: None,
        snapshot_id: String::new(),
    })
}

fn resolve_target_snapshot(
    connection: &Connection,
    target: &StudyTargetRef,
) -> Result<SourceSetSnapshotDto, CommandError> {
    match target.kind {
        StudyTargetKind::Document => collections::resolve_selection(
            connection,
            SourceSelectionInput {
                collection_ids: Vec::new(),
                documents: vec![SelectedDocumentInput {
                    document_id: target.id.clone(),
                    ranges: Vec::new(),
                }],
                exclusions: Vec::new(),
            },
        ),
        StudyTargetKind::Collection => collections::resolve_selection(
            connection,
            SourceSelectionInput {
                collection_ids: vec![target.id.clone()],
                documents: Vec::new(),
                exclusions: Vec::new(),
            },
        ),
    }
}

fn load_bundle_status(
    database: &Database,
    target: &StudyTargetRef,
) -> Result<Option<StudyTargetBundleStatus>, CommandError> {
    database.with_connection(|connection| {
        let Some(stored) = load_target(connection, target)? else {
            return Ok(None);
        };
        let latest: Option<(String, String)> = connection
            .query_row(
                "SELECT operation_id,state FROM study_target_batches
                 WHERE target_id=?1
                 ORDER BY created_at DESC,operation_id DESC LIMIT 1",
                [stored.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;

        let flashcards = load_artifact_status(
            connection,
            &stored,
            latest.as_ref().map(|value| value.0.as_str()),
            StudyTargetArtifactKind::Flashcards,
        )?;
        let study_guide = load_artifact_status(
            connection,
            &stored,
            latest.as_ref().map(|value| value.0.as_str()),
            StudyTargetArtifactKind::StudyGuide,
        )?;

        let collection_guide: Option<(String, i64)> = connection
            .query_row(
                "SELECT id,revision FROM study_target_guides
                 WHERE target_id=?1 AND state='active'",
                [stored.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        let document_guide = if stored.target.kind == StudyTargetKind::Document {
            connection
                .query_row(
                    "SELECT id,revision FROM document_study_guides
                     WHERE document_id=?1 AND state='active'",
                    [stored.target.id.as_str()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()
                .map_err(CommandError::database)?
        } else {
            None
        };

        Ok(Some(StudyTargetBundleStatus {
            target_id: stored.id,
            target: stored.target,
            title: stored.title,
            revision: stored.revision,
            source_snapshot_id: stored.snapshot_id,
            deck_id: active_deck_id(connection, stored.deck_id.as_deref())?,
            guide_id: collection_guide
                .as_ref()
                .map(|value| value.0.clone())
                .or_else(|| document_guide.as_ref().map(|value| value.0.clone())),
            guide_revision: collection_guide
                .as_ref()
                .map(|value| value.1)
                .or_else(|| document_guide.as_ref().map(|value| value.1)),
            latest_operation_id: latest.as_ref().map(|value| value.0.clone()),
            latest_batch_state: latest.map(|value| value.1),
            flashcards,
            study_guide,
        }))
    })
}

fn load_artifact_status(
    connection: &Connection,
    target: &StoredTarget,
    batch_id: Option<&str>,
    kind: StudyTargetArtifactKind,
) -> Result<StudyTargetArtifactStatus, CommandError> {
    if let Some(batch_id) = batch_id {
        if let Some((state, artifact_id, total, completed, code, message)) = connection
            .query_row(
                "SELECT state,artifact_id,total_units,completed_units,
                        safe_error_code,safe_error_message
                 FROM study_target_artifact_jobs
                 WHERE batch_id=?1 AND artifact_kind=?2",
                params![batch_id, kind.as_db()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
        {
            return Ok(StudyTargetArtifactStatus {
                kind,
                state,
                artifact_id,
                total_units: usize::try_from(total).unwrap_or(usize::MAX),
                completed_units: usize::try_from(completed).unwrap_or(usize::MAX),
                safe_error_code: code,
                safe_error_message: message,
            });
        }
    }

    if let Some(artifact_id) = suppression_artifact(connection, &target.id, kind)? {
        return Ok(StudyTargetArtifactStatus {
            kind,
            state: "suppressed".to_string(),
            artifact_id,
            total_units: 0,
            completed_units: 0,
            safe_error_code: String::new(),
            safe_error_message: String::new(),
        });
    }

    let artifact_id = match kind {
        StudyTargetArtifactKind::Flashcards => {
            active_deck_id(connection, target.deck_id.as_deref())?
        }
        StudyTargetArtifactKind::StudyGuide
            if target.target.kind == StudyTargetKind::Collection =>
        {
            connection
                .query_row(
                    "SELECT id FROM study_target_guides
                     WHERE target_id=?1 AND state='active'",
                    [target.id.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(CommandError::database)?
        }
        StudyTargetArtifactKind::StudyGuide => connection
            .query_row(
                "SELECT id FROM document_study_guides
                 WHERE document_id=?1 AND state='active'",
                [target.target.id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?,
    };

    Ok(StudyTargetArtifactStatus {
        kind,
        state: if artifact_id.is_some() {
            "succeeded"
        } else {
            "not_started"
        }
        .to_string(),
        artifact_id,
        total_units: 0,
        completed_units: 0,
        safe_error_code: String::new(),
        safe_error_message: String::new(),
    })
}

fn load_target(
    connection: &Connection,
    target: &StudyTargetRef,
) -> Result<Option<StoredTarget>, CommandError> {
    let (predicate, id) = match target.kind {
        StudyTargetKind::Document => (
            "target_kind='document' AND document_id=?1",
            target.id.as_str(),
        ),
        StudyTargetKind::Collection => (
            "target_kind='collection' AND collection_id=?1",
            target.id.as_str(),
        ),
    };
    let row = connection
        .query_row(
            &format!(
                "SELECT id,target_kind,document_id,collection_id,revision,deck_id,
                        active_snapshot_id
                 FROM study_targets WHERE {predicate}"
            ),
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((id, kind, document_id, collection_id, revision, deck_id, snapshot_id)) = row else {
        return Ok(None);
    };
    let kind = StudyTargetKind::from_db(&kind)?;
    let source_id = match kind {
        StudyTargetKind::Document => {
            document_id.ok_or_else(|| invalid("A document target lost its source identity."))?
        }
        StudyTargetKind::Collection => {
            collection_id.ok_or_else(|| invalid("A collection target lost its source identity."))?
        }
    };
    let stored_ref = StudyTargetRef {
        kind,
        id: source_id,
    };
    Ok(Some(StoredTarget {
        title: target_title(connection, &stored_ref)?,
        id,
        target: stored_ref,
        revision,
        deck_id,
        snapshot_id,
    }))
}

fn target_title(connection: &Connection, target: &StudyTargetRef) -> Result<String, CommandError> {
    let title = match target.kind {
        StudyTargetKind::Document => connection
            .query_row(
                "SELECT title FROM documents WHERE id=?1",
                [target.id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?,
        StudyTargetKind::Collection => connection
            .query_row(
                "SELECT name FROM document_collections WHERE id=?1",
                [target.id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?,
    };
    title.ok_or_else(target_not_found)
}

fn active_deck_id(
    connection: &Connection,
    deck_id: Option<&str>,
) -> Result<Option<String>, CommandError> {
    let Some(deck_id) = deck_id else {
        return Ok(None);
    };
    connection
        .query_row(
            "SELECT id FROM decks
             WHERE id=?1 AND deleted_at IS NULL",
            [deck_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)
}

fn suppression_artifact(
    connection: &Connection,
    target_id: &str,
    kind: StudyTargetArtifactKind,
) -> Result<Option<Option<String>>, CommandError> {
    connection
        .query_row(
            "SELECT artifact_id FROM study_target_suppressions
             WHERE target_id=?1 AND artifact_kind=?2",
            params![target_id, kind.as_db()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(CommandError::database)
}

fn validate_target_ref(target: &StudyTargetRef) -> Result<(), CommandError> {
    validate_entity_id(&target.id, "study target source")
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn default_true() -> bool {
    true
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_study_target", message)
}

fn target_not_found() -> CommandError {
    CommandError::new(
        "study_target_not_found",
        "That study target is no longer available.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_policy_defaults_to_both_artifacts() {
        let policy: StudyTargetOutputPolicy = serde_json::from_value(json!({})).unwrap();
        assert!(policy.flashcards);
        assert!(policy.study_guide);
    }

    #[test]
    fn restart_interrupts_jobs_without_granting_provider_authorization() {
        let database = Database::open_in_memory().unwrap();
        database
            .with_connection(|connection| {
                let document_id = "d".repeat(32);
                connection
                    .execute(
                        "INSERT INTO documents(
                         id,title,original_filename,mime_type,byte_size,sha256,
                         source_relative_path,lifecycle_state,created_at,updated_at,
                         revision,source_format,source_unit_kind
                     ) VALUES(
                         ?1,'Doc','doc.pdf','application/pdf',1,?2,
                         'documents/doc.pdf','ready',1,1,1,'pdf','page'
                     )",
                        params![document_id, "e".repeat(64)],
                    )
                    .map_err(CommandError::database)?;

                let target_id = "t".repeat(32);
                connection
                    .execute(
                        r#"INSERT INTO study_targets(
                         id,target_kind,document_id,output_policy_json,created_at,updated_at
                     ) VALUES(
                         ?1,'document',?2,
                         '{"flashcards":true,"studyGuide":true}',1,1
                     )"#,
                        params![target_id, document_id],
                    )
                    .map_err(CommandError::database)?;

                let operation_id = "o".repeat(32);
                connection
                    .execute(
                        "INSERT INTO study_target_batches(
                         operation_id,target_id,request_sha256,source_snapshot_id,
                         source_snapshot_json,output_policy_json,provider_authorized,
                         state,created_at,updated_at
                     ) VALUES(?1,?2,?3,?4,'{}','{}',1,'composing',1,1)",
                        params![operation_id, target_id, "a".repeat(64), "b".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                connection
                    .execute(
                        "INSERT INTO study_target_artifact_jobs(
                         batch_id,artifact_kind,state,created_at,updated_at
                     ) VALUES(?1,'flashcards','running',1,1)",
                        [operation_id],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();

        reconcile_startup(&database).unwrap();

        database
            .with_connection(|connection| {
                let values: (String, i64, String) = connection
                    .query_row(
                        "SELECT b.state,b.provider_authorized,j.state
                         FROM study_target_batches b
                         JOIN study_target_artifact_jobs j
                           ON j.batch_id=b.operation_id",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(values, ("interrupted".into(), 1, "interrupted".into()));
                Ok(())
            })
            .unwrap();
    }
}
