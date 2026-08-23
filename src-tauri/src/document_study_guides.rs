//! Durable document study guides and learner-owned editing overlays.
//!
//! Generated guide content is immutable by revision. Learner annotations and
//! edits are stored separately so regeneration can replace derived prose without
//! deleting user work. Source-grounded topic identity comes from
//! `document_learning`; guide sections never invent their own topic identities or
//! source locators.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    database::Database,
    document_domain::SourceLocator,
    document_learning::{load_document_learning_plan, DocumentLearningPlan},
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

pub(crate) const STUDY_GUIDE_SCHEMA_VERSION: i64 = 1;
pub(crate) const STUDY_GUIDE_PROMPT_VERSION: i64 = 1;
pub(crate) const STUDY_GUIDE_ASSEMBLY_VERSION: i64 = 1;

const MAX_GUIDE_SECTIONS: usize = 512;
const MAX_OVERVIEW_CHARS: usize = 200_000;
const MAX_SECTION_TEXT_CHARS: usize = 500_000;
const MAX_SECTION_ITEMS: usize = 96;
const MAX_SECTION_SUBSECTIONS: usize = 48;
const MAX_ITEM_CHARS: usize = 20_000;
const MAX_TERM_CHARS: usize = 1_000;
const MAX_RICH_DOC_BYTES: usize = 2 * 1024 * 1024;
const MAX_OVERLAY_TEXT_CHARS: usize = 500_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyGuideOverview {
    pub summary: String,
    #[serde(default)]
    pub learning_objectives: Vec<String>,
    #[serde(default)]
    pub key_concepts: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyGuideSectionContent {
    pub overview: String,
    #[serde(default)]
    pub key_points: Vec<String>,
    #[serde(default)]
    pub explanations: Vec<StudyGuideExplanation>,
    #[serde(default)]
    pub important_terms: Vec<StudyGuideTerm>,
    #[serde(default)]
    pub process_steps: Vec<String>,
    #[serde(default)]
    pub comparisons: Vec<StudyGuideComparison>,
    #[serde(default)]
    pub common_misconceptions: Vec<String>,
    #[serde(default)]
    pub visual_explanations: Vec<StudyGuideVisualExplanation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyGuideExplanation {
    pub heading: String,
    pub body: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyGuideTerm {
    pub term: String,
    pub explanation: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyGuideComparison {
    pub subject_a: String,
    pub subject_b: String,
    pub relationship: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StudyGuideVisualExplanation {
    pub title: String,
    pub explanation: String,
}

#[derive(Clone, Debug)]
pub(crate) struct StudyGuideSectionDraft {
    pub topic_id: String,
    pub content: StudyGuideSectionContent,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentStudyGuide {
    pub id: String,
    pub document_id: String,
    pub plan_id: String,
    pub job_id: Option<String>,
    pub revision: i64,
    pub schema_version: i64,
    pub prompt_version: i64,
    pub assembly_version: i64,
    pub overview: StudyGuideOverview,
    pub overview_text: String,
    pub sections: Vec<DocumentStudyGuideSection>,
    pub overlays: Vec<StudyGuideOverlay>,
    pub created_at: i64,
    pub updated_at: i64,
    pub published_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentStudyGuideSection {
    pub topic_id: String,
    pub parent_topic_id: Option<String>,
    pub sort_index: usize,
    pub title: String,
    pub topic_summary: String,
    pub content: StudyGuideSectionContent,
    pub content_text: String,
    pub content_hash: String,
    pub source_locator: SourceLocator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StudyGuideOverlayKind {
    GuideNote,
    TopicNote,
    TitleOverride,
    BodyOverride,
    Highlight,
}

impl StudyGuideOverlayKind {
    fn as_db(self) -> &'static str {
        match self {
            Self::GuideNote => "guide_note",
            Self::TopicNote => "topic_note",
            Self::TitleOverride => "title_override",
            Self::BodyOverride => "body_override",
            Self::Highlight => "highlight",
        }
    }

    fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "guide_note" => Ok(Self::GuideNote),
            "topic_note" => Ok(Self::TopicNote),
            "title_override" => Ok(Self::TitleOverride),
            "body_override" => Ok(Self::BodyOverride),
            "highlight" => Ok(Self::Highlight),
            _ => Err(CommandError::new(
                "invalid_study_guide_overlay",
                "A persisted study-guide annotation type was invalid.",
            )),
        }
    }

    fn is_content_bound(self) -> bool {
        matches!(
            self,
            Self::TitleOverride | Self::BodyOverride | Self::Highlight
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StudyGuideOverlayState {
    Active,
    Conflicted,
    Discarded,
}

impl StudyGuideOverlayState {
    fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "active" => Ok(Self::Active),
            "conflicted" => Ok(Self::Conflicted),
            "discarded" => Ok(Self::Discarded),
            _ => Err(CommandError::new(
                "invalid_study_guide_overlay",
                "A persisted study-guide annotation state was invalid.",
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StudyGuideOverlay {
    pub id: String,
    pub document_id: String,
    pub guide_id: Option<String>,
    pub topic_id: Option<String>,
    pub overlay_kind: StudyGuideOverlayKind,
    pub base_content_hash: Option<String>,
    pub rich_doc: Value,
    pub plain_text: String,
    pub state: StudyGuideOverlayState,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SaveStudyGuideOverlayInput {
    pub overlay_id: Option<String>,
    pub document_id: String,
    pub guide_id: Option<String>,
    pub topic_id: Option<String>,
    pub overlay_kind: StudyGuideOverlayKind,
    pub base_content_hash: Option<String>,
    pub rich_doc: Value,
    pub plain_text: String,
    pub expected_revision: Option<i64>,
}

/// Publish a complete immutable study-guide revision atomically.
///
/// Existing active content is superseded only inside the same transaction that
/// inserts the complete replacement. A malformed or incomplete replacement
/// leaves the prior active guide untouched. Learner notes remain document/topic
/// overlays; content-bound edits move forward only when the topic and exact base
/// content are unchanged, otherwise they become explicit conflicts.
pub(crate) fn publish_document_study_guide(
    database: &Database,
    document_id: &str,
    plan_id: &str,
    job_id: Option<&str>,
    overview: StudyGuideOverview,
    section_drafts: Vec<StudyGuideSectionDraft>,
) -> Result<DocumentStudyGuide, CommandError> {
    validate_entity_id(document_id, "document")?;
    validate_entity_id(plan_id, "learning plan")?;
    if let Some(job_id) = job_id {
        validate_entity_id(job_id, "artifact job")?;
    }

    let plan = load_document_learning_plan(database, plan_id)?;
    if plan.document_id != document_id {
        return Err(CommandError::new(
            "document_learning_plan_mismatch",
            "The selected learning plan does not belong to this document.",
        ));
    }
    let normalized_overview = normalize_overview(overview);
    let sections = normalize_complete_sections(&plan, section_drafts)?;
    let overview_json = serialize_json(&normalized_overview)?;
    let overview_text = overview_plain_text(&normalized_overview);
    let now = unix_time_millis()?;
    let guide_id = database.with_connection(|connection| generate_id(connection))?;

    database.with_connection(|connection| {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;

        if let Some(job_id) = job_id {
            let job_matches = transaction
                .query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM document_artifact_jobs
                         WHERE id = ?1 AND document_id = ?2
                           AND artifact_kind = 'study_guide'
                           AND state = 'committing'
                     )",
                    params![job_id, document_id],
                    |row| row.get::<_, i64>(0),
                )
                .map(|value| value != 0)
                .map_err(CommandError::database)?;
            if !job_matches {
                return Err(CommandError::new(
                    "document_artifact_job_changed",
                    "The study-guide job changed before its guide could be published.",
                ));
            }
        }

        let revision = transaction
            .query_row(
                "SELECT COALESCE(MAX(revision), 0) + 1
                 FROM document_study_guides WHERE document_id = ?1",
                [document_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(CommandError::database)?;
        let prior_active: Option<String> = transaction
            .query_row(
                "SELECT id FROM document_study_guides
                 WHERE document_id = ?1 AND state = 'active'",
                [document_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?;

        if let Some(prior_id) = prior_active.as_deref() {
            transaction
                .execute(
                    "UPDATE document_study_guides
                     SET state = 'superseded', updated_at = ?1
                     WHERE id = ?2 AND state = 'active'",
                    params![now, prior_id],
                )
                .map_err(CommandError::database)?;
        }

        transaction
            .execute(
                "INSERT INTO document_study_guides (
                     id, document_id, plan_id, job_id, revision,
                     schema_version, prompt_version, assembly_version,
                     overview_json, overview_text, state,
                     created_at, updated_at, published_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                           'active', ?11, ?11, ?11)",
                params![
                    guide_id,
                    document_id,
                    plan_id,
                    job_id,
                    revision,
                    STUDY_GUIDE_SCHEMA_VERSION,
                    STUDY_GUIDE_PROMPT_VERSION,
                    STUDY_GUIDE_ASSEMBLY_VERSION,
                    overview_json,
                    overview_text,
                    now,
                ],
            )
            .map_err(CommandError::database)?;

        for section in &sections {
            transaction
                .execute(
                    "INSERT INTO document_study_guide_sections (
                         guide_id, topic_id, sort_index, content_json,
                         content_text, content_hash, source_locator_json,
                         created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                    params![
                        guide_id,
                        section.topic_id,
                        to_i64(section.sort_index),
                        serialize_json(&section.content)?,
                        section.content_text,
                        section.content_hash,
                        serialize_json(&section.source_locator)?,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }

        if let Some(prior_id) = prior_active.as_deref() {
            // Move an edit/highlight to the new revision only when its topic and
            // exact generated base content are unchanged.
            transaction
                .execute(
                    "UPDATE document_study_guide_overlays
                     SET guide_id = ?1, updated_at = ?2, revision = revision + 1
                     WHERE guide_id = ?3 AND state = 'active'
                       AND overlay_kind IN ('title_override', 'body_override', 'highlight')
                       AND EXISTS (
                           SELECT 1 FROM document_study_guide_sections section
                           WHERE section.guide_id = ?1
                             AND section.topic_id = document_study_guide_overlays.topic_id
                             AND section.content_hash = document_study_guide_overlays.base_content_hash
                       )",
                    params![guide_id, now, prior_id],
                )
                .map_err(CommandError::database)?;
            transaction
                .execute(
                    "UPDATE document_study_guide_overlays
                     SET state = 'conflicted', updated_at = ?1, revision = revision + 1
                     WHERE guide_id = ?2 AND state = 'active'
                       AND overlay_kind IN ('title_override', 'body_override', 'highlight')",
                    params![now, prior_id],
                )
                .map_err(CommandError::database)?;
        }

        let document_changed = transaction
            .execute(
                "UPDATE documents
                 SET lifecycle_state = CASE WHEN archived_at IS NULL THEN 'ready' ELSE 'archived' END,
                     safe_error_code = CASE WHEN archived_at IS NULL THEN '' ELSE safe_error_code END,
                     safe_error_message = CASE WHEN archived_at IS NULL THEN '' ELSE safe_error_message END,
                     completed_at = CASE
                         WHEN archived_at IS NULL THEN COALESCE(completed_at, ?1)
                         ELSE completed_at
                     END,
                     updated_at = ?1, revision = revision + 1
                 WHERE id = ?2",
                params![now, document_id],
            )
            .map_err(CommandError::database)?;
        if document_changed != 1 {
            return Err(CommandError::new(
                "document_changed",
                "The document changed before its study guide could be published.",
            ));
        }

        transaction.commit().map_err(CommandError::database)
    })?;
    load_document_study_guide(database, &guide_id)
}

pub(crate) fn load_active_document_study_guide(
    database: &Database,
    document_id: &str,
) -> Result<Option<DocumentStudyGuide>, CommandError> {
    validate_entity_id(document_id, "document")?;
    let guide_id = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT id FROM document_study_guides
                 WHERE document_id = ?1 AND state = 'active'",
                [document_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)
    })?;
    guide_id
        .map(|guide_id| load_document_study_guide(database, &guide_id))
        .transpose()
}

pub(crate) fn load_document_study_guide(
    database: &Database,
    guide_id: &str,
) -> Result<DocumentStudyGuide, CommandError> {
    validate_entity_id(guide_id, "study guide")?;
    let header = database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT document_id, plan_id, job_id, revision, schema_version,
                        prompt_version, assembly_version, overview_json,
                        overview_text, created_at, updated_at, published_at
                 FROM document_study_guides WHERE id = ?1",
                [guide_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
                    ))
                },
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "document_study_guide_not_found",
                    "That study guide is no longer available.",
                )
            })
    })?;
    let plan = load_document_learning_plan(database, &header.1)?;
    if plan.document_id != header.0 {
        return Err(CommandError::new(
            "document_learning_plan_invalid",
            "The persisted study guide and learning plan have inconsistent document ownership.",
        ));
    }
    let topics = plan
        .topics
        .iter()
        .map(|topic| (topic.id.as_str(), topic))
        .collect::<HashMap<_, _>>();
    let sections = database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT topic_id, sort_index, content_json, content_text,
                        content_hash, source_locator_json
                 FROM document_study_guide_sections
                 WHERE guide_id = ?1 ORDER BY sort_index, topic_id",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([guide_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(CommandError::database)?;
        let sections = rows
            .map(|row| {
                let (topic_id, sort_index, content_json, content_text, content_hash, locator_json) =
                    row.map_err(CommandError::database)?;
                let topic = topics.get(topic_id.as_str()).ok_or_else(|| {
                    CommandError::new(
                        "document_study_guide_invalid",
                        "A persisted study-guide section referenced a topic outside its learning plan.",
                    )
                })?;
                Ok(DocumentStudyGuideSection {
                    topic_id,
                    parent_topic_id: topic.parent_topic_id.clone(),
                    sort_index: to_usize(sort_index)?,
                    title: topic.title.clone(),
                    topic_summary: topic.summary.clone(),
                    content: deserialize_json(&content_json)?,
                    content_text,
                    content_hash,
                    source_locator: deserialize_json(&locator_json)?,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(sections)
    })?;
    let overlays = load_document_study_guide_overlays(database, &header.0, Some(guide_id))?;

    Ok(DocumentStudyGuide {
        id: guide_id.to_string(),
        document_id: header.0,
        plan_id: header.1,
        job_id: header.2,
        revision: header.3,
        schema_version: header.4,
        prompt_version: header.5,
        assembly_version: header.6,
        overview: deserialize_json(&header.7)?,
        overview_text: header.8,
        sections,
        overlays,
        created_at: header.9,
        updated_at: header.10,
        published_at: header.11,
    })
}

pub(crate) fn save_study_guide_overlay(
    database: &Database,
    input: SaveStudyGuideOverlayInput,
) -> Result<StudyGuideOverlay, CommandError> {
    validate_entity_id(&input.document_id, "document")?;
    if let Some(guide_id) = input.guide_id.as_deref() {
        validate_entity_id(guide_id, "study guide")?;
    }
    if let Some(topic_id) = input.topic_id.as_deref() {
        validate_entity_id(topic_id, "learning topic")?;
    }
    if let Some(overlay_id) = input.overlay_id.as_deref() {
        validate_entity_id(overlay_id, "study guide overlay")?;
    }
    validate_overlay_contract(&input)?;
    let rich_doc_json = serialize_json(&input.rich_doc)?;
    if rich_doc_json.len() > MAX_RICH_DOC_BYTES {
        return Err(CommandError::new(
            "study_guide_overlay_too_large",
            "That study-guide annotation is too large to save.",
        ));
    }
    let plain_text = normalize_text(&input.plain_text, MAX_OVERLAY_TEXT_CHARS, "");
    let now = unix_time_millis()?;
    let overlay_id = match input.overlay_id.clone() {
        Some(overlay_id) => overlay_id,
        None => database.with_connection(|connection| generate_id(connection))?,
    };

    database.with_connection(|connection| {
        validate_overlay_scope(
            connection,
            &input.document_id,
            input.guide_id.as_deref(),
            input.topic_id.as_deref(),
        )?;
        if let Some(expected_revision) = input.expected_revision {
            if expected_revision < 1 {
                return Err(CommandError::new(
                    "invalid_revision",
                    "The study-guide annotation revision was invalid.",
                ));
            }
            let changed = connection
                .execute(
                    "UPDATE document_study_guide_overlays
                     SET guide_id = ?1, topic_id = ?2, overlay_kind = ?3,
                         base_content_hash = ?4, rich_doc_json = ?5,
                         plain_text = ?6, state = 'active',
                         revision = revision + 1, updated_at = ?7
                     WHERE id = ?8 AND document_id = ?9 AND revision = ?10",
                    params![
                        input.guide_id,
                        input.topic_id,
                        input.overlay_kind.as_db(),
                        input.base_content_hash,
                        rich_doc_json,
                        plain_text,
                        now,
                        overlay_id,
                        input.document_id,
                        expected_revision,
                    ],
                )
                .map_err(CommandError::database)?;
            if changed != 1 {
                return Err(CommandError::new(
                    "study_guide_overlay_changed",
                    "This study-guide annotation changed before it could be saved. Reload it and try again.",
                ));
            }
        } else {
            connection
                .execute(
                    "INSERT INTO document_study_guide_overlays (
                         id, document_id, guide_id, topic_id, overlay_kind,
                         base_content_hash, rich_doc_json, plain_text,
                         state, revision, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                               'active', 1, ?9, ?9)",
                    params![
                        overlay_id,
                        input.document_id,
                        input.guide_id,
                        input.topic_id,
                        input.overlay_kind.as_db(),
                        input.base_content_hash,
                        rich_doc_json,
                        plain_text,
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }
        Ok(())
    })?;
    load_study_guide_overlay(database, &overlay_id)
}

pub(crate) fn discard_study_guide_overlay(
    database: &Database,
    overlay_id: &str,
    expected_revision: i64,
) -> Result<(), CommandError> {
    validate_entity_id(overlay_id, "study guide overlay")?;
    if expected_revision < 1 {
        return Err(CommandError::new(
            "invalid_revision",
            "The study-guide annotation revision was invalid.",
        ));
    }
    let now = unix_time_millis()?;
    database.with_connection(|connection| {
        let changed = connection
            .execute(
                "UPDATE document_study_guide_overlays
                 SET state = 'discarded', revision = revision + 1, updated_at = ?1
                 WHERE id = ?2 AND revision = ?3 AND state <> 'discarded'",
                params![now, overlay_id, expected_revision],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "study_guide_overlay_changed",
                "This study-guide annotation changed before it could be discarded.",
            ));
        }
        Ok(())
    })
}

pub(crate) fn load_document_study_guide_overlays(
    database: &Database,
    document_id: &str,
    guide_id: Option<&str>,
) -> Result<Vec<StudyGuideOverlay>, CommandError> {
    validate_entity_id(document_id, "document")?;
    if let Some(guide_id) = guide_id {
        validate_entity_id(guide_id, "study guide")?;
    }
    database.with_connection(|connection| {
        load_overlays_from_connection(connection, document_id, guide_id)
    })
}

fn load_study_guide_overlay(
    database: &Database,
    overlay_id: &str,
) -> Result<StudyGuideOverlay, CommandError> {
    database.with_connection(|connection| {
        connection
            .query_row(
                "SELECT document_id, guide_id, topic_id, overlay_kind,
                        base_content_hash, rich_doc_json, plain_text, state,
                        revision, created_at, updated_at
                 FROM document_study_guide_overlays WHERE id = ?1",
                [overlay_id],
                overlay_row,
            )
            .optional()
            .map_err(CommandError::database)?
            .ok_or_else(|| {
                CommandError::new(
                    "study_guide_overlay_not_found",
                    "That study-guide annotation is no longer available.",
                )
            })
            .map(|mut overlay| {
                overlay.id = overlay_id.to_string();
                overlay
            })
    })
}

fn load_overlays_from_connection(
    connection: &rusqlite::Connection,
    document_id: &str,
    guide_id: Option<&str>,
) -> Result<Vec<StudyGuideOverlay>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT id, document_id, guide_id, topic_id, overlay_kind,
                    base_content_hash, rich_doc_json, plain_text, state,
                    revision, created_at, updated_at
             FROM document_study_guide_overlays
             WHERE document_id = ?1
               AND state <> 'discarded'
               AND (guide_id IS NULL OR guide_id = ?2)
             ORDER BY created_at, id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![document_id, guide_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
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
            let row = row.map_err(CommandError::database)?;
            Ok(StudyGuideOverlay {
                id: row.0,
                document_id: row.1,
                guide_id: row.2,
                topic_id: row.3,
                overlay_kind: StudyGuideOverlayKind::from_db(&row.4)?,
                base_content_hash: row.5,
                rich_doc: deserialize_json(&row.6)?,
                plain_text: row.7,
                state: StudyGuideOverlayState::from_db(&row.8)?,
                revision: row.9,
                created_at: row.10,
                updated_at: row.11,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(overlays)
}

fn overlay_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StudyGuideOverlay> {
    let rich_doc_json: String = row.get(5)?;
    let rich_doc = serde_json::from_str(&rich_doc_json).unwrap_or(Value::Null);
    let overlay_kind = StudyGuideOverlayKind::from_db(&row.get::<_, String>(3)?)
        .unwrap_or(StudyGuideOverlayKind::GuideNote);
    let state = StudyGuideOverlayState::from_db(&row.get::<_, String>(7)?)
        .unwrap_or(StudyGuideOverlayState::Conflicted);
    Ok(StudyGuideOverlay {
        id: String::new(),
        document_id: row.get(0)?,
        guide_id: row.get(1)?,
        topic_id: row.get(2)?,
        overlay_kind,
        base_content_hash: row.get(4)?,
        rich_doc,
        plain_text: row.get(6)?,
        state,
        revision: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn normalize_complete_sections(
    plan: &DocumentLearningPlan,
    drafts: Vec<StudyGuideSectionDraft>,
) -> Result<Vec<DocumentStudyGuideSection>, CommandError> {
    if plan.topics.is_empty() || plan.topics.len() > MAX_GUIDE_SECTIONS {
        return Err(CommandError::new(
            "document_study_guide_invalid",
            "The learning plan does not contain a supported number of guide topics.",
        ));
    }
    if drafts.len() != plan.topics.len() {
        return Err(CommandError::new(
            "document_study_guide_incomplete",
            "Every learning-plan topic must have exactly one validated study-guide section before publication.",
        ));
    }
    let mut drafts_by_topic = HashMap::with_capacity(drafts.len());
    for draft in drafts {
        validate_entity_id(&draft.topic_id, "learning topic")?;
        if drafts_by_topic
            .insert(draft.topic_id, draft.content)
            .is_some()
        {
            return Err(CommandError::new(
                "document_study_guide_invalid",
                "The generated study guide contained a duplicate topic section.",
            ));
        }
    }

    let mut sections = Vec::with_capacity(plan.topics.len());
    for topic in &plan.topics {
        let content = drafts_by_topic.remove(&topic.id).ok_or_else(|| {
            CommandError::new(
                "document_study_guide_incomplete",
                format!(
                    "The study guide is missing the section for topic '{}'.",
                    topic.title
                ),
            )
        })?;
        let content = normalize_section_content(content)?;
        let content_json = serialize_json(&content)?;
        let content_text = section_plain_text(&content);
        sections.push(DocumentStudyGuideSection {
            topic_id: topic.id.clone(),
            parent_topic_id: topic.parent_topic_id.clone(),
            sort_index: topic.sort_index,
            title: topic.title.clone(),
            topic_summary: topic.summary.clone(),
            content,
            content_text,
            content_hash: hash_text(&content_json),
            source_locator: topic.source_locator.clone(),
        });
    }
    if !drafts_by_topic.is_empty() {
        return Err(CommandError::new(
            "document_study_guide_invalid",
            "The generated study guide contained a section outside the active learning plan.",
        ));
    }
    Ok(sections)
}

fn normalize_overview(overview: StudyGuideOverview) -> StudyGuideOverview {
    StudyGuideOverview {
        summary: normalize_text(
            &overview.summary,
            MAX_OVERVIEW_CHARS,
            "Structured overview of the source document.",
        ),
        learning_objectives: normalize_items(
            overview.learning_objectives,
            MAX_SECTION_ITEMS,
            MAX_ITEM_CHARS,
        ),
        key_concepts: normalize_items(overview.key_concepts, MAX_SECTION_ITEMS, MAX_ITEM_CHARS),
    }
}

pub(crate) fn validate_study_guide_section_content(
    content: StudyGuideSectionContent,
) -> Result<StudyGuideSectionContent, CommandError> {
    normalize_section_content(content)
}

fn normalize_section_content(
    content: StudyGuideSectionContent,
) -> Result<StudyGuideSectionContent, CommandError> {
    let normalized = StudyGuideSectionContent {
        overview: normalize_text(
            &content.overview,
            MAX_SECTION_TEXT_CHARS,
            "Source-grounded explanation for this topic.",
        ),
        key_points: normalize_items(content.key_points, MAX_SECTION_ITEMS, MAX_ITEM_CHARS),
        explanations: content
            .explanations
            .into_iter()
            .take(MAX_SECTION_SUBSECTIONS)
            .map(|value| StudyGuideExplanation {
                heading: normalize_text(&value.heading, MAX_TERM_CHARS, "Explanation"),
                body: normalize_text(&value.body, MAX_ITEM_CHARS, ""),
            })
            .filter(|value| !value.body.is_empty())
            .collect(),
        important_terms: content
            .important_terms
            .into_iter()
            .take(MAX_SECTION_ITEMS)
            .map(|value| StudyGuideTerm {
                term: normalize_text(&value.term, MAX_TERM_CHARS, ""),
                explanation: normalize_text(&value.explanation, MAX_ITEM_CHARS, ""),
            })
            .filter(|value| !value.term.is_empty() && !value.explanation.is_empty())
            .collect(),
        process_steps: normalize_items(content.process_steps, MAX_SECTION_ITEMS, MAX_ITEM_CHARS),
        comparisons: content
            .comparisons
            .into_iter()
            .take(MAX_SECTION_ITEMS)
            .map(|value| StudyGuideComparison {
                subject_a: normalize_text(&value.subject_a, MAX_TERM_CHARS, ""),
                subject_b: normalize_text(&value.subject_b, MAX_TERM_CHARS, ""),
                relationship: normalize_text(&value.relationship, MAX_ITEM_CHARS, ""),
            })
            .filter(|value| {
                !value.subject_a.is_empty()
                    && !value.subject_b.is_empty()
                    && !value.relationship.is_empty()
            })
            .collect(),
        common_misconceptions: normalize_items(
            content.common_misconceptions,
            MAX_SECTION_ITEMS,
            MAX_ITEM_CHARS,
        ),
        visual_explanations: content
            .visual_explanations
            .into_iter()
            .take(MAX_SECTION_ITEMS)
            .map(|value| StudyGuideVisualExplanation {
                title: normalize_text(&value.title, MAX_TERM_CHARS, "Visual evidence"),
                explanation: normalize_text(&value.explanation, MAX_ITEM_CHARS, ""),
            })
            .filter(|value| !value.explanation.is_empty())
            .collect(),
    };
    let text = section_plain_text(&normalized);
    if text.trim().is_empty() || text.chars().count() > MAX_SECTION_TEXT_CHARS {
        return Err(CommandError::new(
            "document_study_guide_section_invalid",
            "A generated study-guide section was empty or exceeded the local size limit.",
        ));
    }
    Ok(normalized)
}

fn validate_overlay_contract(input: &SaveStudyGuideOverlayInput) -> Result<(), CommandError> {
    match input.overlay_kind {
        StudyGuideOverlayKind::GuideNote => {
            if input.topic_id.is_some() || input.guide_id.is_some() {
                return Err(CommandError::new(
                    "invalid_study_guide_overlay",
                    "A guide-wide note belongs to the document rather than one generated guide revision or topic.",
                ));
            }
        }
        StudyGuideOverlayKind::TopicNote => {
            if input.topic_id.is_none() || input.guide_id.is_some() {
                return Err(CommandError::new(
                    "invalid_study_guide_overlay",
                    "A topic note must reference a stable learning topic and remain independent of guide revisions.",
                ));
            }
        }
        kind if kind.is_content_bound()
            && (input.topic_id.is_none()
                || input.guide_id.is_none()
                || input
                    .base_content_hash
                    .as_deref()
                    .is_none_or(|hash| !is_sha256(hash))) =>
        {
            return Err(CommandError::new(
                "invalid_study_guide_overlay",
                "A content edit or highlight must reference a guide revision, topic, and valid base-content hash.",
            ));
        }
        _ => {}
    }
    crate::cards::validate_document(&input.rich_doc).map_err(|error| {
        CommandError::new(
            "invalid_study_guide_overlay",
            format!(
                "The study-guide annotation is not valid rich content: {}",
                error.message
            ),
        )
    })
}

fn validate_overlay_scope(
    connection: &rusqlite::Connection,
    document_id: &str,
    guide_id: Option<&str>,
    topic_id: Option<&str>,
) -> Result<(), CommandError> {
    if let Some(guide_id) = guide_id {
        let matches = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM document_study_guides
                     WHERE id = ?1 AND document_id = ?2
                 )",
                params![guide_id, document_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(CommandError::database)?;
        if !matches {
            return Err(CommandError::new(
                "document_study_guide_not_found",
                "The selected study guide does not belong to this document.",
            ));
        }
    }
    if let Some(topic_id) = topic_id {
        let matches = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM document_learning_topics
                     WHERE id = ?1 AND document_id = ?2
                 )",
                params![topic_id, document_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(CommandError::database)?;
        if !matches {
            return Err(CommandError::new(
                "document_learning_topic_not_found",
                "The selected learning topic does not belong to this document.",
            ));
        }
    }
    Ok(())
}

fn overview_plain_text(overview: &StudyGuideOverview) -> String {
    let mut lines = vec![overview.summary.clone()];
    lines.extend(overview.learning_objectives.iter().cloned());
    lines.extend(overview.key_concepts.iter().cloned());
    normalize_text(&lines.join("\n"), MAX_OVERVIEW_CHARS, "")
}

fn section_plain_text(content: &StudyGuideSectionContent) -> String {
    let mut lines = vec![content.overview.clone()];
    lines.extend(content.key_points.iter().cloned());
    for explanation in &content.explanations {
        lines.push(explanation.heading.clone());
        lines.push(explanation.body.clone());
    }
    for term in &content.important_terms {
        lines.push(format!("{}: {}", term.term, term.explanation));
    }
    lines.extend(content.process_steps.iter().cloned());
    for comparison in &content.comparisons {
        lines.push(format!(
            "{} / {}: {}",
            comparison.subject_a, comparison.subject_b, comparison.relationship
        ));
    }
    lines.extend(content.common_misconceptions.iter().cloned());
    for visual in &content.visual_explanations {
        lines.push(visual.title.clone());
        lines.push(visual.explanation.clone());
    }
    normalize_text(&lines.join("\n"), MAX_SECTION_TEXT_CHARS, "")
}

fn normalize_items(values: Vec<String>, maximum: usize, maximum_chars: usize) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| {
            let value = normalize_text(&value, maximum_chars, "");
            if value.is_empty() {
                return None;
            }
            seen.insert(value.to_ascii_lowercase()).then_some(value)
        })
        .take(maximum)
        .collect()
}

fn normalize_text(value: &str, maximum_chars: usize, fallback: &str) -> String {
    let selected = if value.trim().is_empty() {
        fallback.trim()
    } else {
        value.trim()
    };
    selected.chars().take(maximum_chars).collect()
}

fn serialize_json<T: Serialize>(value: &T) -> Result<String, CommandError> {
    serde_json::to_string(value).map_err(CommandError::serialization)
}

fn deserialize_json<T: for<'de> Deserialize<'de>>(value: &str) -> Result<T, CommandError> {
    serde_json::from_str(value).map_err(CommandError::serialization)
}

fn hash_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn to_usize(value: i64) -> Result<usize, CommandError> {
    usize::try_from(value).map_err(|_| {
        CommandError::new(
            "document_study_guide_invalid",
            "A persisted study-guide section index was invalid.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_domain::SourceUnitKind,
        document_learning::{
            ensure_document_learning_plan, DocumentLearningPlanInput, LearningEvidenceChunk,
            LearningTopicInput,
        },
    };

    fn content() -> StudyGuideSectionContent {
        StudyGuideSectionContent {
            overview: " Preload describes ventricular filling before contraction. ".to_string(),
            key_points: vec!["More filling can increase stroke volume.".to_string()],
            explanations: vec![StudyGuideExplanation {
                heading: "Mechanism".to_string(),
                body: "Sarcomere stretch changes force generation.".to_string(),
            }],
            important_terms: vec![StudyGuideTerm {
                term: "Preload".to_string(),
                explanation: "Ventricular filling/stretch before systole.".to_string(),
            }],
            process_steps: Vec::new(),
            comparisons: Vec::new(),
            common_misconceptions: Vec::new(),
            visual_explanations: Vec::new(),
        }
    }

    fn rich_doc(text: &str) -> Value {
        serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": text}]
            }]
        })
    }

    #[test]
    fn normalized_section_has_deterministic_plain_text_and_hash() {
        let first = normalize_section_content(content()).unwrap();
        let second = normalize_section_content(content()).unwrap();
        let first_json = serialize_json(&first).unwrap();
        let second_json = serialize_json(&second).unwrap();
        assert_eq!(first_json, second_json);
        assert_eq!(hash_text(&first_json), hash_text(&second_json));
        assert!(section_plain_text(&first).contains("Sarcomere stretch"));
    }

    #[test]
    fn topic_notes_are_revision_independent() {
        let input = SaveStudyGuideOverlayInput {
            overlay_id: None,
            document_id: "a".repeat(32),
            guide_id: None,
            topic_id: Some("b".repeat(32)),
            overlay_kind: StudyGuideOverlayKind::TopicNote,
            base_content_hash: None,
            rich_doc: rich_doc("Remember this mechanism"),
            plain_text: "Remember this mechanism".to_string(),
            expected_revision: None,
        };
        assert!(validate_overlay_contract(&input).is_ok());
    }

    #[test]
    fn direct_content_edits_require_revision_and_base_hash() {
        let mut input = SaveStudyGuideOverlayInput {
            overlay_id: None,
            document_id: "a".repeat(32),
            guide_id: None,
            topic_id: Some("b".repeat(32)),
            overlay_kind: StudyGuideOverlayKind::BodyOverride,
            base_content_hash: None,
            rich_doc: rich_doc("Edited explanation"),
            plain_text: "Edited explanation".to_string(),
            expected_revision: None,
        };
        assert!(validate_overlay_contract(&input).is_err());
        input.guide_id = Some("c".repeat(32));
        input.base_content_hash = Some("d".repeat(64));
        assert!(validate_overlay_contract(&input).is_ok());
    }

    #[test]
    fn publishing_guide_atomically_marks_guide_only_document_ready() {
        let database = Database::open_in_memory().expect("database should open");
        let document_id = "1".repeat(32);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO documents (
                         id, title, original_filename, mime_type, byte_size, sha256,
                         source_relative_path, lifecycle_state, safe_error_code,
                         safe_error_message, created_at, updated_at
                     ) VALUES (?1, 'Guide only', 'guide.pdf', 'application/pdf', 1,
                               ?2, 'documents/guide/source.pdf', 'failed',
                               'prior_failure', 'Prior failure', 1, 1)",
                        params![document_id, "2".repeat(64)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .expect("document fixture should insert");

        let locator = SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap();
        let plan = ensure_document_learning_plan(
            &database,
            &document_id,
            DocumentLearningPlanInput {
                summary: "Guide summary".to_string(),
                learning_objectives: vec!["Explain preload".to_string()],
                key_concepts: vec!["Preload".to_string()],
                topics: vec![LearningTopicInput {
                    parent_index: None,
                    title: "Preload".to_string(),
                    summary: "Ventricular filling before systole".to_string(),
                    learning_objectives: Vec::new(),
                    key_concepts: vec!["Frank-Starling".to_string()],
                    source_locator: locator.clone(),
                }],
            },
            &[LearningEvidenceChunk {
                chunk_index: 0,
                source_locator: locator,
                content_hash: "3".repeat(64),
            }],
        )
        .expect("learning plan should publish");

        let guide = publish_document_study_guide(
            &database,
            &document_id,
            &plan.id,
            None,
            StudyGuideOverview {
                summary: plan.summary.clone(),
                learning_objectives: plan.learning_objectives.clone(),
                key_concepts: plan.key_concepts.clone(),
            },
            vec![StudyGuideSectionDraft {
                topic_id: plan.topics[0].id.clone(),
                content: content(),
            }],
        )
        .expect("guide should publish");
        assert_eq!(guide.document_id, document_id);

        database
            .with_connection(|connection| {
                let state = connection
                    .query_row(
                        "SELECT lifecycle_state, safe_error_code, safe_error_message,
                                completed_at, generated_deck_id
                         FROM documents WHERE id = ?1",
                        [&document_id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, Option<i64>>(3)?,
                                row.get::<_, Option<String>>(4)?,
                            ))
                        },
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(state.0, "ready");
                assert!(state.1.is_empty());
                assert!(state.2.is_empty());
                assert!(state.3.is_some());
                assert!(state.4.is_none());
                Ok(())
            })
            .expect("document lifecycle should be readable");
    }
}
