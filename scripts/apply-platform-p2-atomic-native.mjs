#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");

function replaceExact(text, oldValue, newValue, label) {
  if (!text.includes(oldValue)) throw new Error(`${label}: expected source was not found`);
  return text.replace(oldValue, newValue);
}

function replaceAllExact(text, oldValue, newValue, expectedCount, label) {
  const count = text.split(oldValue).length - 1;
  if (count !== expectedCount) {
    throw new Error(`${label}: expected ${expectedCount} matches, found ${count}`);
  }
  return text.split(oldValue).join(newValue);
}

function replaceBetween(text, startMarker, endMarker, replacement, label) {
  const start = text.indexOf(startMarker);
  const end = text.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0) throw new Error(`${label}: replacement range was not found`);
  return text.slice(0, start) + replacement + text.slice(end);
}

function replaceInside(text, startMarker, endMarker, oldValue, newValue, label) {
  const start = text.indexOf(startMarker);
  const end = text.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0) throw new Error(`${label}: range was not found`);
  const segment = text.slice(start, end);
  const replaced = replaceExact(segment, oldValue, newValue, label);
  return text.slice(0, start) + replaced + text.slice(end);
}

// Durable-draft transaction primitive.
let drafts = await read("src-tauri/src/editor_drafts.rs");
drafts = replaceExact(
  drafts,
  "use std::{collections::BTreeSet, path::PathBuf};",
  "use std::collections::BTreeSet;",
  "remove unused editor draft path import",
);
drafts = replaceExact(
  drafts,
  `#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommitEditorDraftInput {
    pub draft_id: String,
    pub revision: i64,
    pub target_entity_id: Option<String>,
}`,
  `#[derive(Clone, Debug, Deserialize, Serialize)]
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
}`,
  "add canonical editor draft reference",
);
const commitBlock = `#[tauri::command]
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
        return Ok(());
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
            params![target_entity_id, now, reference.draft_id, reference.revision],
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

`;
drafts = replaceBetween(
  drafts,
  "#[tauri::command]\npub fn commit_editor_draft(",
  "#[tauri::command]\npub fn list_editor_drafts(",
  commitBlock,
  "replace standalone draft commit with shared primitive",
);
const draftTests = `

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
                insert_test_draft(connection, &draft_id, EditorDraftKind::BulkCards, None, None)?;
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
                    .query_row("SELECT COUNT(*) FROM atomic_draft_probe", [], |row| row.get(0))
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
                insert_test_draft(connection, &draft_id, EditorDraftKind::AiProfile, None, None)?;
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
`;
const testsEnd = drafts.lastIndexOf("\n}");
if (testsEnd < 0) throw new Error("editor draft test module ending was not found");
drafts = drafts.slice(0, testsEnd) + draftTests + drafts.slice(testsEnd);
await write("src-tauri/src/editor_drafts.rs", drafts);

