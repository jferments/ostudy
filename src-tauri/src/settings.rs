use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database, error::CommandError, paths::AppPaths,
    scheduler::validate_desired_retention, util::unix_time_millis,
};

const THEMES: &[&str] = &["light", "dark", "console", "synthwave"];
const FONT_PRESETS: &[&str] = &[
    "system",
    "atkinson",
    "plex",
    "albert",
    "space-grotesk",
    "spline-sans",
    "sora",
    "roboto",
    "roboto-slab",
    "sofia-sans",
    "lato",
    "ubuntu",
    "humanist",
    "serif",
    "mono",
];
const MOTION_PREFERENCES: &[&str] = &["system", "full", "reduced", "none"];
const MATERIAL_PREFERENCES: &[&str] = &["automatic", "rich", "simple"];
const DECK_VIEW_DENSITIES: &[&str] = &["comfortable", "compact"];
const AI_REASONING_EFFORTS: &[&str] = &[
    "automatic",
    "none",
    "minimal",
    "low",
    "medium",
    "high",
    "xhigh",
    "max",
];
const DEFAULT_AI_REASONING_EFFORT: &str = "low";
const MAX_DAILY_LIMIT: i64 = 10_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettingsDto {
    pub theme: String,
    pub ui_font: String,
    pub card_font: String,
    pub ui_scale: i64,
    pub card_base_size: i64,
    pub motion_preference: String,
    pub material_preference: String,
    pub decorative_artwork: bool,
    pub celebrations_enabled: bool,
    pub tutorials_enabled: bool,
    pub deck_view_density: String,
    pub confirm_deletions: bool,
    pub practice_shuffle: bool,
    pub desired_retention: f64,
    pub daily_new_limit: i64,
    pub daily_review_limit: i64,
    pub ai_reasoning_effort: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub app_version: String,
    pub data_directory: String,
    pub database_path: String,
    pub image_directory: String,
    pub diagnostic_log_path: String,
    pub database_schema_version: i64,
}

#[tauri::command]
pub fn get_settings(database: State<'_, Database>) -> Result<AppSettingsDto, CommandError> {
    database.with_connection(load_settings)
}

#[tauri::command]
pub fn save_settings(
    database: State<'_, Database>,
    input: AppSettingsDto,
) -> Result<AppSettingsDto, CommandError> {
    validate_settings(&input)?;
    database.with_connection(|connection| save_settings_in_connection(connection, &input))?;
    Ok(input)
}

#[tauri::command]
pub fn get_app_info(
    database: State<'_, Database>,
    paths: State<'_, AppPaths>,
) -> Result<AppInfoDto, CommandError> {
    let schema_version = database.with_connection(|connection| {
        connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(CommandError::database)
    })?;

    Ok(AppInfoDto {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        data_directory: paths.app_data_dir().to_string_lossy().into_owned(),
        database_path: paths.database_path().to_string_lossy().into_owned(),
        image_directory: paths.images_dir().to_string_lossy().into_owned(),
        diagnostic_log_path: crate::logging::diagnostic_log_path(paths.app_data_dir())
            .to_string_lossy()
            .into_owned(),
        database_schema_version: schema_version,
    })
}

