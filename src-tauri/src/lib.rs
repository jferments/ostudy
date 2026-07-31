mod analytics;
mod assets;
mod cards;
mod database;
mod decks;
mod error;
mod logging;
mod paths;
mod practice;
mod scheduler;
mod settings;
mod transfer;
mod util;

use std::io;

use database::Database;
use paths::AppPaths;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_directory = app.path().app_data_dir().map_err(|error| {
                io::Error::other(format!(
                    "Could not resolve the application data directory: {error}"
                ))
            })?;
            let paths = AppPaths::new(app_data_directory).map_err(io::Error::other)?;
            let database = Database::open(paths.database_path()).map_err(io::Error::other)?;

            if !app.manage(paths.clone()) {
                return Err(io::Error::other("Application paths were already initialized.").into());
            }
            if !app.manage(database) {
                return Err(io::Error::other("The database state was already initialized.").into());
            }

            logging::info(
                "application_started",
                format!(
                    "data_directory={} database={}",
                    paths.app_data_dir().display(),
                    paths.database_path().display()
                ),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            decks::list_decks,
            decks::get_deck,
            decks::create_deck,
            decks::update_deck,
            decks::delete_deck,
            cards::list_cards,
            cards::get_card,
            cards::create_card,
            cards::update_card,
            cards::delete_card,
            assets::import_image,
            settings::get_settings,
            settings::save_settings,
            settings::get_app_info,
            practice::start_practice,
            practice::mark_practice_card_viewed,
            practice::mark_practice_card_revealed,
            practice::submit_practice_review,
            practice::complete_practice_session,
            analytics::get_analytics,
            transfer::export_deck,
            transfer::import_deck
        ])
        .run(tauri::generate_context!())
        .expect("error while running ostudy");
}
