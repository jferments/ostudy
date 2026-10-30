use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rusqlite::{
    params, params_from_iter,
    types::{Value, ValueRef},
    Connection,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::error::CommandError;

use super::{
    scope::{self, SyncSelection},
    store::{BaselineRecord, NewConflict, SyncStore},
};

#[derive(Clone, Copy)]
pub(crate) struct TableSpec {
    /// What sync calls the rows: their baselines and conflicts are kept under
    /// it. Usually the table's name; a table whose rows belong to different
    /// categories is split into named parts by `where_clause`.
    pub name: &'static str,
    /// The SQLite table.
    pub table: &'static str,
    pub key_columns: &'static [&'static str],
    pub where_clause: Option<&'static str>,
    pub excluded_columns: &'static [&'static str],
}

/// Dependency order is deliberate: parents appear before children for upserts,
/// and `apply_plans` walks the list in reverse for deletes.
pub(crate) const SYNC_TABLES: &[TableSpec] = &[
    // Subjects hold decks and documents. `SyncSelection` enables them with
    // either category.
    TableSpec::new("document_collection_organization", &["singleton_id"]),
    TableSpec::new("document_collections", &["id"]),
    // Flashcard/study graph.
    TableSpec::new("decks", &["id"]),
    TableSpec::new("tags", &["id"]),
    TableSpec::new("card_templates", &["id"]),
    TableSpec::new("card_template_fields", &["id"]),
    // Card images. Notebook pictures and lesson audio are the other parts of
    // `assets`, and travel with their own categories; pictures attached to a
    // Tutor chat stay on the device that has the chat.
    TableSpec::assets(
        "assets",
        "lifecycle_state IN ('ready', 'deleted')
         AND (id IN (SELECT asset_id FROM note_assets)
              OR id IN (SELECT asset_id FROM card_template_assets)
              OR id IN (SELECT asset_id FROM question_attempt_assets)
              OR (id NOT IN (SELECT asset_id FROM notebook_item_assets)
                  AND id NOT IN (SELECT asset_id FROM speech_segments)
                  AND id NOT IN (SELECT asset_id FROM speech_voices)
                  AND id NOT IN (SELECT asset_id FROM tutor_turn_attachments
                                 WHERE asset_id IS NOT NULL)))",
    ),
    TableSpec::new("notes", &["id"]),
    TableSpec::new("question_generation_evidence", &["note_id"]),
    TableSpec::new("cards", &["id"]),
    TableSpec::new("card_merges", &["retired_card_id"]),
    TableSpec::new("card_question_presentations", &["card_id"]),
    TableSpec::new("card_template_rules", &["id"]),
    TableSpec::new("note_tags", &["note_id", "tag_id"]),
    TableSpec::new("note_assets", &["note_id", "asset_id", "side"]),
    TableSpec::new("card_template_assets", &["template_id", "asset_id"]),
    TableSpec::new("card_schedules", &["card_id"]),
    TableSpec::new("practice_sessions", &["id"]),
    TableSpec::new("practice_session_cards", &["session_id", "card_id"]),
    TableSpec::new("question_attempts", &["instance_id"]),
    TableSpec::new("question_attempt_assets", &["instance_id", "asset_id"]),
    TableSpec::new(
        "practice_session_card_snapshots",
        &["session_id", "card_id"],
    ),
    TableSpec::new(
        "practice_session_card_tag_snapshots",
        &["session_id", "card_id", "tag_id"],
    ),
    TableSpec::new("review_events", &["id"]),
    TableSpec::new("review_event_attributions", &["review_event_id"]),
    TableSpec::new(
        "review_event_tag_attributions",
        &["review_event_id", "tag_id"],
    ),
    TableSpec::new("review_event_scheduler_context", &["review_event_id"]),
    TableSpec::new(
        "study_activity_retained",
        &["operation_id", "wall_date", "wall_hour"],
    ),
    TableSpec::new("question_review_links", &["review_event_id"]),
    TableSpec::new("question_remediation_attempts", &["operation_id"]),
    TableSpec::new("source_citation_snapshots", &["id"]),
    TableSpec::new("deck_source_citations", &["deck_id", "citation_id"]),
    TableSpec::new("note_source_citations", &["note_id"]),
    // Provider metadata is a parent of optional document processing records.
    TableSpec {
        name: "ai_provider_profiles",
        table: "ai_provider_profiles",
        key_columns: &["id"],
        where_clause: None,
        excluded_columns: &["credential_storage", "credential_epoch"],
    },
    TableSpec::new("ai_task_defaults", &["task_kind"]),
    // Canonical Documents + durable LLM processing/checkpoint graph.
    TableSpec::new("documents", &["id"]),
    TableSpec::new("document_study_ranges", &["id"]),
    TableSpec::new("study_targets", &["id"]),
    TableSpec::new(
        "study_target_suppressions",
        &["target_id", "artifact_kind"],
    ),
    TableSpec::new("document_source_files", &["document_id", "file_ordinal"]),
    TableSpec::new("document_source_units", &["document_id", "unit_number"]),
    TableSpec::new("document_source_blocks", &["document_id", "block_index"]),
    TableSpec::new("document_source_assets", &["document_id", "asset_id"]),
    TableSpec::new("document_pages", &["document_id", "page_number"]),
    TableSpec::new(
        "document_semantic_elements",
        &["document_id", "element_index"],
    ),
    TableSpec::new("document_semantic_chunks", &["document_id", "chunk_index"]),
    TableSpec::new(
        "document_semantic_chunk_sources",
        &["document_id", "chunk_index"],
    ),
    TableSpec::new("document_outlines", &["document_id"]),
    TableSpec::new(
        "document_outline_checkpoints",
        &["document_id", "contract_key", "window_index"],
    ),
    TableSpec::new("document_generation_jobs", &["id"]),
    TableSpec::new("question_generation_reports", &["job_id"]),
    TableSpec::new("document_generation_units", &["job_id", "page_number"]),
    TableSpec::new("document_generation_windows", &["job_id", "window_index"]),
    TableSpec::new(
        "document_generation_checkpoints",
        &["document_id", "contract_key", "window_index"],
    ),
    TableSpec::new("document_learning_plans", &["id"]),
    TableSpec::new("document_learning_topics", &["id"]),
    TableSpec::new("document_learning_plan_topics", &["plan_id", "topic_id"]),
    TableSpec::new("document_artifact_jobs", &["id"]),
    TableSpec::new("document_artifact_metadata", &["document_id"]),
    TableSpec::new("document_annotations", &["id"]),
    TableSpec::new("document_figure_scans", &["document_id"]),
    TableSpec::new("document_figures", &["document_id", "figure_id"]),
    TableSpec::new(
        "document_figure_passages",
        &["document_id", "figure_id", "ordinal"],
    ),
    TableSpec::new("document_figure_readings", &["document_id", "figure_id"]),
    // Study Guide artifact graph. `SyncSelection` only enables these when the
    // Study Guides category is selected (which itself requires Documents).
    TableSpec::new("document_study_guides", &["id"]),
    TableSpec::new("document_study_guide_sections", &["guide_id", "topic_id"]),
    TableSpec::new("document_study_guide_overlays", &["id"]),
    TableSpec::new(
        "document_study_guide_checkpoints",
        &["document_id", "contract_key", "topic_id"],
    ),
    TableSpec::new("study_target_guides", &["id"]),
    TableSpec::new("study_target_guide_overlays", &["id"]),
    TableSpec::new(
        "document_study_guide_section_reads",
        &["document_id", "topic_id"],
    ),
    TableSpec::new("document_study_guide_positions", &["document_id"]),
    TableSpec::part(
        "guide_ink",
        "material_ink",
        &["id"],
        "document_id IS NOT NULL",
    ),
    // Notebooks.
    TableSpec::new("notebooks", &["id"]),
    TableSpec::new("notebook_pages", &["id"]),
    TableSpec::new("notebook_boards", &["page_id"]),
    TableSpec::new("notebook_items", &["id"]),
    TableSpec::assets(
        "notebook_pictures",
        "lifecycle_state IN ('ready', 'deleted')
         AND id IN (SELECT asset_id FROM notebook_item_assets)
         AND id NOT IN (SELECT asset_id FROM note_assets)
         AND id NOT IN (SELECT asset_id FROM card_template_assets)
         AND id NOT IN (SELECT asset_id FROM question_attempt_assets)",
    ),
    TableSpec::new("notebook_item_assets", &["item_id"]),
    // Lessons and what is made from them.
    TableSpec::new("lessons", &["id"]),
    TableSpec::new("lesson_revisions", &["lesson_id", "revision_number"]),
    TableSpec::new(
        "lesson_revision_units",
        &["lesson_id", "revision_number", "unit_id"],
    ),
    TableSpec::new("lesson_progress", &["lesson_id"]),
    TableSpec::new("lecture_scripts", &["lesson_id", "revision_number"]),
    TableSpec::new("lesson_voices", &["lesson_id"]),
    TableSpec::new("podcasts", &["id"]),
    TableSpec::assets(
        "lesson_audio",
        "lifecycle_state IN ('ready', 'deleted')
         AND (id IN (SELECT asset_id FROM speech_segments)
              OR id IN (SELECT asset_id FROM speech_voices))
         AND id NOT IN (SELECT asset_id FROM note_assets)
         AND id NOT IN (SELECT asset_id FROM card_template_assets)
         AND id NOT IN (SELECT asset_id FROM question_attempt_assets)",
    ),
    TableSpec::new("speech_segments", &["id"]),
    TableSpec::new("speech_voices", &["id"]),
    TableSpec::part("slide_ink", "material_ink", &["id"], "lesson_id IS NOT NULL"),
    // Cross-domain provenance is only enabled when both Documents and
    // Flashcards are selected.
    TableSpec::new("document_decks", &["document_id", "deck_id"]),
    TableSpec::new("document_note_topics", &["note_id", "topic_id"]),
    TableSpec::new(
        "study_target_card_origins",
        &["target_id", "target_note_id"],
    ),
    TableSpec::new("note_annotation_sources", &["note_id"]),
];

