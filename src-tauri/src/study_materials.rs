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
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::State;

pub(crate) mod composition;
mod reuse;

const MAX_COLLECTION_OVERLAY_DOC_BYTES: usize = 2 * 1024 * 1024;
const MAX_COLLECTION_OVERLAY_TEXT_CHARS: usize = 500_000;

use crate::{
    collections::{self, SelectedDocumentInput, SourceSelectionInput, SourceSetSnapshotDto},
    database::Database,
    document_study_guides::{StudyGuideOverview, StudyGuideSectionContent},
    error::CommandError,
    knowledge::collection_view::CollectionSynthesis,
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
    pub source_changed: bool,
    pub deck_id: Option<String>,
    pub guide_id: Option<String>,
    pub guide_revision: Option<i64>,
    pub latest_operation_id: Option<String>,
    pub latest_batch_state: Option<String>,
    pub flashcards: StudyTargetArtifactStatus,
    pub study_guide: StudyTargetArtifactStatus,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GetCollectionStudyGuideInput {
    pub collection_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionStudyGuide {
    pub id: String,
    pub target_id: String,
    pub collection_id: String,
    pub title: String,
    pub revision: i64,
    pub source_snapshot_id: String,
    pub content: composition::CollectionGuideContent,
    pub overlays: Vec<CollectionStudyGuideOverlay>,
    pub created_at: i64,
    pub updated_at: i64,
    pub published_at: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CollectionStudyGuideOverlayKind {
    GuideNote,
    TopicNote,
    TitleOverride,
    BodyOverride,
    Highlight,
}

impl CollectionStudyGuideOverlayKind {
    fn as_db(self) -> &'static str {
        match self {
            Self::GuideNote => "guide_note",
            Self::TopicNote => "topic_note",
            Self::TitleOverride => "title_override",
            Self::BodyOverride => "body_override",
            Self::Highlight => "highlight",
        }
    }

    fn content_bound(self) -> bool {
        matches!(
            self,
            Self::TitleOverride | Self::BodyOverride | Self::Highlight
        )
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SaveCollectionStudyGuideOverlayInput {
    pub overlay_id: Option<String>,
    pub collection_id: String,
    pub guide_id: String,
    pub block_key: Option<String>,
    pub overlay_kind: CollectionStudyGuideOverlayKind,
    pub base_content_hash: Option<String>,
    pub rich_doc: Value,
    pub plain_text: String,
    pub expected_revision: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DiscardCollectionStudyGuideOverlayInput {
    pub overlay_id: String,
    pub collection_id: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CollectionStudyGuideOverlay {
    pub id: String,
    pub guide_id: String,
    pub target_id: String,
    pub block_key: Option<String>,
    pub overlay_kind: String,
    pub base_content_hash: Option<String>,
    pub rich_doc: Value,
    pub plain_text: String,
    pub state: String,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
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
pub(crate) fn save_collection_study_guide_overlay(
    database: State<'_, Database>,
    input: SaveCollectionStudyGuideOverlayInput,
) -> Result<CollectionStudyGuideOverlay, CommandError> {
    save_collection_overlay(database.inner(), input)
}

#[tauri::command]
pub(crate) fn discard_collection_study_guide_overlay(
    database: State<'_, Database>,
    input: DiscardCollectionStudyGuideOverlayInput,
) -> Result<(), CommandError> {
    discard_collection_overlay(database.inner(), input)
}

#[tauri::command]
pub(crate) fn get_collection_study_guide(
    database: State<'_, Database>,
    input: GetCollectionStudyGuideInput,
) -> Result<Option<CollectionStudyGuide>, CommandError> {
    validate_entity_id(&input.collection_id, "document collection")?;
    load_collection_study_guide(database.inner(), &input.collection_id)
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

fn save_collection_overlay(
    database: &Database,
    input: SaveCollectionStudyGuideOverlayInput,
) -> Result<CollectionStudyGuideOverlay, CommandError> {
    validate_entity_id(&input.collection_id, "document collection")?;
    validate_entity_id(&input.guide_id, "collection study guide")?;
    if let Some(overlay_id) = input.overlay_id.as_deref() {
        validate_entity_id(overlay_id, "collection study-guide overlay")?;
    }
    if input.expected_revision.is_some_and(|revision| revision < 1) {
        return Err(invalid(
            "The collection study-guide overlay revision was invalid.",
        ));
    }
    match input.overlay_kind {
        CollectionStudyGuideOverlayKind::GuideNote if input.block_key.is_some() => {
            return Err(invalid(
                "A collection guide note cannot target one section.",
            ));
        }
        CollectionStudyGuideOverlayKind::GuideNote => {}
        _ if input.block_key.as_deref().is_none_or(str::is_empty) => {
            return Err(invalid(
                "A section overlay must target one collection guide block.",
            ));
        }
        _ => {}
    }
    let encoded = serde_json::to_string(&input.rich_doc).map_err(CommandError::serialization)?;
    if encoded.len() > MAX_COLLECTION_OVERLAY_DOC_BYTES {
        return Err(invalid(
            "That collection study-guide annotation is too large.",
        ));
    }
    crate::cards::validate_document(&input.rich_doc).map_err(|error| {
        CommandError::new(
            "invalid_study_target",
            format!(
                "The collection study-guide annotation is not valid rich content: {}",
                error.message
            ),
        )
    })?;
    let plain_text = input
        .plain_text
        .chars()
        .take(MAX_COLLECTION_OVERLAY_TEXT_CHARS)
        .collect::<String>()
        .trim()
        .to_string();
    let guide = load_collection_study_guide(database, &input.collection_id)?.ok_or_else(|| {
        CommandError::new(
            "collection_study_guide_not_found",
            "That collection study guide is no longer available.",
        )
    })?;
    if guide.id != input.guide_id {
        return Err(CommandError::new(
            "collection_study_guide_changed",
            "The collection study guide changed before this edit could be saved.",
        ));
    }
    let block_hash = input.block_key.as_deref().and_then(|key| {
        guide
            .content
            .sections
            .iter()
            .find(|section| section.block_key == key)
            .map(|section| section.content_hash.as_str())
    });
    if input.overlay_kind != CollectionStudyGuideOverlayKind::GuideNote && block_hash.is_none() {
        return Err(invalid(
            "The selected collection study-guide section no longer exists.",
        ));
    }
    if input.overlay_kind.content_bound() {
        let expected_hash = input.base_content_hash.as_deref().ok_or_else(|| {
            invalid("A content edit must reference the generated section it edits.")
        })?;
        if expected_hash.len() != 64 || block_hash != Some(expected_hash) {
            return Err(CommandError::new(
                "collection_study_guide_overlay_base_changed",
                "The collection study-guide section changed before this edit could be saved.",
            ));
        }
    }
    let now = unix_time_millis()?;
    let overlay_id = match input.overlay_id.clone() {
        Some(id) => id,
        None => database.with_connection(|connection| generate_id(connection))?,
    };
    database.with_connection(|connection| {
        let target_id: String = connection
            .query_row(
                "SELECT target_id FROM study_target_guides
                 WHERE id=?1 AND state='active'",
                [input.guide_id.as_str()],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        if let Some(expected_revision) = input.expected_revision {
            let changed = connection
                .execute(
                    "UPDATE study_target_guide_overlays
                     SET guide_id=?1,block_key=?2,overlay_kind=?3,base_content_hash=?4,
                         rich_doc_json=?5,plain_text=?6,state='active',
                         revision=revision+1,updated_at=?7
                     WHERE id=?8 AND target_id=?9 AND revision=?10",
                    params![
                        input.guide_id,
                        input.block_key,
                        input.overlay_kind.as_db(),
                        input.base_content_hash,
                        encoded,
                        plain_text,
                        now,
                        overlay_id,
                        target_id,
                        expected_revision,
                    ],
                )
                .map_err(CommandError::database)?;
            if changed != 1 {
                return Err(CommandError::new(
                    "collection_study_guide_overlay_changed",
                    "This collection study-guide annotation changed before it could be saved.",
                ));
            }
        } else {
            connection
                .execute(
                    "INSERT INTO study_target_guide_overlays(
                         id,target_id,guide_id,block_key,overlay_kind,base_content_hash,
                         rich_doc_json,plain_text,state,revision,created_at,updated_at
                     ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'active',1,?9,?9)",
                    params![
                        overlay_id,
                        target_id,
                        input.guide_id,
                        input.block_key,
                        input.overlay_kind.as_db(),
                        input.base_content_hash,
                        encoded,
                        plain_text,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }
        Ok(())
    })?;
    load_collection_guide_overlays(database, &input.guide_id, &guide.target_id)?
        .into_iter()
        .find(|overlay| overlay.id == overlay_id)
        .ok_or_else(|| {
            invalid("The saved collection study-guide annotation could not be reloaded.")
        })
}

fn discard_collection_overlay(
    database: &Database,
    input: DiscardCollectionStudyGuideOverlayInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.collection_id, "document collection")?;
    validate_entity_id(&input.overlay_id, "collection study-guide overlay")?;
    if input.revision < 1 {
        return Err(invalid(
            "The collection study-guide overlay revision was invalid.",
        ));
    }
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE study_target_guide_overlays
                 SET state='discarded',revision=revision+1,updated_at=?1
                 WHERE id=?2 AND revision=?3
                   AND target_id IN (
                       SELECT id FROM study_targets
                       WHERE target_kind='collection' AND collection_id=?4
                   )",
                params![now, input.overlay_id, input.revision, input.collection_id],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "collection_study_guide_overlay_changed",
                "This collection study-guide annotation changed before it could be removed.",
            ));
        }
        Ok(())
    })
}

pub(crate) fn load_collection_study_guide(
    database: &Database,
    collection_id: &str,
) -> Result<Option<CollectionStudyGuide>, CommandError> {
    let stored = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT guide.id,target.id,collection.id,collection.name,guide.revision,
                        guide.source_snapshot_id,guide.content_json,
                        guide.created_at,guide.updated_at,guide.published_at
                 FROM study_target_guides guide
                 JOIN study_targets target ON target.id=guide.target_id
                 JOIN document_collections collection ON collection.id=target.collection_id
                 WHERE target.target_kind='collection' AND target.collection_id=?1
                   AND guide.state='active'",
                [collection_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    let Some((
        id,
        target_id,
        collection_id,
        title,
        revision,
        snapshot_id,
        content_json,
        created_at,
        updated_at,
        published_at,
    )) = stored
    else {
        return Ok(None);
    };
    let content = decode_collection_guide_content(&collection_id, &title, &content_json)?;
    let overlays = load_collection_guide_overlays(database, &id, &target_id)?;
    Ok(Some(CollectionStudyGuide {
        id,
        target_id,
        collection_id,
        title,
        revision,
        source_snapshot_id: snapshot_id,
        content,
        overlays,
        created_at,
        updated_at,
        published_at,
    }))
}

fn decode_collection_guide_content(
    collection_id: &str,
    title: &str,
    content_json: &str,
) -> Result<composition::CollectionGuideContent, CommandError> {
    if let Ok(mut content) =
        serde_json::from_str::<composition::CollectionGuideContent>(content_json)
    {
        // Older collection-guide revisions could expose internal coverage
        // diagnostics as a synthetic learner-visible section. Keep the durable
        // metadata but never surface that implementation detail in the guide.
        content.sections.retain(|section| {
            section.block_key != "coverage"
                && !section.title.trim().eq_ignore_ascii_case("Coverage notes")
        });
        content
            .overview
            .learning_objectives
            .retain(|objective| !objective.trim().to_ascii_lowercase().starts_with("review "));
        return Ok(content);
    }
    let synthesis: CollectionSynthesis =
        serde_json::from_str(content_json).map_err(CommandError::serialization)?;
    let sections = synthesis
        .topics
        .iter()
        .map(|topic| {
            let content = StudyGuideSectionContent {
                overview: if topic.summary.trim().is_empty() {
                    format!("Review {} in the selected source material.", topic.title)
                } else {
                    topic.summary.clone()
                },
                ..Default::default()
            };
            let content_hash = hash_bytes(
                &serde_json::to_vec(&(&topic.title, &content))
                    .map_err(CommandError::serialization)?,
            );
            Ok(composition::CollectionGuideSection {
                block_key: topic.id.clone(),
                title: topic.title.clone(),
                content,
                content_hash,
                source_document_id: None,
                source_document_title: None,
                source_guide_id: None,
                source_topic_id: None,
                source_locator: None,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(composition::CollectionGuideContent {
        schema_version: 0,
        collection_id: collection_id.to_string(),
        title: title.to_string(),
        overview: StudyGuideOverview {
            summary: synthesis.summary,
            learning_objectives: synthesis.learning_objectives,
            key_concepts: synthesis
                .topics
                .iter()
                .map(|topic| topic.title.clone())
                .take(48)
                .collect(),
        },
        sections,
        coverage_notes: synthesis
            .gaps
            .iter()
            .map(|gap| gap.detail.clone())
            .collect(),
    })
}

fn load_collection_guide_overlays(
    database: &Database,
    guide_id: &str,
    target_id: &str,
) -> Result<Vec<CollectionStudyGuideOverlay>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id,guide_id,target_id,block_key,overlay_kind,base_content_hash,
                        rich_doc_json,plain_text,state,revision,created_at,updated_at
                 FROM study_target_guide_overlays
                 WHERE guide_id=?1 AND target_id=?2 AND state IN ('active','conflicted')
                 ORDER BY created_at,id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map(params![guide_id, target_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                ))
            })
            .map_err(CommandError::database)?;
        let overlays = rows
            .map(|row| {
                let (
                    id,
                    guide_id,
                    target_id,
                    block_key,
                    overlay_kind,
                    base_content_hash,
                    rich_doc_json,
                    plain_text,
                    state,
                    revision,
                    created_at,
                    updated_at,
                ) = row.map_err(CommandError::database)?;
                Ok(CollectionStudyGuideOverlay {
                    id,
                    guide_id,
                    target_id,
                    block_key,
                    overlay_kind,
                    base_content_hash,
                    rich_doc: serde_json::from_str(&rich_doc_json)
                        .map_err(CommandError::serialization)?,
                    plain_text,
                    state,
                    revision,
                    created_at,
                    updated_at,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(overlays)
    })
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

        let provider_authorized =
            input.target.kind == StudyTargetKind::Collection && input.output_policy.study_guide;
        tx.execute(
            "INSERT INTO study_target_batches(
                 operation_id,target_id,request_sha256,source_snapshot_id,source_snapshot_json,
                 output_policy_json,provider_authorized,state,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,'queued',?8,?8)",
            params![
                input.operation_id,
                target.id,
                request_hash,
                snapshot.snapshot_id,
                snapshot_json,
                policy_json,
                i64::from(provider_authorized),
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

        let source_changed = if stored.snapshot_id.is_empty() {
            false
        } else {
            resolve_target_snapshot(connection, &stored.target)?.snapshot_id != stored.snapshot_id
        };

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
            source_changed,
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
    fn collection_guide_decoder_hides_internal_coverage_and_placeholder_review_objectives() {
        let content = composition::CollectionGuideContent {
            schema_version: 2,
            collection_id: "c".repeat(32),
            title: "Gut bacteria".to_string(),
            overview: StudyGuideOverview {
                summary: "A readable overview.".to_string(),
                learning_objectives: vec![
                    "Review bacteriophage biology".to_string(),
                    "Compare bacteriophage replication strategies.".to_string(),
                ],
                key_concepts: vec!["Bacteriophages".to_string()],
            },
            sections: vec![
                composition::CollectionGuideSection {
                    block_key: "topic".to_string(),
                    title: "Bacteriophage biology".to_string(),
                    content: StudyGuideSectionContent {
                        overview: "Bacteriophages are viruses that infect bacteria and can follow distinct replication strategies.".to_string(),
                        key_points: vec![
                            "Phages require bacterial hosts for replication.".to_string(),
                            "Replication strategies can differ in timing and host-cell outcome.".to_string(),
                        ],
                        ..Default::default()
                    },
                    content_hash: "a".repeat(64),
                    source_document_id: None,
                    source_document_title: None,
                    source_guide_id: None,
                    source_topic_id: None,
                    source_locator: None,
                },
                composition::CollectionGuideSection {
                    block_key: "coverage".to_string(),
                    title: "Coverage notes".to_string(),
                    content: StudyGuideSectionContent {
                        overview: "Internal coverage diagnostics.".to_string(),
                        ..Default::default()
                    },
                    content_hash: "b".repeat(64),
                    source_document_id: None,
                    source_document_title: None,
                    source_guide_id: None,
                    source_topic_id: None,
                    source_locator: None,
                },
            ],
            coverage_notes: vec![
                "Prepared knowledge exists without the current native coverage receipt.".to_string(),
            ],
        };
        let encoded = serde_json::to_string(&content).unwrap();
        let decoded =
            decode_collection_guide_content(&content.collection_id, &content.title, &encoded)
                .unwrap();

        assert_eq!(decoded.sections.len(), 1);
        assert_eq!(decoded.sections[0].title, "Bacteriophage biology");
        assert_eq!(
            decoded.overview.learning_objectives,
            vec!["Compare bacteriophage replication strategies.".to_string()]
        );
        assert_eq!(decoded.coverage_notes.len(), 1);
    }

    #[test]
    fn collection_guide_overlays_reject_unsupported_rich_content_before_persistence() {
        let database = Database::open_in_memory().unwrap();
        let error = save_collection_overlay(
            &database,
            SaveCollectionStudyGuideOverlayInput {
                overlay_id: None,
                collection_id: "a".repeat(32),
                guide_id: "b".repeat(32),
                block_key: None,
                overlay_kind: CollectionStudyGuideOverlayKind::GuideNote,
                base_content_hash: None,
                rich_doc: json!({
                    "type": "doc",
                    "content": [{
                        "type": "script",
                        "attrs": {"src": "file:///tmp/not-allowed"}
                    }]
                }),
                plain_text: "unsafe".to_string(),
                expected_revision: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "invalid_study_target");
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
