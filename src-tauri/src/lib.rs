//! Tauri composition root for the native application.
//!
//! Startup resolves the managed application paths, opens and migrates SQLite,
//! finalizes expired deletion recovery windows, and reconciles the asset ledger
//! with the managed filesystem before publishing any Tauri state. A failed
//! durable initialization therefore stops startup instead of exposing commands
//! to a partially reconciled library.

mod analytics;
mod asset_lifecycle;
mod assets;
mod authoring;
mod card_projections;
mod cards;
mod data_repairs;
mod database;
mod decks;
mod deletion;
mod deletion_commands;
mod error;
mod historical_attribution;
mod learning_signals;
mod library_backup;
mod logging;
mod paths;
mod practice;
mod practice_selection;
mod schedule_controls;
mod scheduler;
mod settings;
mod study_calendar;
mod studyability;
mod studydeck;
mod template_projections;
mod util;
mod workload;

#[cfg(test)]
mod regression_tests;
#[cfg(test)]
mod stage34_35_regression_tests;

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
            let pending_restore =
                library_backup::apply_pending_restore(&paths).map_err(io::Error::other)?;
            let database = Database::open(paths.database_path()).map_err(io::Error::other)?;

            // A prepared restore is not considered successful until the restored
            // database has migrated and its deletion/asset reconciliation passes.
            // If any step below fails, the guard restores the prior database and
            // asset directory after the open database connection is dropped.
            deletion::finalize_expired_deletions(&database)
                .map_err(|error| io::Error::other(error.message))?;
            asset_lifecycle::reconcile_startup(&database, &paths)
                .map_err(|error| io::Error::other(error.message))?;
            if let Some(restore) = pending_restore {
                restore.commit();
            }

            if !app.manage(paths.clone()) {
                return Err(io::Error::other("Application paths were already initialized.").into());
            }
            if !app.manage(database) {
                return Err(io::Error::other("The database state was already initialized.").into());
            }

            logging::info(
                "application_started",
                format!(
                    "data_directory={} database={} asset_reconciliation=complete",
                    paths.app_data_dir().display(),
                    paths.database_path().display()
                ),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            decks::list_decks,
            decks::list_archived_decks,
            decks::get_deck,
            decks::create_deck,
            decks::update_deck,
            decks::archive_deck,
            decks::unarchive_deck,
            deletion_commands::delete_deck_with_undo,
            card_projections::list_deck_cards,
            deletion_commands::delete_card_with_undo,
            authoring::list_tags,
            authoring::search_cards,
            authoring::list_matching_cards,
            authoring::get_organized_card,
            authoring::create_organized_card,
            authoring::update_organized_card,
            authoring::find_duplicate_cards,
            authoring::analyze_bulk_cards,
            authoring::bulk_create_cards,
            // Registered externally as the stable `transfer_cards` IPC command.
            authoring::transfer_generated_cards,
            authoring::bulk_tag_cards,
            deletion_commands::delete_cards_with_undo,
            authoring::get_card_type_info,
            authoring::create_generated_card,
            authoring::update_generated_card,
            template_projections::list_card_template_summaries,
            authoring::get_card_template,
            authoring::create_card_template,
            authoring::update_card_template,
            authoring::delete_card_template,
            authoring::preview_template_cards,
            authoring::generate_cards_from_template,
            authoring::import_generated_flashcards,
            asset_lifecycle::begin_asset_draft,
            asset_lifecycle::cancel_asset_draft,
            asset_lifecycle::sync_asset_draft,
            assets::import_image,
            deletion::undo_deletion,
            settings::get_settings,
            settings::save_settings,
            settings::get_app_info,
            library_backup::export_library_backup,
            library_backup::prepare_library_restore,
            practice_selection::get_practice_workload,
            practice_selection::get_practice_card_window,
            practice_selection::start_selected_practice,
            practice_selection::start_card_practice,
            practice::mark_practice_card_viewed,
            practice::mark_practice_card_revealed,
            practice::submit_practice_review,
            practice::complete_practice_session,
            schedule_controls::apply_schedule_action,
            analytics::get_analytics,
            analytics::get_recent_practice_reviews,
            studydeck::export_deck,
            studydeck::import_deck
        ])
        .run(tauri::generate_context!())
        .expect("error while running ostudy");
}
