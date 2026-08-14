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
    pub deck_id: Option<String>,
    pub editor_draft_id: Option<String>,
    pub practice: Option<PracticeCheckpoint>,
    pub analytics: Option<AnalyticsCheckpoint>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveUiCheckpointInput {
    pub section: CheckpointSection,
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
    let checkpoint: UiCheckpointDto = match serde_json::from_slice(&bytes) {
        Ok(checkpoint) => checkpoint,
        Err(_) => {
            quarantine_invalid_checkpoint(paths.inner(), &path)?;
            return Ok(None);
        }
    };
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

fn validate_checkpoint(checkpoint: &UiCheckpointDto) -> Result<(), CommandError> {
    if checkpoint.schema_version != CHECKPOINT_SCHEMA_VERSION {
        return Err(CommandError::new(
            "unsupported_ui_checkpoint",
            "The device restoration checkpoint uses an unsupported format.",
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
}
