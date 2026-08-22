//! Tauri composition root for the native application.
//!
//! Startup resolves the managed application paths, opens and migrates SQLite,
//! finalizes expired deletion recovery windows, purges finalized card-owned
//! history, reconciles managed images and documents, then publishes Tauri state.
//! A failed durable initialization therefore stops startup instead of exposing
//! commands to a partially reconciled library.

mod ai;
mod analytics;
mod anydoc_document_adapter;
mod asset_lifecycle;
mod asset_protocol;
mod assets;
mod authoring;
mod cards;
mod data_repairs;
mod database;
mod deck_cards;
mod deck_folders;
mod decks;
mod deletion;
mod deletion_commands;
mod deletion_purge;
mod device_sync;
mod device_sync_qr_commands;
mod diagnostics;
mod document_backup;
mod document_canonical;
mod document_canonical_store;
mod document_checkpoints;
mod document_cost;
mod document_delete_reconciliation;
mod document_domain;
mod document_generation;
#[path = "document_adapters.rs"]
mod document_ingestion;
mod document_more;
mod document_operations;
mod document_progress;
mod document_scale;
mod document_semantics;
mod document_store;
mod documents;
mod editor_drafts;
mod error;
mod external_file_commands;
mod external_file_recovery;
mod external_file_streaming;
mod external_files;
mod library_backup;
mod library_backup_commands;
mod logging;
mod paths;
mod pdf_backend;
#[path = "document_ingestion.rs"]
mod pdf_document_ingestion;
mod pdf_ocr_backend;
#[cfg_attr(mobile, allow(dead_code))]
mod pdf_render_backend;
pub mod platform;
mod practice;
mod practice_resume;
mod practice_selection;
mod projections;
mod schedule_controls;
mod scheduler;
mod settings;
mod startup;
mod study_calendar;
mod studyability;
mod studydeck;
mod system_resources;
mod task_cancellation;
mod ui_checkpoint;
mod util;
mod workload;

#[cfg(test)]
mod canonical_migration_tests;
#[cfg(test)]
mod deck_folders_backup_tests;
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

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

static CLOSE_CONFIRMATION_OPEN: AtomicBool = AtomicBool::new(false);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::install_panic_hook();
    let builder = tauri::Builder::default()
        .register_uri_scheme_protocol("ostudy-asset", asset_protocol::respond)
        .plugin(tauri_plugin_ostudy_platform::init());
    #[cfg(mobile)]
    let builder = builder
        .plugin(tauri_plugin_ostudy_platform::init_documents())
        .plugin(tauri_plugin_ostudy_platform::init_qr());

    let result = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(startup::setup)
        .on_window_event(|window, event| {
            let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                return;
            };
            // A startup compatibility block intentionally exits before durable
            // platform state is managed. Do not turn that controlled path into
            // a second panic while Tauri processes window shutdown events.
            let Some(platform_runtime) = window.try_state::<platform::PlatformRuntime>() else {
                return;
            };
            let lifecycle = platform_runtime.lifecycle();
            let requested = lifecycle.transition(platform::LifecyclePhase::TerminationRequested);
            let _ = window.emit("ostudy-lifecycle", requested);
            if window.label() != "main" || !task_cancellation::has_active_document_tasks() {
                let terminating = lifecycle.transition(platform::LifecyclePhase::Terminating);
                let _ = window.emit("ostudy-lifecycle", terminating);
                return;
            }

            api.prevent_close();
            if CLOSE_CONFIRMATION_OPEN.swap(true, Ordering::AcqRel) {
                return;
            }

            let window = window.clone();
            let lifecycle_after = lifecycle.clone();
            window
                .dialog()
                .message(
                    "There is a current PDF processing job underway. If you close ostudy now, the running request will be interrupted and any progress that has not been committed will be lost. Are you sure you want to proceed?",
                )
                .title("PDF processing is still running")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "Close ostudy".to_string(),
                    "Keep working".to_string(),
                ))
                .show(move |should_close| {
                    CLOSE_CONFIRMATION_OPEN.store(false, Ordering::Release);
                    if should_close {
                        let terminating = lifecycle_after.transition(platform::LifecyclePhase::Terminating);
                        let _ = window.emit("ostudy-lifecycle", terminating);
                        let _ = window.destroy();
                    } else {
                        let foreground = lifecycle_after.transition(platform::LifecyclePhase::Foreground);
                        let _ = window.emit("ostudy-lifecycle", foreground);
                    }
                });
        })
        .invoke_handler(tauri::generate_handler![
            decks::list_decks,
            decks::get_deck,
            decks::create_deck,
            decks::update_deck,
            decks::set_deck_archived,
            deck_folders::get_deck_library_organization,
            deck_folders::create_deck_folder,
            deck_folders::update_deck_folder,
            deck_folders::delete_deck_folder,
            deck_folders::move_deck_in_library,
            deck_folders::move_deck_folder,
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
            diagnostics::record_webview_diagnostic_batch,
            diagnostics::get_diagnostic_configuration,
            diagnostics::set_diagnostic_filter,
            diagnostics::flush_diagnostics,
            settings::get_settings,
            settings::save_settings,
            settings::get_app_info,
            platform::get_platform_capabilities,
            platform::set_platform_back_interception,
            device_sync::get_device_sync_state,
            device_sync::update_device_sync_name,
            device_sync::start_device_pairing,
            device_sync::join_device_pairing,
            device_sync::cancel_device_sync_session,
            device_sync_qr_commands::scan_device_pairing_qr_async,
            device_sync::remove_device_sync_peer,
            device_sync::resolve_device_sync_conflict,
            ui_checkpoint::get_ui_checkpoint,
            ui_checkpoint::save_ui_checkpoint,
            ui_checkpoint::clear_ui_checkpoint,
            external_file_streaming::stream_select_and_stage_external_import,
            external_files::release_staged_external_import,
            external_file_streaming::stream_publish_prepared_export,
            external_file_commands::share_prepared_export,
            external_files::release_prepared_export,
            external_file_commands::list_incoming_shares,
            external_file_commands::stage_incoming_share,
            external_file_commands::dismiss_incoming_share,
            editor_drafts::open_editor_draft,
            editor_drafts::save_editor_draft,
            editor_drafts::discard_editor_draft,
            editor_drafts::commit_editor_draft,
            editor_drafts::list_editor_drafts,
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
            document_progress::get_document_generation_progress,
            document_operations::open_document_source,
            document_operations::delete_pdf_document,
            documents::set_document_archived,
            library_backup_commands::prepare_library_backup_async,
            library_backup_commands::prepare_library_restore_async,
            practice_selection::get_practice_workload,
            practice_selection::start_selected_practice,
            practice_selection::start_card_practice,
            practice_resume::resume_practice_session,
            practice_selection::get_practice_card_window,
            practice_selection::refresh_practice_after_card_edit,
            practice::mark_practice_card_viewed,
            practice::mark_practice_card_revealed,
            practice::submit_practice_review,
            practice::complete_practice_session,
            schedule_controls::apply_schedule_action,
            analytics::get_analytics,
            analytics::get_recent_practice_reviews,
            studydeck::prepare_deck_export,
            studydeck::import_deck
        ])
        .run(tauri::generate_context!());
    logging::shutdown();
    result.expect("error while running ostudy");
}