impl TableSpec {
    const fn new(name: &'static str, key_columns: &'static [&'static str]) -> Self {
        Self {
            name,
            table: name,
            key_columns,
            where_clause: None,
            excluded_columns: &[],
        }
    }

    /// The rows of `table` that `where_clause` picks, synchronized as `name`.
    const fn part(
        name: &'static str,
        table: &'static str,
        key_columns: &'static [&'static str],
        where_clause: &'static str,
    ) -> Self {
        Self {
            name,
            table,
            key_columns,
            where_clause: Some(where_clause),
            excluded_columns: &[],
        }
    }

    /// A part of the managed assets, without the columns that only describe
    /// this device's copy of the file.
    const fn assets(name: &'static str, where_clause: &'static str) -> Self {
        Self {
            name,
            table: "assets",
            key_columns: &["id"],
            where_clause: Some(where_clause),
            excluded_columns: &[
                "draft_id",
                "staged_relative_path",
                "quarantine_relative_path",
                "ready_at",
                "verified_at",
                "lifecycle_reason",
            ],
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub(crate) enum CellValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(String),
}

pub(crate) type RowPayload = BTreeMap<String, CellValue>;
type TableRows = BTreeMap<String, RowPayload>;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MergeSummary {
    pub changed_row_count: usize,
    pub conflict_count: usize,
    pub conflict_ids: Vec<String>,
}

struct PlannedConflict {
    table_name: &'static str,
    row_key: String,
    left_payload_json: Option<String>,
    right_payload_json: Option<String>,
    selected_payload_json: Option<String>,
    conflicting_fields: Vec<String>,
}

struct TablePlan {
    spec: &'static TableSpec,
    local: TableRows,
    merged: BTreeMap<String, Option<RowPayload>>,
    conflicts: Vec<PlannedConflict>,
    /// Notes that one device deleted while the other changed their content.
    deleted_while_edited: Vec<String>,
}

/// An entry for the "Check these" list (release schema 6) about a choice the
/// merge settled on its own.
#[derive(Debug, PartialEq)]
struct SettledFiling {
    entity_kind: &'static str,
    entity_id: String,
    reason: &'static str,
    detail: serde_json::Value,
}

pub(crate) fn merge_remote_database(
    local: &mut Connection,
    remote: &Connection,
    store: &SyncStore,
    local_device_id: &str,
    remote_device_id: &str,
    selection: SyncSelection,
) -> Result<MergeSummary, CommandError> {
    let selection = selection.validate()?;
    let mut plans = Vec::with_capacity(SYNC_TABLES.len());
    for spec in SYNC_TABLES {
        if !selection.includes_table(spec.name)
            || !tables_exist(local, spec)?
            || !tables_exist(remote, spec)?
        {
            continue;
        }
        let local_rows = load_table(local, spec)?;
        let remote_rows = load_table(remote, spec)?;
        let baselines = store.load_baselines(remote_device_id, spec.name)?;
        plans.push(plan_three_way_table(
            spec,
            local_rows,
            remote_rows,
            baselines,
            local_device_id,
            remote_device_id,
        )?);
    }

    let refiled = refile_from_deleted_subjects(&mut plans)?;
    keep_own_narration(&mut plans);
    follow_deleted_parents(&mut plans)?;
    let mut settled = settled_filings(&plans)?;
    // A deleted subject explains a refiled item better than the conflict does.
    settled.extend(refiled);
    let changed_row_count = apply_plans(local, &plans, selection.flashcards, &settled)?;
    let mut conflict_ids = Vec::new();
    for plan in &plans {
        for conflict in &plan.conflicts {
            conflict_ids.push(store.record_conflict(NewConflict {
                peer_id: remote_device_id,
                table_name: conflict.table_name,
                row_key: &conflict.row_key,
                left_device_id: local_device_id,
                left_payload_json: conflict.left_payload_json.as_deref(),
                right_device_id: remote_device_id,
                right_payload_json: conflict.right_payload_json.as_deref(),
                selected_payload_json: conflict.selected_payload_json.as_deref(),
                conflicting_fields: &conflict.conflicting_fields,
            })?);
        }
    }
    replace_baselines(store, remote_device_id, &plans)?;
    Ok(MergeSummary {
        changed_row_count,
        conflict_count: conflict_ids.len(),
        conflict_ids,
    })
}

pub(crate) fn remember_sent_database(
    snapshot: &Connection,
    store: &SyncStore,
    peer_id: &str,
    selection: SyncSelection,
) -> Result<(), CommandError> {
    let selection = selection.validate()?;
    for spec in SYNC_TABLES {
        if !selection.includes_table(spec.name) || !tables_exist(snapshot, spec)? {
            continue;
        }
        let rows = load_table(snapshot, spec)?;
        let mut baseline_rows = Vec::with_capacity(rows.len());
        for (key, payload) in rows {
            baseline_rows.push((
                key,
                Some(serde_json::to_string(&payload).map_err(CommandError::serialization)?),
                payload_hash(Some(&payload))?,
            ));
        }
        store.replace_table_baselines(peer_id, spec.name, &baseline_rows)?;
    }
    Ok(())
}

pub(crate) fn apply_conflict_payload(
    connection: &mut Connection,
    table_name: &str,
    row_key: &str,
    payload_json: Option<&str>,
) -> Result<(), CommandError> {
    let spec = table_spec(table_name).ok_or_else(|| {
        CommandError::new(
            "device_sync_table_unsupported",
            "That synchronization entity is not supported by this build.",
        )
    })?;
    let transaction = connection.transaction().map_err(CommandError::database)?;
    match payload_json {
        Some(json) => {
            let payload: RowPayload = serde_json::from_str(json).map_err(|error| {
                CommandError::new(
                    "device_sync_conflict_invalid",
                    format!("The saved conflict payload is invalid: {error}"),
                )
            })?;
            upsert_row(&transaction, spec, &payload)?;
        }
        None => delete_row(&transaction, spec, row_key)?,
    }
    verify_foreign_keys(&transaction)?;
    scope::validate_question_snapshot(&transaction)?;
    transaction.commit().map_err(CommandError::database)
}

pub(crate) fn table_spec(name: &str) -> Option<&'static TableSpec> {
    SYNC_TABLES.iter().find(|spec| spec.name == name)
}

pub(crate) fn payload_hash(payload: Option<&RowPayload>) -> Result<String, CommandError> {
    let json = payload_json(payload)?;
    Ok(format!("{:x}", Sha256::digest(json.as_bytes())))
}

pub(crate) fn payload_json(payload: Option<&RowPayload>) -> Result<String, CommandError> {
    serde_json::to_string(&payload).map_err(CommandError::serialization)
}

fn plan_three_way_table(
    spec: &'static TableSpec,
    local: TableRows,
    remote: TableRows,
    baselines: HashMap<String, BaselineRecord>,
    local_device_id: &str,
    remote_device_id: &str,
) -> Result<TablePlan, CommandError> {
    let mut keys = BTreeSet::new();
    keys.extend(local.keys().cloned());
    keys.extend(remote.keys().cloned());
    keys.extend(baselines.keys().cloned());
    let mut merged = BTreeMap::new();
    let mut conflicts = Vec::new();
    let mut deleted_while_edited = Vec::new();

    for key in keys {
        let left = local.get(&key);
        let right = remote.get(&key);
        let base = if let Some(record) = baselines.get(&key) {
            let parsed = record
                .payload_json
                .as_deref()
                .map(serde_json::from_str::<RowPayload>)
                .transpose()
                .map_err(|error| {
                    CommandError::new(
                        "device_sync_baseline_invalid",
                        format!("A saved synchronization baseline is invalid: {error}"),
                    )
                })?;
            if payload_hash(parsed.as_ref())? != record.payload_hash {
                return Err(CommandError::new(
                    "device_sync_baseline_invalid",
                    "A saved synchronization baseline failed its content hash check.",
                ));
            }
            parsed
        } else {
            None
        };
        if spec.name == "notes" {
            if let (Some(left), Some(right)) = (left, right) {
                if deleted_on_one_side_and_edited(left, right) {
                    deleted_while_edited.push(key.clone());
                }
            }
        }
        let (selected, conflicting_fields) = if atomic_question_row(spec.name) {
            merge_atomic_rows(
                base.as_ref(),
                left,
                right,
                local_device_id,
                remote_device_id,
            )
        } else if spec.name == "card_schedules" {
            merge_schedule_rows(
                base.as_ref(),
                left,
                right,
                local_device_id,
                remote_device_id,
            )
        } else {
            merge_rows(
                base.as_ref(),
                left,
                right,
                spec.key_columns,
                local_device_id,
                remote_device_id,
            )
        };
        if !conflicting_fields.is_empty() {
            if spec.table == "assets" {
                return Err(CommandError::new(
                    "device_sync_asset_conflict",
                    "Two devices contain different bytes or metadata for the same managed image identity.",
                ));
            }
            conflicts.push(PlannedConflict {
                table_name: spec.name,
                row_key: key.clone(),
                left_payload_json: optional_payload_json(left)?,
                right_payload_json: optional_payload_json(right)?,
                selected_payload_json: optional_payload_json(selected.as_ref())?,
                conflicting_fields,
            });
        }
        merged.insert(key, selected);
    }

    Ok(TablePlan {
        spec,
        local,
        merged,
        conflicts,
        deleted_while_edited,
    })
}

fn atomic_question_row(table: &str) -> bool {
    matches!(
        table,
        "card_question_presentations"
            | "question_attempts"
            | "question_review_links"
            | "question_remediation_attempts"
            | "question_generation_evidence"
    )
}

/// Question checkpoints and evidence receipts contain columns that validate as a
/// unit. Concurrent edits select one whole row and record a conflict rather than
/// field-merging revision/phase/checkpoint fragments into an impossible hybrid.
fn merge_atomic_rows(
    base: Option<&RowPayload>,
    left: Option<&RowPayload>,
    right: Option<&RowPayload>,
    left_device_id: &str,
    right_device_id: &str,
) -> (Option<RowPayload>, Vec<String>) {
    merge_whole_rows(base, left, right, left_device_id <= right_device_id)
}

/// A schedule is one FSRS state, so concurrent changes keep the whole row that
/// was studied or reset last (then changed last) instead of mixing the fields
/// of two states. The other row stays available as a conflict.
fn merge_schedule_rows(
    base: Option<&RowPayload>,
    left: Option<&RowPayload>,
    right: Option<&RowPayload>,
    left_device_id: &str,
    right_device_id: &str,
) -> (Option<RowPayload>, Vec<String>) {
    let recency = |row: &RowPayload| {
        ["last_review_at", "updated_at", "revision"].map(|column| match row.get(column) {
            Some(CellValue::Integer(value)) => *value,
            _ => i64::MIN,
        })
    };
    let prefer_left = match (left, right) {
        (Some(left), Some(right)) => match recency(left).cmp(&recency(right)) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => left_device_id <= right_device_id,
        },
        _ => left_device_id <= right_device_id,
    };
    merge_whole_rows(base, left, right, prefer_left)
}