// Card and template input contracts plus card transactions.
let cardTypes = await read("src-tauri/src/authoring/card_types_and_templates.rs");
cardTypes = replaceAllExact(
  cardTypes,
  `        #[serde(default)]
        asset_draft_id: Option<String>,
    },`,
  `        #[serde(default)]
        asset_draft_id: Option<String>,
        #[serde(default)]
        editor_draft: Option<crate::editor_drafts::EditorDraftCommitRef>,
    },`,
  4,
  "add card draft commit references",
);
cardTypes = replaceExact(
  cardTypes,
  `    #[serde(default)]
    pub asset_draft_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCardTemplateInput`,
  `    #[serde(default)]
    pub asset_draft_id: Option<String>,
    #[serde(default)]
    pub editor_draft: Option<crate::editor_drafts::EditorDraftCommitRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateCardTemplateInput`,
  "add create template draft reference",
);
cardTypes = replaceExact(
  cardTypes,
  `    #[serde(default)]
    pub asset_draft_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateFieldDto`,
  `    #[serde(default)]
    pub asset_draft_id: Option<String>,
    #[serde(default)]
    pub editor_draft: Option<crate::editor_drafts::EditorDraftCommitRef>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardTemplateFieldDto`,
  "add update template draft reference",
);
cardTypes = replaceExact(
  cardTypes,
  `    #[serde(default)]
    pub asset_draft_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCardPreviewDto`,
  `    #[serde(default)]
    pub asset_draft_id: Option<String>,
    #[serde(default)]
    pub editor_draft: Option<crate::editor_drafts::EditorDraftCommitRef>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCardPreviewDto`,
  "add generated template draft reference",
);
cardTypes = replaceExact(
  cardTypes,
  `    allow_duplicate: bool,
    asset_draft_id: Option<String>,
}`,
  `    allow_duplicate: bool,
    asset_draft_id: Option<String>,
    editor_draft: Option<crate::editor_drafts::EditorDraftCommitRef>,
}`,
  "extend generated card write",
);
cardTypes = replaceAllExact(
  cardTypes,
  `            asset_draft_id,
        } => GeneratedCardWrite {`,
  `            asset_draft_id,
            editor_draft,
        } => GeneratedCardWrite {`,
  4,
  "destructure card draft commit references",
);
cardTypes = replaceAllExact(
  cardTypes,
  `            asset_draft_id,
        },`,
  `            asset_draft_id,
            editor_draft,
        },`,
  4,
  "store card draft commit references",
);
cardTypes = replaceAllExact(
  cardTypes,
  `        asset_draft_id,
    } = write;`,
  `        asset_draft_id,
        editor_draft,
    } = write;`,
  2,
  "destructure prepared card draft reference",
);
cardTypes = replaceInside(
  cardTypes,
  "fn create_prepared_card_note(",
  "fn update_card_note_in_database(",
  `        )?;
        transaction.commit().map_err(CommandError::database)?;`,
  `        )?;
        crate::editor_drafts::commit_canonical_editor_draft(
            &transaction,
            editor_draft.as_ref(),
            crate::editor_drafts::EditorDraftKind::Card,
            Some(&note_id),
            None,
            asset_draft_id.as_deref(),
            now,
        )?;
        transaction.commit().map_err(CommandError::database)?;`,
  "commit new card draft atomically",
);
cardTypes = replaceInside(
  cardTypes,
  "fn update_prepared_card_note(",
  "fn prepare_basic_card_content(",
  `        )?;
        transaction.commit().map_err(CommandError::database)?;`,
  `        )?;
        crate::editor_drafts::commit_canonical_editor_draft(
            &transaction,
            editor_draft.as_ref(),
            crate::editor_drafts::EditorDraftKind::Card,
            Some(&note_id),
            Some(revision),
            asset_draft_id.as_deref(),
            now,
        )?;
        transaction.commit().map_err(CommandError::database)?;`,
  "commit updated card draft atomically",
);
await write("src-tauri/src/authoring/card_types_and_templates.rs", cardTypes);

// Saved template transactions.
let templates = await read("src-tauri/src/authoring/card_types_and_templates/templates.rs");
templates = replaceExact(
  templates,
  `) -> Result<CardTemplateDto, CommandError> {
    let name = validate_template_name(input.name)?;`,
  `) -> Result<CardTemplateDto, CommandError> {
    let editor_draft = input.editor_draft.clone();
    let asset_draft_id = input.asset_draft_id.clone();
    let name = validate_template_name(input.name)?;`,
  "capture create-template draft reference",
);
templates = replaceInside(
  templates,
  "pub(super) fn create_card_template_in_connection(",
  "pub(super) fn update_card_template_in_connection(",
  `    replace_template_assets(&transaction, &template_id, &asset_ids)?;
    transaction.commit().map_err(CommandError::database)?;`,
  `    replace_template_assets(&transaction, &template_id, &asset_ids)?;
    crate::editor_drafts::commit_canonical_editor_draft(
        &transaction,
        editor_draft.as_ref(),
        crate::editor_drafts::EditorDraftKind::Template,
        Some(&template_id),
        None,
        asset_draft_id.as_deref(),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;`,
  "commit create-template draft atomically",
);
templates = replaceExact(
  templates,
  `) -> Result<CardTemplateDto, CommandError> {
    validate_entity_id(&input.template_id, "card template")?;`,
  `) -> Result<CardTemplateDto, CommandError> {
    let editor_draft = input.editor_draft.clone();
    let asset_draft_id = input.asset_draft_id.clone();
    validate_entity_id(&input.template_id, "card template")?;`,
  "capture update-template draft reference",
);
templates = replaceInside(
  templates,
  "pub(super) fn update_card_template_in_connection(",
  "pub(super) fn delete_card_template_in_connection(",
  `    replace_template_assets(&transaction, &input.template_id, &asset_ids)?;
    transaction.commit().map_err(CommandError::database)?;`,
  `    replace_template_assets(&transaction, &input.template_id, &asset_ids)?;
    crate::editor_drafts::commit_canonical_editor_draft(
        &transaction,
        editor_draft.as_ref(),
        crate::editor_drafts::EditorDraftKind::Template,
        Some(&input.template_id),
        Some(input.revision),
        asset_draft_id.as_deref(),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;`,
  "commit update-template draft atomically",
);
templates = replaceInside(
  templates,
  "fn generate_template_cards_in_connection(",
  "pub(super) fn ensure_template_revision(",
  `    transaction.commit().map_err(CommandError::database)?;`,
  `    crate::editor_drafts::commit_canonical_editor_draft(
        &transaction,
        input.editor_draft.as_ref(),
        crate::editor_drafts::EditorDraftKind::Card,
        None,
        None,
        input.asset_draft_id.as_deref(),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;`,
  "commit inline generated-card draft atomically",
);
await write("src-tauri/src/authoring/card_types_and_templates/templates.rs", templates);

