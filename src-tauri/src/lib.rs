//! Tauri composition root for the native application.
//!
//! Startup resolves the managed application paths, opens and migrates SQLite,
//! finalizes expired deletion recovery windows, purges finalized card-owned
//! history, reconciles managed images and documents, then publishes Tauri state.
//! A failed durable initialization therefore stops startup instead of exposing
//! commands to a partially reconciled library.

mod ai;
mod analytics;
mod asset_lifecycle;
mod assets;
mod authoring;
mod cards;
mod data_repairs;
mod database;
mod deck_cards;
mod decks;
mod deletion;
mod deletion_commands;
mod deletion_purge;
mod diagnostics;
mod document_backup;
mod document_checkpoints;
mod document_cost;
mod document_delete_reconciliation;
mod document_domain;
mod document_generation;
mod document_ingestion;
mod document_more;
mod document_operations;
mod document_scale;
mod document_semantics;
mod document_store;
mod documents;
mod error;
mod library_backup;
mod logging;
mod paths;
mod pdf_backend;
mod pdf_ocr_backend;
mod pdf_render_backend;
mod practice;
mod practice_selection;
mod projections;
mod schedule_controls;
mod scheduler;
mod settings;
mod study_calendar;
mod studyability;
mod studydeck;
mod task_cancellation;
mod util;
mod workload;

#[cfg(test)]
mod document_backup_tests;
#[cfg(test)]
mod document_pdf_tests;
#[cfg(test)]
mod regression_tests;
#[cfg(test)]
mod stage34_35_regression_tests;
#[cfg(test)]
mod stage36_38_regression_tests;

use std::io;

use ai::AiRuntimeState;
use database::Database;
use paths::AppPaths;
use pdf_render_backend::PdfRenderRuntime;
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
            let resource_directory = app.path().resource_dir().ok();
            let paths = AppPaths::new(app_data_directory).map_err(io::Error::other)?;
            let pending_restore =
                library_backup::apply_pending_restore(&paths).map_err(io::Error::other)?;
            let database = Database::open(paths.database_path()).map_err(io::Error::other)?;
            let ai_runtime = AiRuntimeState::new().map_err(io::Error::other)?;
            let pdf_render_runtime = PdfRenderRuntime::new(resource_directory);

            deletion::finalize_expired_deletions(&database)
                .map_err(|error| io::Error::other(error.message))?;
            deletion_purge::purge_finalized_deletions(&database)
                .map_err(|error| io::Error::other(error.message))?;
            asset_lifecycle::reconcile_startup(&database, &paths)
                .map_err(|error| io::Error::other(error.message))?;
            document_delete_reconciliation::restore_interrupted_deletions(&database, &paths)
                .map_err(|error| io::Error::other(error.message))?;
            documents::reconcile_startup(&database, &paths)
                .map_err(|error| io::Error::other(error.message))?;
            document_operations::reconcile_deleted_document_trash(&paths)
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
            if !app.manage(ai_runtime) {
                return Err(io::Error::other("The AI runtime state was already initialized.").into());
            }
            if !app.manage(pdf_render_runtime) {
                return Err(io::Error::other("The PDF renderer state was already initialized.").into());
            }

            logging::info(
                "application_started",
                format!(
                    "data_directory={} database={} asset_reconciliation=complete document_reconciliation=complete ai_gateway=ready pdf_extraction={} pdf_renderer={}",
                    paths.app_data_dir().display(),
                    paths.database_path().display(),
                    pdf_backend::active_backend_id(),
                    pdf_render_backend::active_backend_id()
                ),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            decks::list_decks,
            decks::get_deck,
            decks::create_deck,
            decks::update_deck,
            decks::set_deck_archived,
            deletion_commands::delete_deck_with_undo,
            deck_cards::list_deck_card_summaries,
            deletion_commands::delete_card_with_undo,
            authoring::card_commands::list_tags,
            authoring::card_commands::search_cards,
            authoring::card_commands::list_matching_cards,
            authoring::card_commands::get_card_note,
            authoring::card_commands::find_duplicate_cards,
            authoring::card_commands::analyze_bulk_cards,
            authoring::card_commands::bulk_create_cards,
            authoring::transfer::transfer_cards,
            authoring::card_commands::bulk_tag_cards,
            deletion_commands::delete_cards_with_undo,
            authoring::card_types_and_templates::create_card_note,
            authoring::card_types_and_templates::update_card_note,
            authoring::card_types_and_templates::templates::list_card_template_summaries,
            authoring::card_types_and_templates::get_card_template,
            authoring::card_types_and_templates::create_card_template,
            authoring::card_types_and_templates::update_card_template,
            authoring::card_types_and_templates::delete_card_template,
            authoring::card_types_and_templates::preview_template_cards,
            authoring::card_types_and_templates::generate_cards_from_template,
            authoring::card_types_and_templates::generated_import::import_generated_flashcards,
            asset_lifecycle::begin_asset_draft,
            asset_lifecycle::cancel_asset_draft,
            asset_lifecycle::sync_asset_draft,
            assets::import_image,
            deletion::undo_deletion,
            diagnostics::record_webview_diagnostic,
            settings::get_settings,
            settings::save_settings,
            settings::get_app_info,
            ai::profiles::get_ai_provider_catalog,
            ai::profiles::list_ai_provider_profiles,
            ai::profiles::get_ai_provider_profile,
            ai::profiles::create_ai_provider_profile,
            ai::profiles::update_ai_provider_profile,
            ai::profiles::delete_ai_provider_profile,
            ai::profiles::set_ai_provider_credential,
            ai::profiles::clear_ai_provider_credential,
            ai::profiles::get_ai_task_defaults,
            ai::profiles::set_ai_task_default,
            ai::probes::list_ai_provider_models,
            ai::probes::test_ai_provider_profile,
            ai::probes::get_ai_provider_probe_result,
            documents::list_documents,
            document_operations::import_pdf_document,
            document_operations::run_pdf_flashcard_generation,
            document_more::generate_more_pdf_flashcards,
            document_operations::cancel_pdf_flashcard_generation,
            document_operations::open_document_source,
            document_operations::delete_pdf_document,
            documents::set_document_archived,
            library_backup::export_library_backup,
            library_backup::prepare_library_restore,
            practice_selection::get_practice_workload,
            practice_selection::start_selected_practice,
            practice_selection::start_card_practice,
            practice_selection::get_practice_card_window,
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
