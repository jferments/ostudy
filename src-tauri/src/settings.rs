use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database, error::CommandError, paths::AppPaths,
    scheduler::validate_desired_retention, util::unix_time_millis,
};

const THEMES: &[&str] = &["light", "dark", "console", "synthwave"];
const FONT_PRESETS: &[&str] = &["system", "humanist", "serif", "mono"];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    pub theme: String,
    pub ui_font: String,
    pub card_font: String,
    pub ui_scale: i64,
    pub card_base_size: i64,
    pub reduce_motion: bool,
    pub confirm_deletions: bool,
    pub practice_shuffle: bool,
    pub desired_retention: f64,
    pub new_cards_per_day: i64,
    pub reviews_per_day: i64,
    pub struggling_card_threshold: i64,
    pub auto_suspend_struggling_cards: bool,
    pub bury_related_cards: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfoDto {
    pub app_version: String,
    pub data_directory: String,
    pub database_path: String,
    pub image_directory: String,
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
        database_schema_version: schema_version,
    })
}

pub(crate) fn load_settings(connection: &mut Connection) -> Result<AppSettingsDto, CommandError> {
    Ok(AppSettingsDto {
        theme: get_setting(connection, "theme", "dark".to_string())?,
        ui_font: get_setting(connection, "uiFont", "system".to_string())?,
        card_font: get_setting(connection, "cardFont", "system".to_string())?,
        ui_scale: get_setting(connection, "uiScale", 100_i64)?,
        card_base_size: get_setting(connection, "cardBaseSize", 18_i64)?,
        reduce_motion: get_setting(connection, "reduceMotion", false)?,
        confirm_deletions: get_setting(connection, "confirmDeletions", true)?,
        practice_shuffle: get_setting(connection, "practiceShuffle", true)?,
        desired_retention: get_setting(connection, "desiredRetention", 0.90_f64)?,
        new_cards_per_day: get_setting(connection, "newCardsPerDay", 20_i64)?,
        reviews_per_day: get_setting(connection, "reviewsPerDay", 200_i64)?,
        struggling_card_threshold: get_setting(connection, "strugglingCardThreshold", 8_i64)?,
        auto_suspend_struggling_cards: get_setting(
            connection,
            "autoSuspendStrugglingCards",
            false,
        )?,
        bury_related_cards: get_setting(connection, "buryRelatedCards", true)?,
    })
}

fn get_setting<T>(connection: &Connection, key: &str, default: T) -> Result<T, CommandError>
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

    match serialized {
        Some(value) => serde_json::from_str(&value).map_err(CommandError::serialization),
        None => Ok(default),
    }
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
            "reduceMotion",
            serde_json::to_string(&settings.reduce_motion),
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
            "newCardsPerDay",
            serde_json::to_string(&settings.new_cards_per_day),
        ),
        (
            "reviewsPerDay",
            serde_json::to_string(&settings.reviews_per_day),
        ),
        (
            "strugglingCardThreshold",
            serde_json::to_string(&settings.struggling_card_threshold),
        ),
        (
            "autoSuspendStrugglingCards",
            serde_json::to_string(&settings.auto_suspend_struggling_cards),
        ),
        (
            "buryRelatedCards",
            serde_json::to_string(&settings.bury_related_cards),
        ),
    ];

    for (key, serialized) in entries {
        let serialized = serialized.map_err(CommandError::serialization)?;
        transaction
            .execute(
                "INSERT INTO app_settings (key, value_json, updated_at)\n\
                 VALUES (?1, ?2, ?3)\n\
                 ON CONFLICT(key) DO UPDATE SET\n\
                     value_json = excluded.value_json,\n\
                     updated_at = excluded.updated_at",
                params![key, serialized, now],
            )
            .map_err(CommandError::database)?;
    }

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
    validate_desired_retention(settings.desired_retention)?;
    if !(0..=999).contains(&settings.new_cards_per_day) {
        return Err(CommandError::new(
            "invalid_new_card_limit",
            "The daily new-card limit must be between 0 and 999.",
        ));
    }
    if !(0..=5_000).contains(&settings.reviews_per_day) {
        return Err(CommandError::new(
            "invalid_review_limit",
            "The daily review limit must be between 0 and 5,000.",
        ));
    }
    if !(2..=50).contains(&settings.struggling_card_threshold) {
        return Err(CommandError::new(
            "invalid_struggling_threshold",
            "The Struggling-card threshold must be between 2 and 50 misses.",
        ));
    }
    Ok(())
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
    fn loads_defaults_and_persists_phase_two_review_settings() {
        let database = Database::open_in_memory().expect("database should open");
        let mut settings = database
            .with_connection(load_settings)
            .expect("settings should load");
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.new_cards_per_day, 20);
        assert_eq!(settings.reviews_per_day, 200);
        assert_eq!(settings.struggling_card_threshold, 8);
        assert!(settings.bury_related_cards);

        settings.theme = "console".to_string();
        settings.new_cards_per_day = 35;
        settings.auto_suspend_struggling_cards = true;
        database
            .with_connection(|connection| save_settings_in_connection(connection, &settings))
            .expect("settings should save");
        let loaded = database
            .with_connection(load_settings)
            .expect("settings should reload");
        assert_eq!(loaded.theme, "console");
        assert_eq!(loaded.new_cards_per_day, 35);
        assert!(loaded.auto_suspend_struggling_cards);
        assert!((loaded.desired_retention - 0.90).abs() < f64::EPSILON);
    }

    #[test]
    fn rejects_unsafe_custom_fonts() {
        assert!(validate_font("custom:Safe Font").is_ok());
        assert!(validate_font("custom:Bad; font").is_err());
        assert!(validate_font("custom:Bad\nfont").is_err());
    }
}
