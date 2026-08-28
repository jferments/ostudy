//! Small device-local UI restoration checkpoint.
//!
//! This file is deliberately separate from SQLite and `.ostudybackup`: it
//! contains only presentation destinations and resumable practice hints, never
//! card content, managed files, credentials, learner history, or runtime route
//! request identities.

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

const CHECKPOINT_SCHEMA_VERSION: i64 = 2;
const MAX_CHECKPOINT_BYTES: usize = 64 * 1024;
const DEVICE_STATE_DIRECTORY: &str = "device-state";
const CHECKPOINT_V1_FILENAME: &str = "ui-checkpoint-v1.json";
const CHECKPOINT_V2_FILENAME: &str = "ui-checkpoint-v2.json";
const FLASHCARDS_WORKSPACES: &[&str] = &["decks", "templates", "search", "transfer"];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum CheckpointSection {
    Decks,
    StudyGuides,
    Documents,
    Analytics,
    Settings,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CheckpointReturnTarget {
    pub section: CheckpointSection,
    pub flashcards_workspace: Option<String>,
    pub deck_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PracticeCheckpoint {
    pub session_id: String,
    pub position: i64,
    pub revealed: bool,
    pub return_target: CheckpointReturnTarget,
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
    pub deck_id: Option<String>,
    pub editor_draft_id: Option<String>,
    pub practice: Option<PracticeCheckpoint>,
    pub analytics: Option<AnalyticsCheckpoint>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum LegacyCheckpointSection {
    Decks,
    Documents,
    Cards,
    Templates,
    Practice,
    Analytics,
    Settings,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyPracticeCheckpoint {
    session_id: String,
    position: i64,
    revealed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyUiCheckpoint {
    schema_version: i64,
    updated_at: i64,
    section: LegacyCheckpointSection,
    flashcards_workspace: Option<String>,
    deck_id: Option<String>,
    editor_draft_id: Option<String>,
    practice: Option<LegacyPracticeCheckpoint>,
    analytics: Option<AnalyticsCheckpoint>,
}

#[tauri::command]
pub fn get_ui_checkpoint(
    paths: State<'_, AppPaths>,
) -> Result<Option<UiCheckpointDto>, CommandError> {
    read_ui_checkpoint(paths.inner())
}

#[tauri::command]
pub fn save_ui_checkpoint(
    paths: State<'_, AppPaths>,
    input: SaveUiCheckpointInput,
) -> Result<UiCheckpointDto, CommandError> {
    save_ui_checkpoint_at(paths.inner(), input)
}

#[tauri::command]
pub fn clear_ui_checkpoint(paths: State<'_, AppPaths>) -> Result<(), CommandError> {
    clear_ui_checkpoint_at(paths.inner())
}

fn read_ui_checkpoint(paths: &AppPaths) -> Result<Option<UiCheckpointDto>, CommandError> {
    let current_path = checkpoint_path(paths, CHECKPOINT_V2_FILENAME);
    if let Some(bytes) = read_bounded_file(&current_path)? {
        let checkpoint = match serde_json::from_slice::<UiCheckpointDto>(&bytes) {
            Ok(checkpoint) if validate_checkpoint(&checkpoint).is_ok() => checkpoint,
            _ => {
                quarantine_invalid_checkpoint(paths, &current_path, 2)?;
                remove_file_if_exists(&checkpoint_path(paths, CHECKPOINT_V1_FILENAME))?;
                return Ok(None);
            }
        };
        return Ok(Some(checkpoint));
    }

    let legacy_path = checkpoint_path(paths, CHECKPOINT_V1_FILENAME);
    let Some(bytes) = read_bounded_file(&legacy_path)? else {
        return Ok(None);
    };
    let legacy = match serde_json::from_slice::<LegacyUiCheckpoint>(&bytes) {
        Ok(checkpoint) if validate_legacy_checkpoint(&checkpoint).is_ok() => checkpoint,
        _ => {
            quarantine_invalid_checkpoint(paths, &legacy_path, 1)?;
            return Ok(None);
        }
    };
    let checkpoint = normalize_legacy_checkpoint(legacy);
    validate_checkpoint(&checkpoint)?;
    write_checkpoint(paths, &checkpoint)?;
    remove_file_if_exists(&legacy_path)?;
    Ok(Some(checkpoint))
}

fn save_ui_checkpoint_at(
    paths: &AppPaths,
    input: SaveUiCheckpointInput,
) -> Result<UiCheckpointDto, CommandError> {
    let checkpoint = UiCheckpointDto {
        schema_version: CHECKPOINT_SCHEMA_VERSION,
        updated_at: unix_time_millis()?,
        section: input.section,
        flashcards_workspace: input.flashcards_workspace,
        deck_id: input.deck_id,
        editor_draft_id: input.editor_draft_id,
        practice: input.practice,
        analytics: input.analytics,
    };
    validate_checkpoint(&checkpoint)?;
    write_checkpoint(paths, &checkpoint)?;
    remove_file_if_exists(&checkpoint_path(paths, CHECKPOINT_V1_FILENAME))?;
    Ok(checkpoint)
}

fn clear_ui_checkpoint_at(paths: &AppPaths) -> Result<(), CommandError> {
    remove_file_if_exists(&checkpoint_path(paths, CHECKPOINT_V2_FILENAME))?;
    remove_file_if_exists(&checkpoint_path(paths, CHECKPOINT_V1_FILENAME))
}

fn read_bounded_file(path: &Path) -> Result<Option<Vec<u8>>, CommandError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(CommandError::io(error)),
    };
    if bytes.is_empty() || bytes.len() > MAX_CHECKPOINT_BYTES {
        return Ok(Some(Vec::new()));
    }
    Ok(Some(bytes))
}

fn write_checkpoint(paths: &AppPaths, checkpoint: &UiCheckpointDto) -> Result<(), CommandError> {
    let encoded = serde_json::to_vec(checkpoint).map_err(CommandError::serialization)?;
    if encoded.len() > MAX_CHECKPOINT_BYTES {
        return Err(CommandError::new(
            "ui_checkpoint_too_large",
            "The device restoration checkpoint is larger than the supported limit.",
        ));
    }

    let path = checkpoint_path(paths, CHECKPOINT_V2_FILENAME);
    let parent = path.parent().ok_or_else(|| {
        CommandError::new(
            "invalid_ui_checkpoint_path",
            "The device restoration directory is invalid.",
        )
    })?;
    fs::create_dir_all(parent).map_err(CommandError::io)?;
    let temporary = parent.join(format!("{CHECKPOINT_V2_FILENAME}.partial"));
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
    sync_directory(parent)
}

fn normalize_legacy_checkpoint(mut legacy: LegacyUiCheckpoint) -> UiCheckpointDto {
    if legacy.flashcards_workspace.as_deref() == Some("create") {
        legacy.flashcards_workspace = Some("decks".to_string());
        legacy.deck_id = None;
        legacy.editor_draft_id = None;
    }

    let (section, flashcards_workspace, deck_id, editor_draft_id) = match legacy.section {
        LegacyCheckpointSection::Practice => (
            CheckpointSection::Decks,
            Some("decks".to_string()),
            None,
            None,
        ),
        LegacyCheckpointSection::Cards => (
            CheckpointSection::Decks,
            Some("search".to_string()),
            None,
            legacy.editor_draft_id,
        ),
        LegacyCheckpointSection::Templates => (
            CheckpointSection::Decks,
            Some("templates".to_string()),
            None,
            legacy.editor_draft_id,
        ),
        LegacyCheckpointSection::Decks => (
            CheckpointSection::Decks,
            legacy
                .flashcards_workspace
                .or_else(|| Some("decks".to_string())),
            legacy.deck_id,
            legacy.editor_draft_id,
        ),
        LegacyCheckpointSection::Documents => (
            CheckpointSection::Documents,
            None,
            None,
            legacy.editor_draft_id,
        ),
        LegacyCheckpointSection::Analytics => (
            CheckpointSection::Analytics,
            None,
            None,
            legacy.editor_draft_id,
        ),
        LegacyCheckpointSection::Settings => (
            CheckpointSection::Settings,
            None,
            None,
            legacy.editor_draft_id,
        ),
    };
    let practice = legacy.practice.map(|practice| PracticeCheckpoint {
        session_id: practice.session_id,
        position: practice.position,
        revealed: practice.revealed,
        return_target: CheckpointReturnTarget {
            section: CheckpointSection::Decks,
            flashcards_workspace: Some("decks".to_string()),
            deck_id: None,
        },
    });
    UiCheckpointDto {
        schema_version: CHECKPOINT_SCHEMA_VERSION,
        updated_at: legacy.updated_at,
        section,
        flashcards_workspace,
        deck_id,
        editor_draft_id,
        practice,
        analytics: legacy.analytics,
    }
}

fn validate_legacy_checkpoint(checkpoint: &LegacyUiCheckpoint) -> Result<(), CommandError> {
    if checkpoint.schema_version != 1 {
        return Err(unsupported_checkpoint());
    }
    if checkpoint
        .flashcards_workspace
        .as_deref()
        .is_some_and(|workspace| {
            workspace != "create" && !FLASHCARDS_WORKSPACES.contains(&workspace)
        })
    {
        return Err(invalid_checkpoint(
            "The restored Flashcards workspace is invalid.",
        ));
    }
    validate_optional_id(checkpoint.deck_id.as_deref(), "deck")?;
    validate_optional_id(checkpoint.editor_draft_id.as_deref(), "editor draft")?;
    if let Some(practice) = &checkpoint.practice {
        validate_practice_hint(&practice.session_id, practice.position)?;
    }
    validate_analytics(checkpoint.analytics.as_ref())
}

fn validate_checkpoint(checkpoint: &UiCheckpointDto) -> Result<(), CommandError> {
    if checkpoint.schema_version != CHECKPOINT_SCHEMA_VERSION {
        return Err(unsupported_checkpoint());
    }
    validate_route_fields(
        checkpoint.section,
        checkpoint.flashcards_workspace.as_deref(),
        checkpoint.deck_id.as_deref(),
    )?;
    validate_optional_id(checkpoint.editor_draft_id.as_deref(), "editor draft")?;
    if let Some(practice) = &checkpoint.practice {
        validate_practice_hint(&practice.session_id, practice.position)?;
        validate_route_fields(
            practice.return_target.section,
            practice.return_target.flashcards_workspace.as_deref(),
            practice.return_target.deck_id.as_deref(),
        )?;
    }
    validate_analytics(checkpoint.analytics.as_ref())
}

fn validate_route_fields(
    section: CheckpointSection,
    workspace: Option<&str>,
    deck_id: Option<&str>,
) -> Result<(), CommandError> {
    if section == CheckpointSection::Decks {
        if workspace.is_some_and(|workspace| !FLASHCARDS_WORKSPACES.contains(&workspace)) {
            return Err(invalid_checkpoint(
                "The restored Flashcards workspace is invalid.",
            ));
        }
        validate_optional_id(deck_id, "deck")?;
    } else if workspace.is_some() || deck_id.is_some() {
        return Err(invalid_checkpoint(
            "Only Flashcards destinations may contain workspace or deck state.",
        ));
    }
    Ok(())
}

fn validate_practice_hint(session_id: &str, position: i64) -> Result<(), CommandError> {
    validate_entity_id(session_id, "practice session")?;
    if !(0..=1_000_000).contains(&position) {
        return Err(invalid_checkpoint(
            "The restored practice position is invalid.",
        ));
    }
    Ok(())
}

fn validate_analytics(analytics: Option<&AnalyticsCheckpoint>) -> Result<(), CommandError> {
    let Some(analytics) = analytics else {
        return Ok(());
    };
    if !matches!(analytics.range.as_str(), "7d" | "30d" | "90d" | "all") {
        return Err(invalid_checkpoint(
            "The restored analytics range is invalid.",
        ));
    }
    validate_id_list(&analytics.deck_ids, "analytics deck")?;
    validate_id_list(&analytics.tag_ids, "analytics tag")
}

fn unsupported_checkpoint() -> CommandError {
    CommandError::new(
        "unsupported_ui_checkpoint",
        "The device restoration checkpoint uses an unsupported format.",
    )
}

fn invalid_checkpoint(message: &str) -> CommandError {
    CommandError::new("invalid_ui_checkpoint", message)
}

fn validate_optional_id(value: Option<&str>, label: &str) -> Result<(), CommandError> {
    if let Some(value) = value {
        validate_entity_id(value, label)?;
    }
    Ok(())
}

fn validate_id_list(values: &[String], label: &str) -> Result<(), CommandError> {
    if values.len() > 2_000 {
        return Err(invalid_checkpoint(
            "The device restoration checkpoint contains too many identifiers.",
        ));
    }
    for value in values {
        validate_entity_id(value, label)?;
    }
    Ok(())
}

fn checkpoint_path(paths: &AppPaths, filename: &str) -> PathBuf {
    paths
        .app_data_dir()
        .join(DEVICE_STATE_DIRECTORY)
        .join(filename)
}

fn quarantine_invalid_checkpoint(
    paths: &AppPaths,
    path: &Path,
    schema_version: i64,
) -> Result<(), CommandError> {
    let quarantine = paths
        .app_data_dir()
        .join(DEVICE_STATE_DIRECTORY)
        .join(format!(
            "invalid-ui-checkpoint-v{schema_version}.json.quarantine"
        ));
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

    fn fixture(name: &str) -> &'static [u8] {
        match name {
            "v1-resumable-practice" => {
                include_bytes!("../tests/fixtures/ui-checkpoints/v1-resumable-practice.json")
            }
            "v1-setup-only-practice" => {
                include_bytes!("../tests/fixtures/ui-checkpoints/v1-setup-only-practice.json")
            }
            "v2-study-guides-practice-return" => include_bytes!(
                "../tests/fixtures/ui-checkpoints/v2-study-guides-practice-return.json"
            ),
            _ => panic!("unknown fixture"),
        }
    }

    fn test_paths(label: &str) -> AppPaths {
        let root = std::env::temp_dir().join(format!(
            "ostudy-ui-checkpoint-{label}-{}-{}",
            std::process::id(),
            unix_time_millis().expect("clock")
        ));
        AppPaths::new(root).expect("temporary app paths")
    }

    fn write_fixture(paths: &AppPaths, filename: &str, bytes: &[u8]) {
        let path = checkpoint_path(paths, filename);
        fs::create_dir_all(path.parent().expect("checkpoint parent")).expect("state directory");
        fs::write(path, bytes).expect("checkpoint fixture");
    }

    fn cleanup(paths: &AppPaths) {
        fs::remove_dir_all(paths.app_data_dir()).expect("temporary app data cleanup");
    }

    #[test]
    fn released_v1_resumable_practice_migrates_without_losing_session_data() {
        let legacy: LegacyUiCheckpoint =
            serde_json::from_slice(fixture("v1-resumable-practice")).expect("released v1 fixture");
        let checkpoint = normalize_legacy_checkpoint(legacy);
        let practice = checkpoint.practice.expect("resumable session");
        assert_eq!(checkpoint.schema_version, 2);
        assert_eq!(checkpoint.section, CheckpointSection::Decks);
        assert_eq!(practice.session_id, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(practice.position, 4);
        assert!(practice.revealed);
        assert_eq!(practice.return_target.section, CheckpointSection::Decks);
        assert_eq!(
            practice.return_target.flashcards_workspace.as_deref(),
            Some("decks")
        );
    }

    #[test]
    fn released_v1_setup_only_practice_normalizes_to_flashcards() {
        let legacy: LegacyUiCheckpoint =
            serde_json::from_slice(fixture("v1-setup-only-practice")).expect("released v1 fixture");
        let checkpoint = normalize_legacy_checkpoint(legacy);
        assert_eq!(checkpoint.section, CheckpointSection::Decks);
        assert_eq!(checkpoint.flashcards_workspace.as_deref(), Some("decks"));
        assert_eq!(checkpoint.practice, None);
    }

    #[test]
    fn released_v1_aliases_and_removed_create_workspace_keep_safe_fallbacks() {
        for (section, expected_workspace) in [
            (LegacyCheckpointSection::Cards, "search"),
            (LegacyCheckpointSection::Templates, "templates"),
        ] {
            let checkpoint = normalize_legacy_checkpoint(LegacyUiCheckpoint {
                schema_version: 1,
                updated_at: 1,
                section,
                flashcards_workspace: None,
                deck_id: None,
                editor_draft_id: None,
                practice: None,
                analytics: None,
            });
            assert_eq!(checkpoint.section, CheckpointSection::Decks);
            assert_eq!(
                checkpoint.flashcards_workspace.as_deref(),
                Some(expected_workspace)
            );
        }

        let checkpoint = normalize_legacy_checkpoint(LegacyUiCheckpoint {
            schema_version: 1,
            updated_at: 1,
            section: LegacyCheckpointSection::Decks,
            flashcards_workspace: Some("create".to_string()),
            deck_id: Some("d".repeat(32)),
            editor_draft_id: Some("e".repeat(32)),
            practice: None,
            analytics: None,
        });
        assert_eq!(checkpoint.flashcards_workspace.as_deref(), Some("decks"));
        assert_eq!(checkpoint.deck_id, None);
        assert_eq!(checkpoint.editor_draft_id, None);
    }

    #[test]
    fn canonical_v2_round_trips_study_guides_and_practice_return_target() {
        let checkpoint: UiCheckpointDto =
            serde_json::from_slice(fixture("v2-study-guides-practice-return")).expect("v2 fixture");
        validate_checkpoint(&checkpoint).expect("valid v2 fixture");
        let encoded = serde_json::to_vec(&checkpoint).expect("encode checkpoint");
        assert_eq!(
            serde_json::from_slice::<UiCheckpointDto>(&encoded).expect("decode checkpoint"),
            checkpoint
        );
    }

    #[test]
    fn checkpoint_rejects_sensitive_or_unbounded_shapes_through_strict_deserialization() {
        let mut value: serde_json::Value =
            serde_json::from_slice(fixture("v2-study-guides-practice-return"))
                .expect("fixture JSON");
        value["credential"] = serde_json::Value::String("secret".to_string());
        assert!(serde_json::from_value::<UiCheckpointDto>(value).is_err());

        let mut legacy: serde_json::Value =
            serde_json::from_slice(fixture("v1-resumable-practice")).expect("legacy fixture JSON");
        legacy["requestId"] = serde_json::Value::Number(9.into());
        assert!(serde_json::from_value::<LegacyUiCheckpoint>(legacy).is_err());
    }

    #[test]
    fn invalid_v2_does_not_resurrect_an_older_v1_checkpoint() {
        let paths = test_paths("precedence");
        write_fixture(&paths, CHECKPOINT_V2_FILENAME, b"{not-json");
        write_fixture(
            &paths,
            CHECKPOINT_V1_FILENAME,
            fixture("v1-resumable-practice"),
        );
        assert_eq!(read_ui_checkpoint(&paths).expect("checkpoint read"), None);
        assert!(!checkpoint_path(&paths, CHECKPOINT_V1_FILENAME).exists());
        assert_eq!(
            read_ui_checkpoint(&paths).expect("second checkpoint read"),
            None
        );
        cleanup(&paths);
    }

    #[test]
    fn reading_v1_persists_v2_and_retires_the_legacy_file() {
        let paths = test_paths("retirement");
        write_fixture(
            &paths,
            CHECKPOINT_V1_FILENAME,
            fixture("v1-resumable-practice"),
        );
        let checkpoint = read_ui_checkpoint(&paths)
            .expect("checkpoint read")
            .expect("migrated checkpoint");
        assert!(checkpoint.practice.is_some());
        assert!(checkpoint_path(&paths, CHECKPOINT_V2_FILENAME).exists());
        assert!(!checkpoint_path(&paths, CHECKPOINT_V1_FILENAME).exists());
        cleanup(&paths);
    }

    #[test]
    fn clear_removes_both_checkpoint_generations() {
        let paths = test_paths("clear");
        write_fixture(
            &paths,
            CHECKPOINT_V1_FILENAME,
            fixture("v1-setup-only-practice"),
        );
        write_fixture(
            &paths,
            CHECKPOINT_V2_FILENAME,
            fixture("v2-study-guides-practice-return"),
        );
        clear_ui_checkpoint_at(&paths).expect("clear checkpoint generations");
        assert_eq!(read_ui_checkpoint(&paths).expect("checkpoint read"), None);
        cleanup(&paths);
    }

    #[test]
    fn checkpoint_rejects_non_flashcards_route_fields() {
        let mut checkpoint: UiCheckpointDto =
            serde_json::from_slice(fixture("v2-study-guides-practice-return"))
                .expect("fixture JSON");
        checkpoint.deck_id = Some("d".repeat(32));
        assert_eq!(
            validate_checkpoint(&checkpoint).unwrap_err().code,
            "invalid_ui_checkpoint"
        );
    }
}