pub(crate) fn load_settings(connection: &mut Connection) -> Result<AppSettingsDto, CommandError> {
    let motion_preference = match get_optional_setting::<String>(connection, "motionPreference")? {
        Some(value) => value,
        None => legacy_motion_preference(connection)?,
    };
    Ok(AppSettingsDto {
        theme: get_setting(connection, "theme", "dark".to_string())?,
        ui_font: get_font_setting(connection, "uiFont")?,
        card_font: get_font_setting(connection, "cardFont")?,
        ui_scale: get_setting(connection, "uiScale", 100_i64)?,
        card_base_size: get_setting(connection, "cardBaseSize", 18_i64)?,
        motion_preference,
        material_preference: get_setting(
            connection,
            "materialPreference",
            "automatic".to_string(),
        )?,
        decorative_artwork: get_setting(connection, "decorativeArtwork", true)?,
        celebrations_enabled: get_setting(connection, "celebrationsEnabled", true)?,
        tutorials_enabled: get_setting(connection, "tutorialsEnabled", true)?,
        deck_view_density: get_setting(connection, "deckViewDensity", "comfortable".to_string())?,
        confirm_deletions: get_setting(connection, "confirmDeletions", true)?,
        practice_shuffle: get_setting(connection, "practiceShuffle", true)?,
        desired_retention: get_setting(connection, "desiredRetention", 0.90_f64)?,
        daily_new_limit: get_setting(connection, "dailyNewLimit", 20_i64)?,
        daily_review_limit: get_setting(connection, "dailyReviewLimit", 200_i64)?,
        ai_reasoning_effort: load_ai_reasoning_effort(connection)?,
    })
}

pub(crate) fn load_ai_reasoning_effort(connection: &Connection) -> Result<String, CommandError> {
    let value = get_setting(
        connection,
        "aiReasoningEffort",
        DEFAULT_AI_REASONING_EFFORT.to_string(),
    )?;
    validate_choice(
        &value,
        AI_REASONING_EFFORTS,
        "invalid_ai_reasoning_effort",
        "Select an available document AI reasoning level.",
    )?;
    Ok(value)
}

fn legacy_motion_preference(connection: &Connection) -> Result<String, CommandError> {
    let animations_enabled = get_setting(connection, "animationsEnabled", true)?;
    let reduce_motion = get_setting(connection, "reduceMotion", false)?;
    Ok(if !animations_enabled {
        "none"
    } else if reduce_motion {
        "reduced"
    } else {
        "system"
    }
    .to_string())
}

