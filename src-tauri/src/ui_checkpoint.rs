//! Small device-local UI restoration checkpoint.
//!
//! This file is deliberately separate from SQLite and `.ostudybackup`: it
//! contains only presentation destinations and idempotent practice identities,
//! never card content, managed files, credentials, or learner history.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    error::CommandError,
    paths::AppPaths,
    util::{unix_time_millis, validate_entity_id},
};

const CHECKPOINT_SCHEMA_VERSION: i64 = 1;
const MAX_CHECKPOINT_BYTES: usize = 64 * 1024;
const DEVICE_STATE_DIRECTORY: &str = "device-state";
const CHECKPOINT_FILENAME: &str = "ui-checkpoint-v1.json";
const FLASHCARDS_WORKSPACES: &[&str] = &["decks", "templates", "search", "transfer"];
const FLASHCARDS_VIEWS: &[&str] = &["root", "deck"];
const LEGACY_CREATE_WORKSPACE: &str = "create";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointSection {
    Decks,
    Documents,
    Cards,
    Templates,
    Practice,
    Analytics,
    Settings,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PracticeCheckpoint {
    pub session_id: String,
    pub position: i64,
    pub revealed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyticsCheckpoint {
    pub range: String,
    pub deck_ids: Vec<String>,
    pub tag_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiCheckpointDto {
    pub schema_version: i64,
    pub updated_at: i64,
    pub section: CheckpointSection,
    pub flashcards_workspace: Option<String>,
    pub flashcards_view: Option<String>,
    pub deck_id: Option<String>,
    pub editor_draft_id: Option<String>,
    pub practice: Option<PracticeCheckpoint>,
    pub analytics: Option<AnalyticsCheckpoint>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveUiCheckpointInput {
    pub section: CheckpointSection,
    pub flashcards_workspace: Option<String>,
    pub flashcards_view: Option<String>,
    pub deck_id: Option<String>,
    pub editor_draft_id: Option<String>,
    pub practice: Option<PracticeCheckpoint>,
    pub analytics: Option<AnalyticsCheckpoint>,
}

#[tauri::command]
pub fn get_ui_checkpoint(
    paths: State<'_, AppPaths>,
) -> Result<Option<UiCheckpointDto>, CommandError> {
    let path = checkpoint_path(paths.inner());
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(CommandError::io(error)),
    };
    if bytes.is_empty() || bytes.len() > MAX_CHECKPOINT_BYTES {
        quarantine_invalid_checkpoint(paths.inner(), &path)?;
        return Ok(None);
    }
    let mut checkpoint: UiCheckpointDto = match serde_json::from_slice(&bytes) {
        Ok(checkpoint) => checkpoint,
        Err(_) => {
            quarantine_invalid_checkpoint(paths.inner(), &path)?;
            return Ok(None);
        }
    };
    normalize_legacy_checkpoint(&mut checkpoint);
    if validate_checkpoint(&checkpoint).is_err() {
        quarantine_invalid_checkpoint(paths.inner(), &path)?;
        return Ok(None);
    }
    Ok(Some(checkpoint))
}

#[tauri::command]
pub fn save_ui_checkpoint(
    paths: State<'_, AppPaths>,
    input: SaveUiCheckpointInput,
) -> Result<UiCheckpointDto, CommandError> {
    let checkpoint = UiCheckpointDto {
        schema_version: CHECKPOINT_SCHEMA_VERSION,
        updated_at: unix_time_millis()?,
        section: input.section,
        flashcards_workspace: input.flashcards_workspace,
        flashcards_view: input.flashcards_view,
        deck_id: input.deck_id,
        editor_draft_id: input.editor_draft_id,
        practice: input.practice,
        analytics: input.analytics,
    };
    validate_checkpoint(&checkpoint)?;
    let encoded = serde_json::to_vec(&checkpoint).map_err(CommandError::serialization)?;
    if encoded.len() > MAX_CHECKPOINT_BYTES {
        return Err(CommandError::new(
            "ui_checkpoint_too_large",
            "The device restoration checkpoint is larger than the supported limit.",
        ));
    }

    let path = checkpoint_path(paths.inner());
    let parent = path.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_ui_checkpoint_path",
            "The device restoration directory is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let temporary = parent.join(format!("{CHECKPOINT_FILENAME}.partial"));
    remove_file_if_exists(&temporary)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(CommandError::io)?;
    file.write_all(&encoded).map_err(CommandError::io)?;
    file.sync_all().map_err(CommandError::io)?;
    drop(file);
    remove_file_if_exists(&path)?;
    fs::rename(&temporary, &path).map_err(CommandError::io)?;
    sync_directory(parent)?;
    Ok(checkpoint)
}

#[tauri::command]
pub fn clear_ui_checkpoint(paths: State<'_, AppPaths>) -> Result<(), CommandError> {
    remove_file_if_exists(&checkpoint_path(paths.inner()))
}

fn normalize_legacy_checkpoint(checkpoint: &mut UiCheckpointDto) {
    if checkpoint.flashcards_workspace.as_deref() == Some(LEGACY_CREATE_WORKSPACE) {
        checkpoint.flashcards_workspace = Some("decks".to_string());
        checkpoint.flashcards_view = Some("root".to_string());
        checkpoint.deck_id = None;
        return;
    }

    if checkpoint.flashcards_view.is_none() {
        checkpoint.flashcards_view = Some(
            if checkpoint.flashcards_workspace.as_deref() == Some("decks")
                && checkpoint.deck_id.is_some()
            {
                "deck"
            } else {
                "root"
            }
            .to_string(),
        );
    }
}

fn validate_checkpoint(checkpoint: &UiCheckpointDto) -> Result<(), CommandError> {
    if checkpoint.schema_version != CHECKPOINT_SCHEMA_VERSION {
        return Err(CommandError::new(
            "unsupported_ui_checkpoint",
            "The device restoration checkpoint uses an unsupported format.",
        ));
    }
    if checkpoint
        .flashcards_workspace
        .as_deref()
        .is_some_and(|workspace| !FLASHCARDS_WORKSPACES.contains(&workspace))
    {
        return Err(CommandError::new(
            "invalid_ui_checkpoint",
            "The restored Flashcards workspace is invalid.",
        ));
    }
    if checkpoint
        .flashcards_view
        .as_deref()
        .is_some_and(|view| !FLASHCARDS_VIEWS.contains(&view))
    {
        return Err(CommandError::new(
            "invalid_ui_checkpoint",
            "The restored Flashcards view is invalid.",
        ));
    }
    if checkpoint.flashcards_view.as_deref() == Some("deck") {
        if checkpoint.flashcards_workspace.as_deref() != Some("decks")
            || checkpoint.deck_id.is_none()
        {
            return Err(CommandError::new(
                "invalid_ui_checkpoint",
                "A restored deck view requires a deck destination.",
            ));
        }
    }
    if checkpoint.flashcards_view.as_deref() == Some("root") && checkpoint.deck_id.is_some() {
        return Err(CommandError::new(
            "invalid_ui_checkpoint",
            "A restored Flashcards root cannot include a deck destination.",
        ));
    }
    validate_optional_id(checkpoint.deck_id.as_deref(), "deck")?;
    validate_optional_id(checkpoint.editor_draft_id.as_deref(), "editor draft")?;
    if let Some(practice) = &checkpoint.practice {
        validate_entity_id(&practice.session_id, "practice session")?;
        if practice.position < 0 || practice.position > 1_000_000 {
            return Err(CommandError::new(
                "invalid_ui_checkpoint",
                "The restored practice position is invalid.",
            ));
        }
    }
    if let Some(analytics) = &checkpoint.analytics {
        if !matches!(analytics.range.as_str(), "7d" | "30d" | "90d" | "all") {
            return Err(CommandError::new(
                "invalid_ui_checkpoint",
                "The restored analytics range is invalid.",
            ));
        }
        validate_id_list(&analytics.deck_ids, "analytics deck")?;
        validate_id_list(&analytics.tag_ids, "analytics tag")?;
    }
    Ok(())
}

fn validate_optional_id(value: Option<&str>, label: &str) -> Result<(), CommandError> {
    if let Some(value) = value {
        validate_entity_id(value, label)?;
    }
    Ok(())
}

fn validate_id_list(values: &[String], label: &str) -> Result<(), CommandError> {
    if values.len() > 2_000 {
        return Err(CommandError::new(
            "invalid_ui_checkpoint",
            "The device restoration checkpoint contains too many identifiers.",
        ));
    }
    for value in values {
        validate_entity_id(value, label)?;
    }
    Ok(())
}

fn checkpoint_path(paths: &AppPaths) -> PathBuf {
    paths
        .app_data_dir()
        .join(DEVICE_STATE_DIRECTORY)
        .join(CHECKPOINT_FILENAME)
}

fn quarantine_invalid_checkpoint(paths: &AppPaths, path: &Path) -> Result<(), CommandError> {
    let quarantine = paths
        .app_data_dir()
        .join(DEVICE_STATE_DIRECTORY)
        .join("invalid-ui-checkpoint.json.quarantine");
    remove_file_if_exists(&quarantine)?;
    match fs::rename(path, quarantine) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::io(error)),
    }
}

fn remove_file_if_exists(path: &Path) -> Result<(), CommandError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::io(error)),
    }
}