let automatic = await read(
  "src-tauri/src/authoring/card_types_and_templates/templates/automatic_layout.rs",
);
automatic = replaceInside(
  automatic,
  "pub(in super::super) fn generate_automatic_template_cards_in_connection(",
  "\n}",
  `    transaction.commit().map_err(CommandError::database)?;`,
  `    crate::editor_drafts::commit_canonical_editor_draft(
        &transaction,
        input.editor_draft.as_ref(),
        crate::editor_drafts::EditorDraftKind::Card,
        None,
        None,
        input.asset_draft_id.as_deref(),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;`,
  "commit structured generated-card draft atomically",
);
await write(
  "src-tauri/src/authoring/card_types_and_templates/templates/automatic_layout.rs",
  automatic,
);

// Bulk entry transaction.
let authoring = await read("src-tauri/src/authoring/mod.rs");
authoring = replaceExact(
  authoring,
  `pub struct BulkCreateCardsInput {
    pub deck_id: String,
    pub rows: Vec<BulkCardRowInput>,
}`,
  `pub struct BulkCreateCardsInput {
    pub deck_id: String,
    pub rows: Vec<BulkCardRowInput>,
    #[serde(default)]
    pub editor_draft: Option<crate::editor_drafts::EditorDraftCommitRef>,
}`,
  "add bulk draft commit reference",
);
await write("src-tauri/src/authoring/mod.rs", authoring);

let cardCommands = await read("src-tauri/src/authoring/card_commands.rs");
cardCommands = replaceExact(
  cardCommands,
  `    let BulkCreateCardsInput { deck_id, rows } = input;`,
  `    let BulkCreateCardsInput {
        deck_id,
        rows,
        editor_draft,
    } = input;`,
  "destructure bulk draft reference",
);
cardCommands = replaceInside(
  cardCommands,
  "pub fn bulk_create_cards(",
  "#[tauri::command]\npub fn bulk_tag_cards(",
  `        transaction.commit().map_err(CommandError::database)?;`,
  `        crate::editor_drafts::commit_canonical_editor_draft(
            &transaction,
            editor_draft.as_ref(),
            crate::editor_drafts::EditorDraftKind::BulkCards,
            None,
            None,
            None,
            now,
        )?;
        transaction.commit().map_err(CommandError::database)?;`,
  "commit bulk-entry draft atomically",
);
await write("src-tauri/src/authoring/card_commands.rs", cardCommands);

// AI profile metadata transactions.
let profiles = await read("src-tauri/src/ai/profiles.rs");
profiles = replaceExact(
  profiles,
  `    pub request_timeout_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAiProviderProfileInput`,
  `    pub request_timeout_ms: i64,
    #[serde(default)]
    pub editor_draft: Option<crate::editor_drafts::EditorDraftCommitRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAiProviderProfileInput`,
  "add create-profile draft reference",
);
profiles = replaceExact(
  profiles,
  `    pub request_timeout_ms: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteAiProviderProfileInput`,
  `    pub request_timeout_ms: i64,
    #[serde(default)]
    pub editor_draft: Option<crate::editor_drafts::EditorDraftCommitRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteAiProviderProfileInput`,
  "add update-profile draft reference",
);
profiles = replaceInside(
  profiles,
  "fn create_profile(",
  "fn update_profile(",
  `    record_operation(
        &transaction,
        &input.operation_id,
        "create_profile",
        Some(&id),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;`,
  `    record_operation(
        &transaction,
        &input.operation_id,
        "create_profile",
        Some(&id),
        now,
    )?;
    crate::editor_drafts::commit_canonical_editor_draft(
        &transaction,
        input.editor_draft.as_ref(),
        crate::editor_drafts::EditorDraftKind::AiProfile,
        Some(&id),
        None,
        None,
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;`,
  "commit created profile draft atomically",
);
profiles = replaceInside(
  profiles,
  "fn update_profile(",
  "fn delete_profile(",
  `    record_operation(
        &transaction,
        &input.operation_id,
        "update_profile",
        Some(&input.profile_id),
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;`,
  `    record_operation(
        &transaction,
        &input.operation_id,
        "update_profile",
        Some(&input.profile_id),
        now,
    )?;
    crate::editor_drafts::commit_canonical_editor_draft(
        &transaction,
        input.editor_draft.as_ref(),
        crate::editor_drafts::EditorDraftKind::AiProfile,
        Some(&input.profile_id),
        Some(input.revision),
        None,
        now,
    )?;
    transaction.commit().map_err(CommandError::database)?;`,
  "commit updated profile draft atomically",
);
await write("src-tauri/src/ai/profiles.rs", profiles);

console.log("Applied transaction-atomic durable-draft retirement to native authoring workflows.");
