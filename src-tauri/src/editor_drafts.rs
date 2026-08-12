//! Durable, device-local recovery for incomplete authoring workflows.
//!
//! Draft payloads are typed by `draft_kind`, versioned, bounded, and checked for
//! sensitive fields. They are not canonical notes/templates/profiles and never
//! bypass the final domain validator. Card/template drafts own a persistent
//! managed asset draft so image bytes survive WebView or process loss.

use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{
    asset_lifecycle,
    database::Database,
    error::CommandError,
    paths::AppPaths,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

const PAYLOAD_SCHEMA_VERSION: i64 = 1;
const MAX_DRAFT_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
const MAX_DRAFT_DEPTH: usize = 48;
const MAX_DRAFT_COLLECTION_ITEMS: usize = 2_000;
const MAX_DRAFT_STRING_CHARS: usize = 1_000_000;
const MAX_BULK_ROWS: usize = 500;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EditorDraftKind {
    Card,
    Template,
    BulkCards,
    AiProfile,
}

impl EditorDraftKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Card => "card",
            Self::Template => "template",
            Self::BulkCards => "bulk_cards",
            Self::AiProfile => "ai_profile",
        }
    }

    fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "card" => Ok(Self::Card),
            "template" => Ok(Self::Template),
            "bulk_cards" => Ok(Self::BulkCards),
            "ai_profile" => Ok(Self::AiProfile),
            _ => Err(CommandError::new(
                "invalid_editor_draft_kind",
                "The saved editor draft kind is invalid.",
            )),
        }
    }

    fn asset_purpose(self) -> Option<&'static str> {
        match self {
            Self::Card => Some("card"),
            Self::Template => Some("template"),
            Self::BulkCards | Self::AiProfile => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenEditorDraftInput {
    pub operation_id: String,
    pub draft_kind: EditorDraftKind,
    pub target_entity_id: Option<String>,
    pub context_entity_id: Option<String>,
    pub base_revision: Option<i64>,
    #[serde(default = "default_payload_schema_version")]
    pub payload_schema_version: i64,
    pub initial_payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveEditorDraftInput {
    pub draft_id: String,
    pub revision: i64,
    pub payload_schema_version: i64,
    pub payload: Value,
    #[serde(default)]
    pub asset_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscardEditorDraftInput {
    pub draft_id: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditorDraftCommitRef {
    pub draft_id: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommitEditorDraftInput {
    pub draft_id: String,
    pub revision: i64,
    pub target_entity_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorDraftDto {
    pub id: String,
    pub draft_kind: EditorDraftKind,
    pub target_entity_id: Option<String>,
    pub context_entity_id: Option<String>,
    pub base_revision: Option<i64>,
    pub payload_schema_version: i64,
    pub payload: Value,
    pub revision: i64,
    pub state: String,
    pub asset_draft_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_opened_at: i64,
}

fn default_payload_schema_version() -> i64 {
    PAYLOAD_SCHEMA_VERSION
}

#[tauri::command]
pub fn open_editor_draft(
    database: State<'_, Database>,
    input: OpenEditorDraftInput,
) -> Result<EditorDraftDto, CommandError> {
    validate_entity_id(&input.operation_id, "editor draft operation")?;
    validate_optional_id(input.target_entity_id.as_deref(), "editor draft target")?;
    validate_optional_id(input.context_entity_id.as_deref(), "editor draft context")?;
    validate_base_revision(input.base_revision)?;
    validate_payload(
        input.draft_kind,
        input.payload_schema_version,
        &input.initial_payload,
    )?;

    database.with_connection(|connection| {
        if let Some(existing) = load_by_operation(connection, &input.operation_id)? {
            ensure_open_request_matches(&existing, &input)?;
            return touch_draft(connection, &existing.id);
        }
        if let Some(existing) = load_resumable_draft(
            connection,
            input.draft_kind,
            input.target_entity_id.as_deref(),
            input.context_entity_id.as_deref(),
        )? {
            return touch_draft(connection, &existing.id);
        }

        let now = unix_time_millis()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let draft_id = generate_id(&transaction)?;
        let asset_draft_id = if let Some(purpose) = input.draft_kind.asset_purpose() {
            let id = generate_id(&transaction)?;
            transaction
                .execute(
                    "INSERT INTO asset_drafts (
                         id, purpose, owner_id, created_at, updated_at, expires_at,
                         retention_mode, editor_draft_id
                     ) VALUES (?1, ?2, ?3, ?4, ?4, ?5, 'persistent_editor', ?6)",
                    params![id, purpose, input.target_entity_id, now, i64::MAX, draft_id,],
                )
                .map_err(CommandError::database)?;
            Some(id)
        } else {
            None
        };
        let payload_json = canonical_payload_json(&input.initial_payload)?;
        let payload_sha256 = hash_payload(&payload_json);
        transaction
            .execute(
                "INSERT INTO editor_drafts (
                     id, open_operation_id, draft_kind, target_entity_id,
                     context_entity_id, base_revision, payload_schema_version,
                     payload_json, payload_sha256, revision, state, asset_draft_id,
                     created_at, updated_at, last_opened_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, 'active', ?10,
                     ?11, ?11, ?11
                 )",
                params![
                    draft_id,
                    input.operation_id,
                    input.draft_kind.as_str(),
                    input.target_entity_id,
                    input.context_entity_id,
                    input.base_revision,
                    input.payload_schema_version,
                    payload_json,
                    payload_sha256,
                    asset_draft_id,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        transaction.commit().map_err(CommandError::database)?;
        load_draft(connection, &draft_id)
    })
}

#[tauri::command]
pub fn save_editor_draft(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: SaveEditorDraftInput,
) -> Result<EditorDraftDto, CommandError> {
    validate_entity_id(&input.draft_id, "editor draft")?;
    if input.revision < 1 {
        return Err(invalid_draft_revision());
    }
    let retained = normalize_asset_ids(input.asset_ids)?;

    let (draft, cleanup) = database.with_connection(|connection| {
        let current = load_draft(connection, &input.draft_id)?;
        if current.state != "active" {
            return Err(inactive_draft_error(&current.state));
        }
        if current.revision != input.revision {
            return Err(CommandError::new(
                "editor_draft_changed",
                "This recovered draft changed before autosave completed. Reload it and try again.",
            ));
        }
        validate_payload(
            current.draft_kind,
            input.payload_schema_version,
            &input.payload,
        )?;
        if current.asset_draft_id.is_none() && !retained.is_empty() {
            return Err(CommandError::new(
                "editor_draft_assets_not_supported",
                "This draft type cannot contain managed images.",
            ));
        }

        let payload_json = canonical_payload_json(&input.payload)?;
        let payload_sha256 = hash_payload(&payload_json);
        let now = unix_time_millis()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(asset_draft_id) = current.asset_draft_id.as_deref() {
            asset_lifecycle::ensure_assets_available(
                &transaction,
                paths.inner(),
                &retained.iter().cloned().collect::<Vec<_>>(),
                Some(asset_draft_id),
            )?;
        }
        transaction
            .execute(
                "DELETE FROM editor_draft_assets WHERE draft_id = ?1",
                [input.draft_id.as_str()],
            )
            .map_err(CommandError::database)?;
        for asset_id in &retained {
            transaction
                .execute(
                    "INSERT INTO editor_draft_assets (draft_id, asset_id) VALUES (?1, ?2)",
                    params![input.draft_id, asset_id],
                )
                .map_err(CommandError::database)?;
        }
        let cleanup = if let Some(asset_draft_id) = current.asset_draft_id.as_deref() {
            asset_lifecycle::retire_unused_editor_assets(
                &transaction,
                paths.inner(),
                asset_draft_id,
                &retained,
                now,
                "removed from durable editor draft",
            )?
        } else {
            Vec::new()
        };
        let changed = if payload_sha256 == hash_payload(&canonical_payload_json(&current.payload)?)
            && input.payload_schema_version == current.payload_schema_version
        {
            transaction
                .execute(
                    "UPDATE editor_drafts
                     SET last_opened_at = ?1, updated_at = ?1
                     WHERE id = ?2 AND revision = ?3 AND state = 'active'",
                    params![now, input.draft_id, input.revision],
                )
                .map_err(CommandError::database)?
        } else {
            transaction
                .execute(
                    "UPDATE editor_drafts
                     SET payload_schema_version = ?1, payload_json = ?2,
                         payload_sha256 = ?3, revision = revision + 1,
                         updated_at = ?4, last_opened_at = ?4
                     WHERE id = ?5 AND revision = ?6 AND state = 'active'",
                    params![
                        input.payload_schema_version,
                        payload_json,
                        payload_sha256,
                        now,
                        input.draft_id,
                        input.revision,
                    ],
                )
                .map_err(CommandError::database)?
        };
        if changed != 1 {
            return Err(CommandError::new(
                "editor_draft_changed",
                "This recovered draft changed before autosave completed. Reload it and try again.",
            ));
        }
        transaction.commit().map_err(CommandError::database)?;
        Ok((load_draft(connection, &input.draft_id)?, cleanup))
    })?;
    asset_lifecycle::remove_editor_cleanup_paths(&cleanup);
    Ok(draft)
}

#[tauri::command]
pub fn discard_editor_draft(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
    input: DiscardEditorDraftInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.draft_id, "editor draft")?;
    if input.revision < 1 {
        return Err(invalid_draft_revision());
    }
    let cleanup = database.with_connection(|connection| {
        let current = load_draft(connection, &input.draft_id)?;
        if current.state == "discarded" || current.state == "committed" {
            return Ok(Vec::new());
        }
        if current.revision != input.revision {
            return Err(CommandError::new(
                "editor_draft_changed",
                "This recovered draft changed before it could be discarded.",
            ));
        }
        let now = unix_time_millis()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        let cleanup = if let Some(asset_draft_id) = current.asset_draft_id.as_deref() {
            let cleanup = asset_lifecycle::retire_unused_editor_assets(
                &transaction,
                paths.inner(),
                asset_draft_id,
                &BTreeSet::new(),
                now,
                "durable editor draft discarded",
            )?;
            transaction
                .execute(
                    "UPDATE asset_drafts
                     SET canceled_at = COALESCE(canceled_at, ?1), updated_at = ?1
                     WHERE id = ?2 AND committed_at IS NULL AND canceled_at IS NULL",
                    params![now, asset_draft_id],
                )
                .map_err(CommandError::database)?;
            cleanup
        } else {
            Vec::new()
        };
        let changed = transaction
            .execute(
                "UPDATE editor_drafts
                 SET state = 'discarded', discarded_at = ?1, updated_at = ?1,
                     revision = revision + 1
                 WHERE id = ?2 AND revision = ?3 AND state IN ('active', 'conflicted')",
                params![now, input.draft_id, input.revision],
            )
            .map_err(CommandError::database)?;
        if changed != 1 {
            return Err(CommandError::new(
                "editor_draft_changed",
                "This recovered draft changed before it could be discarded.",
            ));
        }
        transaction.commit().map_err(CommandError::database)?;
        Ok(cleanup)
    })?;
    asset_lifecycle::remove_editor_cleanup_paths(&cleanup);
    Ok(())
}

#[tauri::command]
pub fn commit_editor_draft(
    database: State<'_, Database>,
    input: CommitEditorDraftInput,
) -> Result<(), CommandError> {
    validate_entity_id(&input.draft_id, "editor draft")?;
    validate_optional_id(input.target_entity_id.as_deref(), "editor draft target")?;
    if input.revision < 1 {
        return Err(invalid_draft_revision());
    }
    database.with_connection(|connection| {
        let current = load_draft(connection, &input.draft_id)?;
        let reference = EditorDraftCommitRef {
            draft_id: input.draft_id.clone(),
            revision: input.revision,
        };
        ensure_canonical_commit_matches(
            &current,
            current.draft_kind,
            input.target_entity_id.as_deref(),
            current.base_revision,
            current.asset_draft_id.as_deref(),
        )?;
        if current.state == "committed" {
            return Ok(());
        }
        if current.state != "active" || current.revision != input.revision {
            return Err(editor_draft_changed());
        }

        let now = unix_time_millis()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        if let Some(asset_draft_id) = current.asset_draft_id.as_deref() {
            let active_assets: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM assets
                     WHERE draft_id = ?1 AND lifecycle_state = 'staged' AND deleted_at IS NULL",
                    [asset_draft_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            if active_assets > 0 {
                return Err(CommandError::new(
                    "editor_draft_assets_not_committed",
                    "The final authoring transaction did not promote every draft image.",
                ));
            }
            transaction
                .execute(
                    "UPDATE asset_drafts
                     SET committed_at = COALESCE(committed_at, ?1), updated_at = ?1
                     WHERE id = ?2 AND canceled_at IS NULL",
                    params![now, asset_draft_id],
                )
                .map_err(CommandError::database)?;
        }
        commit_canonical_editor_draft(
            &transaction,
            Some(&reference),
            current.draft_kind,
            input.target_entity_id.as_deref(),
            current.base_revision,
            current.asset_draft_id.as_deref(),
            now,
        )?;
        transaction.commit().map_err(CommandError::database)
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn commit_canonical_editor_draft(
    connection: &Connection,
    reference: Option<&EditorDraftCommitRef>,
    expected_kind: EditorDraftKind,
    target_entity_id: Option<&str>,
    expected_base_revision: Option<i64>,
    expected_asset_draft_id: Option<&str>,
    now: i64,
) -> Result<(), CommandError> {
    let Some(reference) = reference else {
        return Ok(());
    };
    validate_entity_id(&reference.draft_id, "editor draft")?;
    validate_optional_id(target_entity_id, "editor draft target")?;
    validate_optional_id(expected_asset_draft_id, "asset draft")?;
    validate_base_revision(expected_base_revision)?;
    if reference.revision < 1 {
        return Err(invalid_draft_revision());
    }

    let current = load_draft(connection, &reference.draft_id)?;
    ensure_canonical_commit_matches(
        &current,
        expected_kind,
        target_entity_id,
        expected_base_revision,
        expected_asset_draft_id,
    )?;
    if current.state == "committed" {
        if target_entity_id.is_some() {
            return Ok(());
        }
        return Err(CommandError::new(
            "editor_draft_already_committed",
            "This authoring draft was already committed. Reload the saved content instead of creating it again.",
        ));
    }
    if current.state != "active" || current.revision != reference.revision {
        return Err(editor_draft_changed());
    }

    let changed = connection
        .execute(
            "UPDATE editor_drafts
             SET state = 'committed', target_entity_id = COALESCE(?1, target_entity_id),
                 committed_at = ?2, updated_at = ?2, revision = revision + 1
             WHERE id = ?3 AND revision = ?4 AND state = 'active'",
            params![
                target_entity_id,
                now,
                reference.draft_id,
                reference.revision
            ],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(editor_draft_changed());
    }
    Ok(())
}

fn ensure_canonical_commit_matches(
    current: &EditorDraftDto,
    expected_kind: EditorDraftKind,
    target_entity_id: Option<&str>,
    expected_base_revision: Option<i64>,
    expected_asset_draft_id: Option<&str>,
) -> Result<(), CommandError> {
    if current.draft_kind != expected_kind {
        return Err(CommandError::new(
            "editor_draft_kind_mismatch",
            "The recovered draft belongs to a different authoring workflow.",
        ));
    }
    if current
        .target_entity_id
        .as_deref()
        .is_some_and(|stored| Some(stored) != target_entity_id)
    {
        return Err(CommandError::new(
            "editor_draft_target_mismatch",
            "The recovered draft belongs to a different saved item.",
        ));
    }
    if current.base_revision != expected_base_revision {
        return Err(CommandError::new(
            "editor_draft_base_changed",
            "The saved item changed after this recoverable draft was opened.",
        ));
    }
    if current.asset_draft_id.as_deref() != expected_asset_draft_id {
        return Err(CommandError::new(
            "editor_draft_asset_mismatch",
            "The recovered draft does not own the images supplied to this save.",
        ));
    }
    Ok(())
}

fn editor_draft_changed() -> CommandError {
    CommandError::new(
        "editor_draft_changed",
        "This recovered draft changed before its final save completed.",
    )
}

#[tauri::command]
pub fn list_editor_drafts(
    database: State<'_, Database>,
) -> Result<Vec<EditorDraftDto>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT id FROM editor_drafts
                 WHERE state IN ('active', 'conflicted')
                 ORDER BY updated_at DESC, id",
            )
            .map_err(CommandError::database)?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        ids.into_iter()
            .map(|id| load_draft(connection, &id))
            .collect()
    })
}

pub(crate) fn reconcile_startup(database: &Database) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let ids = {
            let mut statement = connection
                .prepare("SELECT id FROM editor_drafts WHERE state = 'active' ORDER BY id")
                .map_err(CommandError::database)?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            rows
        };
        let now = unix_time_millis()?;
        for id in ids {
            let draft = load_draft(connection, &id)?;
            let invalid_payload = validate_payload(
                draft.draft_kind,
                draft.payload_schema_version,
                &draft.payload,
            )
            .is_err();
            let missing_asset_draft = match draft.asset_draft_id.as_deref() {
                Some(asset_draft_id) => {
                    connection
                        .query_row(
                            "SELECT EXISTS(
                                 SELECT 1 FROM asset_drafts
                                 WHERE id = ?1 AND canceled_at IS NULL
                             )",
                            [asset_draft_id],
                            |row| row.get::<_, i64>(0),
                        )
                        .map_err(CommandError::database)?
                        == 0
                }
                None => false,
            };
            if invalid_payload || missing_asset_draft {
                connection
                    .execute(
                        "UPDATE editor_drafts
                         SET state = 'conflicted', conflicted_at = ?1, updated_at = ?1,
                             revision = revision + 1
                         WHERE id = ?2 AND state = 'active'",
                        params![now, id],
                    )
                    .map_err(CommandError::database)?;
            }
        }
        Ok(())
    })
}

fn load_by_operation(
    connection: &Connection,
    operation_id: &str,
) -> Result<Option<EditorDraftDto>, CommandError> {
    connection
        .query_row(
            "SELECT id FROM editor_drafts WHERE open_operation_id = ?1",
            [operation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(CommandError::database)?
        .map(|id| load_draft(connection, &id))
        .transpose()
}

fn load_resumable_draft(
    connection: &Connection,
    kind: EditorDraftKind,
    target_entity_id: Option<&str>,
    context_entity_id: Option<&str>,
) -> Result<Option<EditorDraftDto>, CommandError> {
    let id: Option<String> = if let Some(target) = target_entity_id {
        connection
            .query_row(
                "SELECT id FROM editor_drafts
                 WHERE draft_kind = ?1 AND target_entity_id = ?2 AND state = 'active'
                 ORDER BY updated_at DESC LIMIT 1",
                params![kind.as_str(), target],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?
    } else if let Some(context) = context_entity_id {
        connection
            .query_row(
                "SELECT id FROM editor_drafts
                 WHERE draft_kind = ?1 AND target_entity_id IS NULL
                   AND context_entity_id = ?2 AND state = 'active'
                 ORDER BY updated_at DESC LIMIT 1",
                params![kind.as_str(), context],
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::database)?
    } else {
        None
    };
    id.map(|id| load_draft(connection, &id)).transpose()
}

fn touch_draft(connection: &Connection, draft_id: &str) -> Result<EditorDraftDto, CommandError> {
    let now = unix_time_millis()?;
    connection
        .execute(
            "UPDATE editor_drafts SET last_opened_at = ?1 WHERE id = ?2",
            params![now, draft_id],
        )
        .map_err(CommandError::database)?;
    load_draft(connection, draft_id)
}

fn load_draft(connection: &Connection, draft_id: &str) -> Result<EditorDraftDto, CommandError> {
    connection
        .query_row(
            "SELECT draft_kind, target_entity_id, context_entity_id, base_revision,
                    payload_schema_version, payload_json, revision, state,
                    asset_draft_id, created_at, updated_at, last_opened_at
             FROM editor_drafts WHERE id = ?1",
            [draft_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
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
                "editor_draft_not_found",
                "This recovered editor draft no longer exists.",
            )
        })
        .and_then(|row| {
            Ok(EditorDraftDto {
                id: draft_id.to_string(),
                draft_kind: EditorDraftKind::from_db(&row.0)?,
                target_entity_id: row.1,
                context_entity_id: row.2,
                base_revision: row.3,
                payload_schema_version: row.4,
                payload: serde_json::from_str(&row.5).map_err(CommandError::serialization)?,
                revision: row.6,
                state: row.7,
                asset_draft_id: row.8,
                created_at: row.9,
                updated_at: row.10,
                last_opened_at: row.11,
            })
        })
}

fn ensure_open_request_matches(
    existing: &EditorDraftDto,
    input: &OpenEditorDraftInput,
) -> Result<(), CommandError> {
    if existing.draft_kind == input.draft_kind
        && existing.target_entity_id == input.target_entity_id
        && existing.context_entity_id == input.context_entity_id
        && existing.base_revision == input.base_revision
    {
        Ok(())
    } else {
        Err(CommandError::new(
            "idempotency_conflict",
            "That draft-open retry identifier was already used for another editor.",
        ))
    }
}

fn validate_payload(
    kind: EditorDraftKind,
    schema_version: i64,
    payload: &Value,
) -> Result<(), CommandError> {
    if schema_version != PAYLOAD_SCHEMA_VERSION {
        return Err(CommandError::new(
            "unsupported_editor_draft_version",
            "This editor draft uses an unsupported payload version.",
        ));
    }
    let object = payload.as_object().ok_or_else(|| {
        CommandError::new(
            "invalid_editor_draft",
            "Editor draft payloads must be JSON objects.",
        )
    })?;
    if kind == EditorDraftKind::BulkCards {
        let row_count = object
            .get("rows")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        if row_count > MAX_BULK_ROWS {
            return Err(CommandError::new(
                "editor_draft_too_large",
                "A bulk-card draft cannot contain more than 500 rows.",
            ));
        }
    }
    inspect_payload(payload, 0)?;
    let json = canonical_payload_json(payload)?;
    if json.len() > MAX_DRAFT_PAYLOAD_BYTES {
        return Err(CommandError::new(
            "editor_draft_too_large",
            "The editor draft exceeds the 4 MB recovery limit.",
        ));
    }
    Ok(())
}

fn inspect_payload(value: &Value, depth: usize) -> Result<(), CommandError> {
    if depth > MAX_DRAFT_DEPTH {
        return Err(CommandError::new(
            "editor_draft_too_deep",
            "The editor draft contains excessively nested data.",
        ));
    }
    match value {
        Value::Object(object) => {
            if object.len() > MAX_DRAFT_COLLECTION_ITEMS {
                return Err(CommandError::new(
                    "editor_draft_too_large",
                    "The editor draft contains too many properties.",
                ));
            }
            for (key, nested) in object {
                if is_sensitive_key(key) {
                    return Err(CommandError::new(
                        "editor_draft_contains_secret",
                        "Credentials and secret tokens cannot be stored in an editor draft.",
                    ));
                }
                inspect_payload(nested, depth + 1)?;
            }
            if object.get("type").and_then(Value::as_str) == Some("doc") {
                crate::cards::validate_document(value)?;
            }
        }
        Value::Array(values) => {
            if values.len() > MAX_DRAFT_COLLECTION_ITEMS {
                return Err(CommandError::new(
                    "editor_draft_too_large",
                    "The editor draft contains too many list items.",
                ));
            }
            for nested in values {
                inspect_payload(nested, depth + 1)?;
            }
        }
        Value::String(text) if text.chars().count() > MAX_DRAFT_STRING_CHARS => {
            return Err(CommandError::new(
                "editor_draft_too_large",
                "One editor-draft field exceeds the recovery size limit.",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    matches!(
        normalized.as_str(),
        "credential"
            | "credentialdraft"
            | "apikey"
            | "secret"
            | "secretvalue"
            | "accesstoken"
            | "refreshtoken"
            | "password"
            | "privatekey"
    )
}

fn canonical_payload_json(payload: &Value) -> Result<String, CommandError> {
    serde_json::to_string(payload).map_err(CommandError::serialization)
}

fn hash_payload(payload_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(payload_json.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn normalize_asset_ids(values: Vec<String>) -> Result<BTreeSet<String>, CommandError> {
    let mut output = BTreeSet::new();
    for value in values {
        validate_entity_id(&value, "draft asset")?;
        output.insert(value);
    }
    Ok(output)
}

fn validate_optional_id(value: Option<&str>, label: &str) -> Result<(), CommandError> {
    if let Some(value) = value {
        validate_entity_id(value, label)?;
    }
    Ok(())
}

fn validate_base_revision(value: Option<i64>) -> Result<(), CommandError> {
    if value.is_some_and(|revision| revision < 1) {
        return Err(CommandError::new(
            "invalid_revision",
            "The editor draft base revision is invalid.",
        ));
    }
    Ok(())
}

fn invalid_draft_revision() -> CommandError {
    CommandError::new("invalid_revision", "The editor draft revision is invalid.")
}

fn inactive_draft_error(state: &str) -> CommandError {
    let message = match state {
        "committed" => "This editor draft was already saved.",
        "discarded" => "This editor draft was already discarded.",
        "conflicted" => "This editor draft conflicts with newer saved data.",
        _ => "This editor draft is no longer active.",
    };
    CommandError::new("editor_draft_inactive", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_sensitive_fields_at_any_depth() {
        assert!(validate_payload(
            EditorDraftKind::AiProfile,
            1,
            &serde_json::json!({"profile": {"credentialDraft": "secret"}}),
        )
        .is_err());
    }

    #[test]
    fn accepts_incomplete_bounded_authoring_data() {
        validate_payload(
            EditorDraftKind::Card,
            1,
            &serde_json::json!({
                "frontDoc": {"type":"doc","content":[{"type":"paragraph"}]},
                "backDoc": {"type":"doc","content":[{"type":"paragraph"}]},
                "frontText":"",
                "backText":""
            }),
        )
        .expect("incomplete safe card draft should be recoverable");
    }

    fn insert_test_draft(
        connection: &Connection,
        draft_id: &str,
        kind: EditorDraftKind,
        target_entity_id: Option<&str>,
        base_revision: Option<i64>,
    ) -> Result<(), CommandError> {
        let operation_id = "f".repeat(32);
        let payload = "{}";
        connection
            .execute(
                "INSERT INTO editor_drafts (
                     id, open_operation_id, draft_kind, target_entity_id,
                     context_entity_id, base_revision, payload_schema_version,
                     payload_json, payload_sha256, revision, state, asset_draft_id,
                     created_at, updated_at, last_opened_at
                 ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, 1, ?6, ?7, 1, 'active', NULL, 1, 1, 1)",
                params![
                    draft_id,
                    operation_id,
                    kind.as_str(),
                    target_entity_id,
                    base_revision,
                    payload,
                    hash_payload(payload),
                ],
            )
            .map_err(CommandError::database)?;
        Ok(())
    }

    #[test]
    fn canonical_draft_commit_rolls_back_with_the_callers_transaction() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let draft_id = "a".repeat(32);
                insert_test_draft(
                    connection,
                    &draft_id,
                    EditorDraftKind::BulkCards,
                    None,
                    None,
                )?;
                connection
                    .execute_batch("CREATE TEMP TABLE atomic_draft_probe (value INTEGER NOT NULL);")
                    .map_err(CommandError::database)?;
                {
                    let transaction = connection
                        .transaction_with_behavior(TransactionBehavior::Immediate)
                        .map_err(CommandError::database)?;
                    transaction
                        .execute("INSERT INTO atomic_draft_probe (value) VALUES (1)", [])
                        .map_err(CommandError::database)?;
                    commit_canonical_editor_draft(
                        &transaction,
                        Some(&EditorDraftCommitRef {
                            draft_id: draft_id.clone(),
                            revision: 1,
                        }),
                        EditorDraftKind::BulkCards,
                        None,
                        None,
                        None,
                        2,
                    )?;
                }
                let count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM atomic_draft_probe", [], |row| {
                        row.get(0)
                    })
                    .map_err(CommandError::database)?;
                let state: String = connection
                    .query_row(
                        "SELECT state FROM editor_drafts WHERE id = ?1",
                        [draft_id],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(count, 0);
                assert_eq!(state, "active");
                Ok(())
            })
            .expect("rollback should be observable");
    }

    #[test]
    fn canonical_draft_commit_commits_with_the_callers_mutation() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let draft_id = "b".repeat(32);
                let target_id = "c".repeat(32);
                insert_test_draft(
                    connection,
                    &draft_id,
                    EditorDraftKind::AiProfile,
                    None,
                    None,
                )?;
                connection
                    .execute_batch("CREATE TEMP TABLE atomic_draft_probe (value INTEGER NOT NULL);")
                    .map_err(CommandError::database)?;
                let transaction = connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(CommandError::database)?;
                transaction
                    .execute("INSERT INTO atomic_draft_probe (value) VALUES (1)", [])
                    .map_err(CommandError::database)?;
                commit_canonical_editor_draft(
                    &transaction,
                    Some(&EditorDraftCommitRef {
                        draft_id: draft_id.clone(),
                        revision: 1,
                    }),
                    EditorDraftKind::AiProfile,
                    Some(&target_id),
                    None,
                    None,
                    2,
                )?;
                transaction.commit().map_err(CommandError::database)?;
                let committed: (String, Option<String>) = connection
                    .query_row(
                        "SELECT state, target_entity_id FROM editor_drafts WHERE id = ?1",
                        [draft_id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(committed, ("committed".to_string(), Some(target_id)));
                Ok(())
            })
            .expect("commit should be observable");
    }

    #[test]
    fn canonical_draft_commit_rejects_the_wrong_workflow() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                let draft_id = "d".repeat(32);
                insert_test_draft(connection, &draft_id, EditorDraftKind::Card, None, None)?;
                let error = commit_canonical_editor_draft(
                    connection,
                    Some(&EditorDraftCommitRef {
                        draft_id,
                        revision: 1,
                    }),
                    EditorDraftKind::Template,
                    None,
                    None,
                    None,
                    2,
                )
                .expect_err("wrong draft kind must fail");
                assert_eq!(error.code, "editor_draft_kind_mismatch");
                Ok(())
            })
            .expect("test database should remain usable");
    }
}