fn get_font_setting(connection: &Connection, key: &str) -> Result<String, CommandError> {
    let row: Option<(String, i64)> = connection
        .query_row(
            "SELECT value_json, updated_at FROM app_settings WHERE key = ?1",
            [key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((serialized, updated_at)) = row else {
        return Ok("plex".to_string());
    };
    let value: String = serde_json::from_str(&serialized).map_err(CommandError::serialization)?;
    // Migration 0002 seeded system fonts at timestamp zero. That sentinel lets
    // the new default become Plex without overwriting an explicit user choice:
    // every real settings save writes a nonzero wall-clock timestamp.
    if updated_at == 0 && value == "system" {
        Ok("plex".to_string())
    } else {
        Ok(value)
    }
}

fn get_setting<T>(connection: &Connection, key: &str, default: T) -> Result<T, CommandError>
where
    T: DeserializeOwned,
{
    get_optional_setting(connection, key).map(|value| value.unwrap_or(default))
}

fn get_optional_setting<T>(connection: &Connection, key: &str) -> Result<Option<T>, CommandError>
where
    T: DeserializeOwned,
{
    let serialized: Option<String> = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::database)?;
    serialized
        .map(|value| serde_json::from_str(&value).map_err(CommandError::serialization))
        .transpose()
}

fn save_settings_in_connection(
    connection: &mut Connection,
    settings: &AppSettingsDto,
) -> Result<(), CommandError> {
    let now = unix_time_millis()?;
    let transaction = connection.transaction().map_err(CommandError::database)?;
    let entries = [
        ("theme", serde_json::to_string(&settings.theme)),
        ("uiFont", serde_json::to_string(&settings.ui_font)),
        ("cardFont", serde_json::to_string(&settings.card_font)),
        ("uiScale", serde_json::to_string(&settings.ui_scale)),
        (
            "cardBaseSize",
            serde_json::to_string(&settings.card_base_size),
        ),
        (
            "motionPreference",
            serde_json::to_string(&settings.motion_preference),
        ),
        (
            "materialPreference",
            serde_json::to_string(&settings.material_preference),
        ),
        (
            "decorativeArtwork",
            serde_json::to_string(&settings.decorative_artwork),
        ),
        (
            "celebrationsEnabled",
            serde_json::to_string(&settings.celebrations_enabled),
        ),
        (
            "tutorialsEnabled",
            serde_json::to_string(&settings.tutorials_enabled),
        ),
        (
            "deckViewDensity",
            serde_json::to_string(&settings.deck_view_density),
        ),
        (
            "confirmDeletions",
            serde_json::to_string(&settings.confirm_deletions),
        ),
        (
            "practiceShuffle",
            serde_json::to_string(&settings.practice_shuffle),
        ),
        (
            "desiredRetention",
            serde_json::to_string(&settings.desired_retention),
        ),
        (
            "dailyNewLimit",
            serde_json::to_string(&settings.daily_new_limit),
        ),
        (
            "dailyReviewLimit",
            serde_json::to_string(&settings.daily_review_limit),
        ),
        (
            "aiReasoningEffort",
            serde_json::to_string(&settings.ai_reasoning_effort),
        ),
    ];
    for (key, serialized) in entries {
        let serialized = serialized.map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO app_settings (key, value_json, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET
                     value_json = excluded.value_json,
                     updated_at = excluded.updated_at",
                params![key, serialized, now],
            )
            .map_err(CommandError::database)?;
    }
    transaction
        .execute(
            "DELETE FROM app_settings WHERE key IN ('animationsEnabled', 'reduceMotion')",
            [],
        )
        .map_err(CommandError::database)?;
    transaction.commit().map_err(CommandError::database)
}

fn validate_settings(settings: &AppSettingsDto) -> Result<(), CommandError> {
    if !THEMES.contains(&settings.theme.as_str()) {
        return Err(CommandError::new(
            "invalid_theme",
            "Select one of the available application themes.",
        ));
    }
    validate_font(&settings.ui_font)?;
    validate_font(&settings.card_font)?;
    validate_choice(
        &settings.motion_preference,
        MOTION_PREFERENCES,
        "invalid_motion_preference",
        "Select an available motion preference.",
    )?;
    validate_choice(
        &settings.material_preference,
        MATERIAL_PREFERENCES,
        "invalid_material_preference",
        "Select an available material preference.",
    )?;
    validate_choice(
        &settings.deck_view_density,
        DECK_VIEW_DENSITIES,
        "invalid_deck_view_density",
        "Select an available deck-list density.",
    )?;
    validate_choice(
        &settings.ai_reasoning_effort,
        AI_REASONING_EFFORTS,
        "invalid_ai_reasoning_effort",
        "Select an available document AI reasoning level.",
    )?;
    if !(80..=140).contains(&settings.ui_scale) {
        return Err(CommandError::new(
            "invalid_ui_scale",
            "Interface scale must be between 80% and 140%.",
        ));
    }
    if !(12..=32).contains(&settings.card_base_size) {
        return Err(CommandError::new(
            "invalid_card_size",
            "Default card text size must be between 12 and 32 pixels.",
        ));
    }
    if !(0..=MAX_DAILY_LIMIT).contains(&settings.daily_new_limit)
        || !(0..=MAX_DAILY_LIMIT).contains(&settings.daily_review_limit)
    {
        return Err(CommandError::new(
            "invalid_daily_limit",
            format!("Daily card limits must be between 0 and {MAX_DAILY_LIMIT}."),
        ));
    }
    validate_desired_retention(settings.desired_retention)
}

fn validate_choice(
    value: &str,
    choices: &[&str],
    code: &str,
    message: &str,
) -> Result<(), CommandError> {
    if choices.contains(&value) {
        Ok(())
    } else {
        Err(CommandError::new(code, message))
    }
}

fn validate_font(value: &str) -> Result<(), CommandError> {
    if FONT_PRESETS.contains(&value) {
        return Ok(());
    }
    if let Some(custom) = value.strip_prefix("custom:") {
        let valid = !custom.trim().is_empty()
            && custom.len() <= 200
            && !custom.chars().any(|character| {
                character.is_control()
                    || matches!(character, ';' | '{' | '}' | '<' | '>' | '\\' | '/')
            });
        if valid {
            return Ok(());
        }
    }
    Err(CommandError::new(
        "invalid_font",
        "Select a built-in font or enter a valid installed font family.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_persists_workload_defaults() {
        let database = Database::open_in_memory().expect("database should open");
        let mut settings = database
            .with_connection(load_settings)
            .expect("settings should load");
        assert_eq!(settings.ui_font, "plex");
        assert_eq!(settings.card_font, "plex");
        assert_eq!(settings.daily_new_limit, 20);
        assert_eq!(settings.daily_review_limit, 200);
        assert_eq!(settings.ai_reasoning_effort, "low");
        settings.daily_new_limit = 12;
        settings.daily_review_limit = 80;
        settings.ai_reasoning_effort = "none".to_string();
        database
            .with_connection(|connection| save_settings_in_connection(connection, &settings))
            .expect("settings should save");
        let loaded = database
            .with_connection(load_settings)
            .expect("settings should reload");
        assert_eq!(loaded.daily_new_limit, 12);
        assert_eq!(loaded.daily_review_limit, 80);
        assert_eq!(loaded.ai_reasoning_effort, "none");
    }

    #[test]
    fn explicit_system_font_choice_survives_the_new_default() {
        let database = Database::open_in_memory().expect("database should open");
        let mut settings = database
            .with_connection(load_settings)
            .expect("settings should load");
        assert_eq!(settings.ui_font, "plex");
        assert_eq!(settings.card_font, "plex");
        settings.ui_font = "system".to_string();
        settings.card_font = "system".to_string();
        database
            .with_connection(|connection| save_settings_in_connection(connection, &settings))
            .expect("explicit font settings should save");
        let loaded = database
            .with_connection(load_settings)
            .expect("settings should reload");
        assert_eq!(loaded.ui_font, "system");
        assert_eq!(loaded.card_font, "system");
    }

    #[test]
    fn accepts_bundled_font_presets() {
        for preset in [
            "plex",
            "albert",
            "space-grotesk",
            "spline-sans",
            "sora",
            "roboto",
            "roboto-slab",
            "sofia-sans",
            "lato",
            "ubuntu",
        ] {
            assert!(
                validate_font(preset).is_ok(),
                "preset {preset} should be valid"
            );
        }
    }

    #[test]
    fn migrates_legacy_animation_preferences_when_new_setting_is_absent() {
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {
                connection
                    .execute_batch(
                        "DELETE FROM app_settings WHERE key = 'motionPreference';
                         INSERT INTO app_settings (key, value_json, updated_at)
                         VALUES
                             ('animationsEnabled', 'true', 1),
                             ('reduceMotion', 'true', 1)
                         ON CONFLICT(key) DO UPDATE SET
                             value_json = excluded.value_json,
                             updated_at = excluded.updated_at;",
                    )
                    .map_err(CommandError::database)?;
                let loaded = load_settings(connection)?;
                assert_eq!(loaded.motion_preference, "reduced");
                save_settings_in_connection(connection, &loaded)?;
                let legacy_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM app_settings
                         WHERE key IN ('animationsEnabled', 'reduceMotion')",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(legacy_count, 0);
                Ok(())
            })
            .expect("legacy motion settings should migrate");
    }

    #[test]
    fn rejects_unsafe_custom_fonts() {
        assert!(validate_font("custom:Safe Font").is_ok());
        assert!(validate_font("custom:Bad; font").is_err());
        assert!(validate_font("custom:Bad\nfont").is_err());
    }
}

#[cfg(test)]
include!("settings/extended_tests.rs");