fn merge_whole_rows(
    base: Option<&RowPayload>,
    left: Option<&RowPayload>,
    right: Option<&RowPayload>,
    prefer_left: bool,
) -> (Option<RowPayload>, Vec<String>) {
    if left == right {
        return (left.cloned(), Vec::new());
    }
    if base == left {
        return (right.cloned(), Vec::new());
    }
    if base == right {
        return (left.cloned(), Vec::new());
    }
    match (left, right) {
        (None, None) => (None, Vec::new()),
        (Some(payload), None) | (None, Some(payload)) if base.is_none() => {
            (Some(payload.clone()), Vec::new())
        }
        (Some(payload), None) | (None, Some(payload)) => {
            (Some(payload.clone()), vec!["$row".to_string()])
        }
        (Some(left), Some(right)) => (
            Some(if prefer_left {
                left.clone()
            } else {
                right.clone()
            }),
            vec!["$row".to_string()],
        ),
    }
}

/// True when one device deleted a note that the other device still has with
/// different content. Placement, deletion and revision bookkeeping is not
/// content: retiring a collection copy changes only those.
fn deleted_on_one_side_and_edited(left: &RowPayload, right: &RowPayload) -> bool {
    const BOOKKEEPING: [&str; 4] = ["deck_id", "deleted_at", "revision", "updated_at"];
    let live = |row: &RowPayload| matches!(row.get("deleted_at"), None | Some(CellValue::Null));
    live(left) != live(right)
        && left
            .keys()
            .chain(right.keys())
            .filter(|column| !BOOKKEEPING.contains(&column.as_str()))
            .any(|column| left.get(column) != right.get(column))
}