fn sync_directory(path: &Path) -> Result<(), CommandError> {
    let directory = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(CommandError::io)?;
    directory.sync_all().map_err(CommandError::io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_rejects_sensitive_or_unbounded_shapes_through_strict_deserialization() {
        let encoded = br#"{
          "schemaVersion":1,
          "updatedAt":1,
          "section":"decks",
          "flashcardsWorkspace":"decks",
          "flashcardsView":"root",
          "deckId":null,
          "editorDraftId":null,
          "practice":null,
          "analytics":null,
          "credential":"secret"
        }"#;
        assert!(serde_json::from_slice::<UiCheckpointDto>(encoded).is_err());
    }

    #[test]
    fn checkpoint_accepts_a_bounded_practice_identity() {
        let checkpoint = UiCheckpointDto {
            schema_version: 1,
            updated_at: 1,
            section: CheckpointSection::Practice,
            flashcards_workspace: Some("decks".to_string()),
            flashcards_view: Some("root".to_string()),
            deck_id: None,
            editor_draft_id: None,
            practice: Some(PracticeCheckpoint {
                session_id: "a".repeat(32),
                position: 4,
                revealed: true,
            }),
            analytics: None,
        };
        validate_checkpoint(&checkpoint).expect("checkpoint should be valid");
    }

    #[test]
    fn checkpoint_rejects_unknown_flashcards_workspace() {
        let checkpoint = UiCheckpointDto {
            schema_version: 1,
            updated_at: 1,
            section: CheckpointSection::Decks,
            flashcards_workspace: Some("hidden-admin".to_string()),
            flashcards_view: Some("root".to_string()),
            deck_id: None,
            editor_draft_id: None,
            practice: None,
            analytics: None,
        };
        assert_eq!(
            validate_checkpoint(&checkpoint).unwrap_err().code,
            "invalid_ui_checkpoint"
        );
    }

    #[test]
    fn legacy_create_workspace_returns_to_decks_root() {
        let mut checkpoint = UiCheckpointDto {
            schema_version: 1,
            updated_at: 1,
            section: CheckpointSection::Decks,
            flashcards_workspace: Some("create".to_string()),
            flashcards_view: None,
            deck_id: Some("a".repeat(32)),
            editor_draft_id: None,
            practice: None,
            analytics: None,
        };
        normalize_legacy_checkpoint(&mut checkpoint);
        assert_eq!(checkpoint.flashcards_workspace.as_deref(), Some("decks"));
        assert_eq!(checkpoint.flashcards_view.as_deref(), Some("root"));
        assert!(checkpoint.deck_id.is_none());
        validate_checkpoint(&checkpoint).expect("normalized checkpoint should be valid");
    }

    #[test]
    fn legacy_deck_id_becomes_an_explicit_deck_view() {
        let mut checkpoint = UiCheckpointDto {
            schema_version: 1,
            updated_at: 1,
            section: CheckpointSection::Decks,
            flashcards_workspace: Some("decks".to_string()),
            flashcards_view: None,
            deck_id: Some("b".repeat(32)),
            editor_draft_id: None,
            practice: None,
            analytics: None,
        };
        normalize_legacy_checkpoint(&mut checkpoint);
        assert_eq!(checkpoint.flashcards_view.as_deref(), Some("deck"));
        validate_checkpoint(&checkpoint).expect("legacy deck checkpoint should remain valid");
    }
}
