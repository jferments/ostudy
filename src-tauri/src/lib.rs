mod analytics;
mod assets;
mod authoring;
mod cards;
mod database;
mod decks;
mod error;
mod logging;
mod paths;
mod practice;
mod practice_selection;
mod scheduler;
mod settings;
mod tagged_transfer;
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
            cards::delete_card,
            authoring::list_tags,
            authoring::search_cards,
            authoring::list_matching_cards,
            authoring::get_organized_card,
            authoring::create_organized_card,
            authoring::update_organized_card,
            authoring::find_duplicate_cards,
            authoring::analyze_bulk_cards,
            authoring::bulk_create_cards,
            authoring::transfer_cards,
            authoring::bulk_tag_cards,
            authoring::delete_cards,
            authoring::get_card_type_info,
            authoring::create_generated_card,
            authoring::update_generated_card,
            authoring::list_card_templates,
            authoring::get_card_template,
            authoring::create_card_template,
            authoring::update_card_template,
            authoring::delete_card_template,
            authoring::preview_template_cards,
            authoring::generate_cards_from_template,
            assets::import_image,
            settings::get_settings,
            settings::save_settings,
            settings::get_app_info,
            practice_selection::start_selected_practice,
            practice_selection::start_card_practice,
            practice::mark_practice_card_viewed,
            practice::mark_practice_card_revealed,
            practice::submit_practice_review,
            practice::complete_practice_session,
            analytics::get_analytics,
            tagged_transfer::export_deck_with_tags,
            tagged_transfer::import_deck_with_tags
        ])
        .run(tauri::generate_context!())
        .expect("error while running ostudy");
}