/// The choices a merge settles on its own that the learner should check: a
/// deck or document the two devices filed in different subjects, and a
/// collection copy one device edited while the other retired it into its
/// original. The copy stays retired, with the edited text, so the learner can
/// find the edit.
fn settled_filings(plans: &[TablePlan]) -> Result<Vec<SettledFiling>, CommandError> {
    let mut settled = Vec::new();
    for plan in plans {
        let entity_kind = match plan.spec.name {
            "decks" => "deck",
            "documents" => "document",
            _ => continue,
        };
        for conflict in &plan.conflicts {
            if !conflict
                .conflicting_fields
                .iter()
                .any(|field| field == "subject_id")
            {
                continue;
            }
            let subject = |payload: &Option<String>| -> Result<serde_json::Value, CommandError> {
                let row = payload
                    .as_deref()
                    .map(serde_json::from_str::<RowPayload>)
                    .transpose()
                    .map_err(CommandError::serialization)?;
                Ok(match row.as_ref().and_then(|row| row.get("subject_id")) {
                    Some(CellValue::Text(subject_id)) => json!(subject_id),
                    _ => serde_json::Value::Null,
                })
            };
            settled.push(SettledFiling {
                entity_kind,
                entity_id: key_text(&conflict.row_key)?,
                reason: "sync_filing_conflict",
                detail: json!({
                    "subjectIds": [
                        subject(&conflict.left_payload_json)?,
                        subject(&conflict.right_payload_json)?,
                    ],
                }),
            });
        }
    }

    let plan = |name: &str| plans.iter().find(|plan| plan.spec.name == name);
    if let (Some(notes), Some(merges)) = (plan("notes"), plan("card_merges")) {
        let originals = merges
            .merged
            .values()
            .flatten()
            .filter_map(|row| match (row.get("retired_note_id"), row.get("surviving_note_id")) {
                (Some(CellValue::Text(retired)), Some(CellValue::Text(original))) => {
                    Some((retired.clone(), original.clone()))
                }
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        for key in &notes.deleted_while_edited {
            let note_id = key_text(key)?;
            if let Some(original) = originals.get(&note_id) {
                settled.push(SettledFiling {
                    entity_kind: "note",
                    entity_id: note_id,
                    reason: "sync_edited_retired_copy",
                    detail: json!({ "originalNoteId": original }),
                });
            }
        }
    }
    Ok(settled)
}

/// A deck or document that one device filed in a subject the other device
/// deleted would point at a subject that no longer exists. It goes to No
/// subject, where deleting the subject sent the decks and documents it already
/// held, and on the "Check these" list. Both devices merge to the same subjects
/// and filings, so both refile it the same way.
fn refile_from_deleted_subjects(
    plans: &mut [TablePlan],
) -> Result<Vec<SettledFiling>, CommandError> {
    let Some(subjects) = plans
        .iter()
        .find(|plan| plan.spec.name == "document_collections")
    else {
        return Ok(Vec::new());
    };
    let kept = subjects
        .merged
        .iter()
        .filter(|(_, payload)| payload.is_some())
        .map(|(key, _)| key_text(key))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut refiled = Vec::new();
    for plan in plans.iter_mut() {
        let entity_kind = match plan.spec.name {
            "decks" => "deck",
            "documents" => "document",
            _ => continue,
        };
        for (key, payload) in &mut plan.merged {
            let Some(row) = payload.as_mut() else {
                continue;
            };
            let deleted = match row.get("subject_id") {
                Some(CellValue::Text(subject_id)) if !kept.contains(subject_id) => {
                    subject_id.clone()
                }
                _ => continue,
            };
            row.insert("subject_id".to_string(), CellValue::Null);
            // A conflict on the same row records what the merge kept.
            for conflict in plan
                .conflicts
                .iter_mut()
                .filter(|conflict| &conflict.row_key == key)
            {
                conflict.selected_payload_json = optional_payload_json(Some(&*row))?;
            }
            refiled.push(SettledFiling {
                entity_kind,
                entity_id: key_text(key)?,
                reason: "sync_filing_conflict",
                detail: json!({ "deletedSubjectId": deleted }),
            });
        }
    }
    Ok(refiled)
}

/// What happens to a row whose parent the merge removed.
#[derive(Clone, Copy)]
enum OnParentDeleted {
    /// The row goes too, as SQLite's `ON DELETE CASCADE` would remove it.
    Delete,
    /// The row stays and stops naming the parent (`ON DELETE SET NULL`).
    Forget,
}

/// Rows that name a parent row in another synchronized table, in the order the
/// merge settles them: a parent before its children. `columns` is the child's
/// columns that hold the parent's key, in the order of the parent's key.
const PARENTS: &[(&str, &[&str], &str, OnParentDeleted)] = &[
    ("document_annotations", &["document_id"], "documents", OnParentDeleted::Delete),
    ("document_figure_scans", &["document_id"], "documents", OnParentDeleted::Delete),
    ("document_figures", &["document_id"], "documents", OnParentDeleted::Delete),
    (
        "document_figure_passages",
        &["document_id", "figure_id"],
        "document_figures",
        OnParentDeleted::Delete,
    ),
    (
        "document_figure_readings",
        &["document_id", "figure_id"],
        "document_figures",
        OnParentDeleted::Delete,
    ),
    ("document_study_guide_section_reads", &["document_id"], "documents", OnParentDeleted::Delete),
    (
        "document_study_guide_section_reads",
        &["topic_id"],
        "document_learning_topics",
        OnParentDeleted::Delete,
    ),
    ("document_study_guide_positions", &["document_id"], "documents", OnParentDeleted::Delete),
    (
        "document_study_guide_positions",
        &["topic_id"],
        "document_learning_topics",
        OnParentDeleted::Delete,
    ),
    ("guide_ink", &["document_id"], "documents", OnParentDeleted::Delete),
    ("notebooks", &["subject_id"], "document_collections", OnParentDeleted::Forget),
    ("notebook_pages", &["notebook_id"], "notebooks", OnParentDeleted::Delete),
    ("notebook_boards", &["page_id"], "notebook_pages", OnParentDeleted::Delete),
    ("notebook_items", &["page_id"], "notebook_pages", OnParentDeleted::Delete),
    ("notebook_item_assets", &["item_id"], "notebook_items", OnParentDeleted::Delete),
    ("lessons", &["subject_id"], "document_collections", OnParentDeleted::Delete),
    ("lesson_revisions", &["lesson_id"], "lessons", OnParentDeleted::Delete),
    (
        "lesson_revision_units",
        &["lesson_id", "revision_number"],
        "lesson_revisions",
        OnParentDeleted::Delete,
    ),
    ("lesson_progress", &["lesson_id"], "lessons", OnParentDeleted::Delete),
    (
        "lesson_progress",
        &["lesson_id", "revision_number"],
        "lesson_revisions",
        OnParentDeleted::Delete,
    ),
    (
        "lecture_scripts",
        &["lesson_id", "revision_number"],
        "lesson_revisions",
        OnParentDeleted::Delete,
    ),
    ("lesson_voices", &["lesson_id"], "lessons", OnParentDeleted::Delete),
    ("podcasts", &["subject_id"], "document_collections", OnParentDeleted::Delete),
    ("slide_ink", &["lesson_id"], "lessons", OnParentDeleted::Delete),
    ("note_annotation_sources", &["note_id"], "notes", OnParentDeleted::Delete),
    ("note_annotation_sources", &["document_id"], "documents", OnParentDeleted::Forget),
];

/// A mark, page, lesson or other row that one device added or changed while
/// the other deleted its parent would name a parent that no longer exists.
/// It follows its parent the way deleting the parent on this device would
/// have treated it: removed with it, or kept without the reference. Both
/// devices merge to the same rows, so both settle it the same way, and a
/// conflict about a removed row is dropped since nothing is left to choose.
fn follow_deleted_parents(plans: &mut [TablePlan]) -> Result<(), CommandError> {
    for (child, columns, parent, on_deleted) in PARENTS {
        let Some(kept) = plans
            .iter()
            .find(|plan| plan.spec.name == *parent)
            .map(|plan| {
                plan.merged
                    .iter()
                    .filter(|(_, payload)| payload.is_some())
                    .map(|(key, _)| key.clone())
                    .collect::<BTreeSet<_>>()
            })
        else {
            continue;
        };
        let Some(plan) = plans.iter_mut().find(|plan| plan.spec.name == *child) else {
            continue;
        };
        let mut removed = BTreeSet::new();
        for (key, payload) in &mut plan.merged {
            let Some(row) = payload.as_mut() else {
                continue;
            };
            let cells = columns
                .iter()
                .map(|column| row.get(*column).cloned().unwrap_or(CellValue::Null))
                .collect::<Vec<_>>();
            if cells.iter().any(|cell| matches!(cell, CellValue::Null)) {
                continue;
            }
            let parent_key = serde_json::to_string(&cells).map_err(CommandError::serialization)?;
            if kept.contains(&parent_key) {
                continue;
            }
            match on_deleted {
                OnParentDeleted::Delete => {
                    *payload = None;
                    removed.insert(key.clone());
                }
                OnParentDeleted::Forget => {
                    for column in *columns {
                        row.insert((*column).to_string(), CellValue::Null);
                    }
                    for conflict in plan
                        .conflicts
                        .iter_mut()
                        .filter(|conflict| &conflict.row_key == key)
                    {
                        conflict.selected_payload_json = optional_payload_json(Some(&*row))?;
                    }
                }
            }
        }
        plan.conflicts
            .retain(|conflict| !removed.contains(&conflict.row_key));
    }
    Ok(())
}

/// Narration audio is kept once per text and voice. When both devices spoke
/// the same text in the same voice, each keeps its own recording: the other
/// device's copy would break that rule, and the two sound the same.
fn keep_own_narration(plans: &mut [TablePlan]) {
    const SPOKEN_AS: [&str; 5] = ["text_sha256", "engine", "model_id", "voice", "format"];
    let Some(segments) = plans
        .iter_mut()
        .find(|plan| plan.spec.name == "speech_segments")
    else {
        return;
    };
    let spoken = |row: &RowPayload| {
        SPOKEN_AS
            .iter()
            .map(|column| row.get(*column).cloned())
            .collect::<Vec<_>>()
    };
    let local_spoken = segments
        .local
        .values()
        .map(|row| serde_json::to_string(&spoken(row)).unwrap_or_default())
        .collect::<BTreeSet<_>>();
    let mut dropped_assets = BTreeSet::new();
    for (key, payload) in &mut segments.merged {
        if segments.local.contains_key(key) {
            continue;
        }
        let Some(row) = payload.as_ref() else {
            continue;
        };
        if !local_spoken.contains(&serde_json::to_string(&spoken(row)).unwrap_or_default()) {
            continue;
        }
        if let Some(asset_id) = row.get("asset_id") {
            if let Ok(asset_key) = serde_json::to_string(&[asset_id]) {
                dropped_assets.insert(asset_key);
            }
        }
        *payload = None;
    }
    if let Some(audio) = plans.iter_mut().find(|plan| plan.spec.name == "lesson_audio") {
        for key in dropped_assets {
            if !audio.local.contains_key(&key) {
                audio.merged.insert(key, None);
            }
        }
    }
}

/// The single text value of a one-column row key.
fn key_text(row_key: &str) -> Result<String, CommandError> {
    match serde_json::from_str::<Vec<CellValue>>(row_key)
        .map_err(CommandError::serialization)?
        .as_slice()
    {
        [CellValue::Text(value)] => Ok(value.clone()),
        _ => Err(CommandError::new(
            "device_sync_row_key_invalid",
            "A synchronized row key has the wrong number of fields.",
        )),
    }
}

fn record_settled_filings(
    connection: &Connection,
    settled: &[SettledFiling],
) -> Result<(), CommandError> {
    if settled.is_empty() || !table_exists(connection, "subject_filing_reviews")? {
        return Ok(());
    }
    for filing in settled {
        connection
            .execute(
                "INSERT INTO subject_filing_reviews(entity_kind, entity_id, reason, detail_json)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(entity_kind, entity_id) DO UPDATE SET
                     reason = excluded.reason,
                     detail_json = excluded.detail_json,
                     dismissed_at = NULL",
                params![
                    filing.entity_kind,
                    filing.entity_id,
                    filing.reason,
                    filing.detail.to_string()
                ],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn merge_rows(
    base: Option<&RowPayload>,
    left: Option<&RowPayload>,
    right: Option<&RowPayload>,
    key_columns: &[&str],
    left_device_id: &str,
    right_device_id: &str,
) -> (Option<RowPayload>, Vec<String>) {
    if left == right {
        return (left.cloned(), Vec::new());
    }
    if base == left {
        return (right.cloned(), Vec::new());
    }
    if base == right {
        return (left.cloned(), Vec::new());
    }
    match (left, right) {
        (None, None) => (None, Vec::new()),
        (Some(payload), None) | (None, Some(payload)) if base.is_none() => {
            (Some(payload.clone()), Vec::new())
        }
        (Some(payload), None) | (None, Some(payload)) => {
            (Some(payload.clone()), vec!["$row".to_string()])
        }
        (Some(left), Some(right)) => merge_present_rows(
            base,
            left,
            right,
            key_columns,
            left_device_id,
            right_device_id,
        ),
    }
}

fn merge_present_rows(
    base: Option<&RowPayload>,
    left: &RowPayload,
    right: &RowPayload,
    key_columns: &[&str],
    left_device_id: &str,
    right_device_id: &str,
) -> (Option<RowPayload>, Vec<String>) {
    let prefer_left = left_device_id <= right_device_id;
    let mut columns = BTreeSet::new();
    columns.extend(left.keys().cloned());
    columns.extend(right.keys().cloned());
    if let Some(base) = base {
        columns.extend(base.keys().cloned());
    }
    let mut output = RowPayload::new();
    let mut conflicts = Vec::new();
    for column in columns {
        let left_value = left.get(&column);
        let right_value = right.get(&column);
        let base_value = base.and_then(|payload| payload.get(&column));
        let selected = if left_value == right_value {
            left_value.or(right_value)
        } else if key_columns.contains(&column.as_str()) {
            conflicts.push(column.clone());
            if prefer_left {
                left_value
            } else {
                right_value
            }
        } else {
            let left_changed = left_value != base_value;
            let right_changed = right_value != base_value;
            match (left_changed, right_changed) {
                (false, true) => right_value,
                (true, false) => left_value,
                (false, false) => left_value.or(right_value),
                (true, true) => {
                    conflicts.push(column.clone());
                    if prefer_left {
                        left_value
                    } else {
                        right_value
                    }
                }
            }
        };
        if let Some(value) = selected {
            output.insert(column, value.clone());
        }
    }
    (Some(output), conflicts)
}

fn apply_plans(
    connection: &mut Connection,
    plans: &[TablePlan],
    validate_questions: bool,
    settled: &[SettledFiling],
) -> Result<usize, CommandError> {
    let transaction = connection.transaction().map_err(CommandError::database)?;
    transaction
        .execute_batch("PRAGMA defer_foreign_keys = ON;")
        .map_err(CommandError::database)?;
    let mut changed = 0_usize;
    let mut document_filing_changed = false;

    for plan in plans.iter().rev() {
        for (key, payload) in &plan.merged {
            if payload.is_none() && plan.local.contains_key(key) {
                delete_row(&transaction, plan.spec, key)?;
                changed = changed.saturating_add(1);
                document_filing_changed |=
                    changes_document_filing(plan.spec.name, plan.local.get(key), None);
            }
        }
    }
    for plan in plans {
        for (key, payload) in &plan.merged {
            if let Some(payload) = payload {
                if plan.local.get(key) != Some(payload) {
                    upsert_row(&transaction, plan.spec, payload)?;
                    changed = changed.saturating_add(1);
                    document_filing_changed |= changes_document_filing(
                        plan.spec.name,
                        plan.local.get(key),
                        Some(payload),
                    );
                }
            }
        }
    }
    let now = crate::util::unix_time_millis()?;
    if table_exists(&transaction, "library_organization")? && changed > 0 {
        transaction
            .execute(
                "UPDATE library_organization
                 SET revision = revision + 1, updated_at = ?1
                 WHERE singleton_id = 1",
                [now],
            )
            .map_err(CommandError::database)?;
    }
    // Study materials stamp the document filing revision, so it must move when
    // the merge changes what a subject holds, even if both devices had already
    // advanced it to the same number.
    if document_filing_changed && table_exists(&transaction, "document_collection_organization")?
    {
        transaction
            .execute(
                "UPDATE document_collection_organization
                 SET revision = revision + 1, updated_at = ?1
                 WHERE singleton_id = 1",
                [now],
            )
            .map_err(CommandError::database)?;
    }
    record_settled_filings(&transaction, settled)?;
    verify_foreign_keys(&transaction)?;
    if validate_questions {
        scope::validate_question_snapshot(&transaction)?;
    }
    transaction.commit().map_err(CommandError::database)?;
    Ok(changed)
}

/// Whether replacing `before` with `after` in `table` changes what the document
/// filing revision covers: which documents a subject holds and which of their
/// parts it studies. Deleting a subject counts, as it does in the subjects API.
fn changes_document_filing(
    table: &str,
    before: Option<&RowPayload>,
    after: Option<&RowPayload>,
) -> bool {
    fn subject(row: Option<&RowPayload>) -> Option<&CellValue> {
        row.and_then(|row| row.get("subject_id"))
            .filter(|subject_id| !matches!(subject_id, CellValue::Null))
    }
    match table {
        "documents" => subject(before) != subject(after),
        "document_study_ranges" => true,
        "document_collections" => after.is_none(),
        _ => false,
    }
}

fn replace_baselines(
    store: &SyncStore,
    peer_id: &str,
    plans: &[TablePlan],
) -> Result<(), CommandError> {
    for plan in plans {
        let mut rows = Vec::with_capacity(plan.merged.len());
        for (key, payload) in &plan.merged {
            let payload_json = optional_payload_json(payload.as_ref())?;
            let hash = payload_hash(payload.as_ref())?;
            rows.push((key.clone(), payload_json, hash));
        }
        store.replace_table_baselines(peer_id, plan.spec.name, &rows)?;
    }
    Ok(())
}

fn load_table(connection: &Connection, spec: &TableSpec) -> Result<TableRows, CommandError> {
    let where_sql = spec
        .where_clause
        .map(|clause| format!(" WHERE {clause}"))
        .unwrap_or_default();
    let sql = format!("SELECT * FROM {}{where_sql}", quote_identifier(spec.table)?);
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let columns = statement
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    for key in spec.key_columns {
        if !columns.iter().any(|column| column == key) {
            return Err(CommandError::new(
                "device_sync_schema_mismatch",
                format!(
                    "Table '{}' is missing synchronization key '{key}'.",
                    spec.name
                ),
            ));
        }
    }
    let mut rows = statement.query([]).map_err(CommandError::database)?;
    let mut output = TableRows::new();
    while let Some(row) = rows.next().map_err(CommandError::database)? {
        let mut payload = RowPayload::new();
        for (index, column) in columns.iter().enumerate() {
            if spec.excluded_columns.contains(&column.as_str()) {
                continue;
            }
            payload.insert(
                column.clone(),
                cell_from_ref(row.get_ref(index).map_err(CommandError::database)?)?,
            );
        }
        let key = row_key(spec, &payload)?;
        if output.insert(key, payload).is_some() {
            return Err(CommandError::new(
                "device_sync_schema_mismatch",
                format!(
                    "Table '{}' contains duplicate synchronization keys.",
                    spec.name
                ),
            ));
        }
    }
    Ok(output)
}

fn upsert_row(
    connection: &Connection,
    spec: &TableSpec,
    payload: &RowPayload,
) -> Result<(), CommandError> {
    for key in spec.key_columns {
        if !payload.contains_key(*key) {
            return Err(CommandError::new(
                "device_sync_row_invalid",
                format!("A synchronized '{}' row is missing key '{key}'.", spec.name),
            ));
        }
    }
    let columns = payload.keys().cloned().collect::<Vec<_>>();
    let quoted_columns = columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Result<Vec<_>, _>>()?;
    let placeholders = (1..=columns.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let conflict_columns = spec
        .key_columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let update_columns = columns
        .iter()
        .filter(|column| !spec.key_columns.contains(&column.as_str()))
        .map(|column| {
            let quoted = quote_identifier(column)?;
            Ok(format!("{quoted} = excluded.{quoted}"))
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    let action = if update_columns.is_empty() {
        "DO NOTHING".to_string()
    } else {
        format!("DO UPDATE SET {}", update_columns.join(", "))
    };
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT ({}) {}",
        quote_identifier(spec.table)?,
        quoted_columns.join(", "),
        placeholders,
        conflict_columns,
        action,
    );
    let values = columns
        .iter()
        .map(|column| cell_to_sql(payload.get(column).expect("column came from payload")))
        .collect::<Result<Vec<_>, _>>()?;
    connection
        .execute(&sql, params_from_iter(values))
        .map_err(CommandError::database)?;
    Ok(())
}

fn delete_row(
    connection: &Connection,
    spec: &TableSpec,
    row_key_json: &str,
) -> Result<(), CommandError> {
    let values: Vec<CellValue> = serde_json::from_str(row_key_json).map_err(|error| {
        CommandError::new(
            "device_sync_row_key_invalid",
            format!("A synchronized row key is invalid: {error}"),
        )
    })?;
    if values.len() != spec.key_columns.len() {
        return Err(CommandError::new(
            "device_sync_row_key_invalid",
            "A synchronized row key has the wrong number of fields.",
        ));
    }
    let predicates = spec
        .key_columns
        .iter()
        .enumerate()
        .map(|(index, column)| Ok(format!("{} = ?{}", quote_identifier(column)?, index + 1)))
        .collect::<Result<Vec<_>, CommandError>>()?;
    let sql = format!(
        "DELETE FROM {} WHERE {}",
        quote_identifier(spec.table)?,
        predicates.join(" AND "),
    );
    let parameters = values
        .iter()
        .map(cell_to_sql)
        .collect::<Result<Vec<_>, _>>()?;
    connection
        .execute(&sql, params_from_iter(parameters))
        .map_err(CommandError::database)?;
    Ok(())
}

fn row_key(spec: &TableSpec, payload: &RowPayload) -> Result<String, CommandError> {
    let values = spec
        .key_columns
        .iter()
        .map(|column| {
            payload.get(*column).cloned().ok_or_else(|| {
                CommandError::new(
                    "device_sync_row_invalid",
                    format!(
                        "A synchronized '{}' row is missing key '{column}'.",
                        spec.name
                    ),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    serde_json::to_string(&values).map_err(CommandError::serialization)
}

fn cell_from_ref(value: ValueRef<'_>) -> Result<CellValue, CommandError> {
    match value {
        ValueRef::Null => Ok(CellValue::Null),
        ValueRef::Integer(value) => Ok(CellValue::Integer(value)),
        ValueRef::Real(value) if value.is_finite() => Ok(CellValue::Real(value)),
        ValueRef::Real(_) => Err(CommandError::new(
            "device_sync_row_invalid",
            "A synchronized numeric field is not finite.",
        )),
        ValueRef::Text(value) => std::str::from_utf8(value)
            .map(|value| CellValue::Text(value.to_string()))
            .map_err(|_| {
                CommandError::new(
                    "device_sync_row_invalid",
                    "A synchronized text field is not valid UTF-8.",
                )
            }),
        ValueRef::Blob(value) => Ok(CellValue::Blob(BASE64_STANDARD.encode(value))),
    }
}

fn cell_to_sql(value: &CellValue) -> Result<Value, CommandError> {
    match value {
        CellValue::Null => Ok(Value::Null),
        CellValue::Integer(value) => Ok(Value::Integer(*value)),
        CellValue::Real(value) if value.is_finite() => Ok(Value::Real(*value)),
        CellValue::Real(_) => Err(CommandError::new(
            "device_sync_row_invalid",
            "A synchronized numeric field is not finite.",
        )),
        CellValue::Text(value) => Ok(Value::Text(value.clone())),
        CellValue::Blob(value) => BASE64_STANDARD.decode(value).map(Value::Blob).map_err(|_| {
            CommandError::new(
                "device_sync_row_invalid",
                "A synchronized binary field is not valid base64.",
            )
        }),
    }
}

fn optional_payload_json(payload: Option<&RowPayload>) -> Result<Option<String>, CommandError> {
    payload
        .map(|payload| serde_json::to_string(payload).map_err(CommandError::serialization))
        .transpose()
}

/// Whether a database has the spec's table and every table its
/// `where_clause` reads, which a library from an older release may lack.
fn tables_exist(connection: &Connection, spec: &TableSpec) -> Result<bool, CommandError> {
    if !table_exists(connection, spec.table)? {
        return Ok(false);
    }
    for read in spec
        .where_clause
        .unwrap_or_default()
        .split("FROM ")
        .skip(1)
    {
        let table = read
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .next()
            .unwrap_or_default();
        if !table_exists(connection, table)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool, CommandError> {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = ?1
             )",
            [table_name],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value == 1)
        .map_err(CommandError::database)
}

fn verify_foreign_keys(connection: &Connection) -> Result<(), CommandError> {
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(CommandError::database)?;
    if count == 0 {
        Ok(())
    } else {
        Err(CommandError::new(
            "device_sync_integrity_failed",
            "The synchronized library would violate relational integrity, so no changes were saved.",
        ))
    }
}

fn quote_identifier(identifier: &str) -> Result<String, CommandError> {
    if identifier.is_empty()
        || !identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(CommandError::new(
            "device_sync_schema_mismatch",
            "A synchronized SQLite identifier is invalid.",
        ));
    }
    Ok(format!("\"{identifier}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entries: &[(&str, CellValue)]) -> RowPayload {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect()
    }

    #[test]
    fn disjoint_field_edits_merge_without_conflict() {
        let base = row(&[
            ("id", CellValue::Text("a".repeat(32))),
            ("name", CellValue::Text("Base".to_string())),
            ("description", CellValue::Text(String::new())),
        ]);
        let mut left = base.clone();
        left.insert("name".to_string(), CellValue::Text("Local".to_string()));
        let mut right = base.clone();
        right.insert(
            "description".to_string(),
            CellValue::Text("Remote description".to_string()),
        );
        let (merged, conflicts) = merge_rows(
            Some(&base),
            Some(&left),
            Some(&right),
            &["id"],
            &"1".repeat(32),
            &"2".repeat(32),
        );
        let merged = merged.expect("row should survive");
        assert!(conflicts.is_empty());
        assert_eq!(merged.get("name"), left.get("name"));
        assert_eq!(merged.get("description"), right.get("description"));
    }

    #[test]
    fn first_contact_remote_creation_is_not_a_delete_conflict() {
        let remote = row(&[
            ("id", CellValue::Text("a".repeat(32))),
            ("name", CellValue::Text("From sender".to_string())),
        ]);
        let (merged, conflicts) = merge_rows(
            None,
            None,
            Some(&remote),
            &["id"],
            &"1".repeat(32),
            &"2".repeat(32),
        );
        assert!(conflicts.is_empty());
        assert_eq!(merged.as_ref(), Some(&remote));
    }

    #[test]
    fn first_contact_local_creation_is_preserved_without_conflict() {
        let local = row(&[
            ("id", CellValue::Text("a".repeat(32))),
            ("name", CellValue::Text("Already on receiver".to_string())),
        ]);
        let (merged, conflicts) = merge_rows(
            None,
            Some(&local),
            None,
            &["id"],
            &"1".repeat(32),
            &"2".repeat(32),
        );
        assert!(conflicts.is_empty());
        assert_eq!(merged.as_ref(), Some(&local));
    }

    #[test]
    fn same_field_edits_create_deterministic_conflict() {
        let base = row(&[
            ("id", CellValue::Text("a".repeat(32))),
            ("name", CellValue::Text("Base".to_string())),
        ]);
        let mut left = base.clone();
        left.insert("name".to_string(), CellValue::Text("Local".to_string()));
        let mut right = base.clone();
        right.insert("name".to_string(), CellValue::Text("Remote".to_string()));
        let (merged, conflicts) = merge_rows(
            Some(&base),
            Some(&left),
            Some(&right),
            &["id"],
            &"1".repeat(32),
            &"2".repeat(32),
        );
        assert_eq!(conflicts, vec!["name"]);
        assert_eq!(
            merged.expect("row should survive").get("name"),
            left.get("name")
        );
    }

    #[test]
    fn delete_versus_edit_preserves_the_row_for_resolution() {
        let base = row(&[
            ("id", CellValue::Text("a".repeat(32))),
            ("name", CellValue::Text("Base".to_string())),
        ]);
        let mut edited = base.clone();
        edited.insert("name".to_string(), CellValue::Text("Edited".to_string()));
        let (merged, conflicts) = merge_rows(
            Some(&base),
            None,
            Some(&edited),
            &["id"],
            &"1".repeat(32),
            &"2".repeat(32),
        );
        assert!(merged.is_some());
        assert_eq!(conflicts, vec!["$row"]);
    }

    #[test]
    fn question_checkpoint_conflicts_select_one_complete_row() {
        let base = row(&[
            ("instance_id", CellValue::Text("a".repeat(32))),
            ("revision", CellValue::Integer(1)),
            ("phase", CellValue::Text("answering".to_string())),
            ("checkpoint_json", CellValue::Text("base".to_string())),
        ]);
        let mut left = base.clone();
        left.insert("revision".into(), CellValue::Integer(2));
        left.insert("checkpoint_json".into(), CellValue::Text("left".into()));
        let mut right = base.clone();
        right.insert("phase".into(), CellValue::Text("checked".into()));
        right.insert("checkpoint_json".into(), CellValue::Text("right".into()));
        let (merged, conflicts) = merge_atomic_rows(
            Some(&base),
            Some(&left),
            Some(&right),
            &"1".repeat(32),
            &"2".repeat(32),
        );
        assert_eq!(conflicts, vec!["$row"]);
        assert_eq!(merged, Some(left));
    }

    #[test]
    fn document_and_study_guide_tables_are_scope_gated() {
        let docs = SyncSelection {
            flashcards: false,
            documents: true,
            study_guides: false,
            notes: false,
            lessons: false,
            api_settings: false,
            api_credentials: false,
        };
        assert!(docs.includes_table("documents"));
        assert!(docs.includes_table("question_generation_reports"));
        assert!(docs.includes_table("document_generation_checkpoints"));
        assert!(!docs.includes_table("document_study_guides"));
        assert!(!docs.includes_table("document_note_topics"));
    }
}
