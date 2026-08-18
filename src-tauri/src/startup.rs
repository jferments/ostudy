use std::{io, path::Path, time::Duration};

use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{
    ai::AiRuntimeState, asset_lifecycle, database, deletion, deletion_purge, device_sync,
    document_delete_reconciliation, document_operations, documents, editor_drafts,
    external_file_recovery, external_files, library_backup, logging, paths::AppPaths, pdf_backend,
    pdf_render_backend::PdfRenderRuntime, platform,
};

mod pre_public_transition;

const STARTUP_CONFIGURATION_EXIT_CODE: i32 = 78;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StartupFailureKind {
    PrePublicDatabase,
    Internal,
}

#[derive(Debug, Eq, PartialEq)]
struct StartupFailurePresentation {
    kind: StartupFailureKind,
    event: &'static str,
    reason_code: &'static str,
    title: &'static str,
    message: String,
}

pub(crate) fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(error) = initialize(app) {
        present_startup_failure(app, &error.to_string());
    }
    // Tauri treats an Err returned by the setup hook as a panic. Startup
    // compatibility and operational failures are therefore presented and the
    // process is exited explicitly instead of being propagated through setup.
    Ok(())
}

fn initialize(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_data_directory = app.path().app_data_dir().map_err(|error| {
        io::Error::other(format!(
            "Could not resolve the application data directory: {error}"
        ))
    })?;
    let resource_directory = app.path().resource_dir().ok();
    let paths = AppPaths::new(app_data_directory).map_err(io::Error::other)?;
    logging::initialize(paths.app_data_dir());
    let pending_restore =
        library_backup::apply_pending_restore(&paths).map_err(io::Error::other)?;
    let database = open_database_with_pre_public_adoption(&paths)?;
    let platform_runtime = platform::PlatformRuntime::for_app(app.handle().clone());
    platform::install_global(platform_runtime.clone()).map_err(io::Error::other)?;
    let ai_runtime =
        AiRuntimeState::new(platform_runtime.secure_secret_store()).map_err(io::Error::other)?;
    let pdf_render_runtime = PdfRenderRuntime::new(resource_directory)
        .map_err(|error| io::Error::other(error.to_string()))?;

    device_sync::reconcile_startup(&paths).map_err(|error| io::Error::other(error.message))?;
    external_file_recovery::reconcile_interrupted_staging(&database, &paths)
        .map_err(|error| io::Error::other(error.message))?;
    external_files::reconcile_startup(&database, &paths)
        .map_err(|error| io::Error::other(error.message))?;
    editor_drafts::reconcile_startup(&database).map_err(|error| io::Error::other(error.message))?;
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
    if !app.manage(platform_runtime.clone()) {
        return Err(io::Error::other("The platform runtime was already initialized.").into());
    }
    if !app.manage(ai_runtime) {
        return Err(io::Error::other("The AI runtime state was already initialized.").into());
    }
    if !app.manage(pdf_render_runtime) {
        return Err(io::Error::other("The PDF renderer state was already initialized.").into());
    }

    let lifecycle = platform_runtime
        .lifecycle()
        .transition(platform::LifecyclePhase::Foreground);
    let _ = app.emit("ostudy-lifecycle", lifecycle);

    logging::info_target(
        "app.startup",
        "application_started",
        "ostudy completed durable startup and reconciliation.",
        serde_json::json!({
            "databaseFile": "library.sqlite3",
            "assetReconciliation": "complete",
            "documentReconciliation": "complete",
            "aiGateway": "ready",
            "pdfExtraction": pdf_backend::active_backend_id(),
            "pdfRenderer": crate::pdf_render_backend::active_backend_id(),
        }),
    );
    Ok(())
}

fn open_database_with_pre_public_adoption(paths: &AppPaths) -> Result<database::Database, io::Error> {
    match database::Database::open(paths.database_path()) {
        Ok(database) => Ok(database),
        Err(error) if error.starts_with(database::PRE_PUBLIC_RESET_PREFIX) => {
            match pre_public_transition::adopt_final_pre_public_database(paths.database_path()) {
                Ok(Some(report)) => {
                    logging::debug_target(
                        "database.migration",
                        "pre_public_database_adoption_verified",
                        "The adopted pre-public database is being reopened through the public migration verifier.",
                        serde_json::json!({
                            "backupFile": report.backup_path.file_name().and_then(|value| value.to_str()).unwrap_or("pre-public.backup"),
                            "deckCount": report.deck_count,
                            "cardCount": report.card_count,
                            "reviewCount": report.review_count,
                        }),
                    );
                    database::Database::open(paths.database_path()).map_err(|retry_error| {
                        io::Error::other(format!(
                            "{}: The pre-public data was adopted and backed up, but the public database verifier rejected the result: {retry_error}",
                            database::PRE_PUBLIC_RESET_PREFIX
                        ))
                    })
                }
                Ok(None) => Err(io::Error::other(error)),
                Err(adoption_error) => Err(io::Error::other(format!(
                    "{}: {adoption_error}",
                    database::PRE_PUBLIC_RESET_PREFIX
                ))),
            }
        }
        Err(error) => Err(io::Error::other(error)),
    }
}

fn present_startup_failure(app: &mut tauri::App, error: &str) {
    let app_data_directory = app.path().app_data_dir().ok();
    let presentation = startup_failure_presentation(error, app_data_directory.as_deref());

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    match presentation.kind {
        StartupFailureKind::PrePublicDatabase => logging::warn_target(
            "app.startup",
            presentation.event,
            "ostudy startup is blocked because the pre-public database could not be adopted automatically.",
            serde_json::json!({ "reasonCode": presentation.reason_code }),
        ),
        StartupFailureKind::Internal => logging::error_target(
            "app.startup",
            presentation.event,
            "ostudy could not complete native startup.",
            serde_json::json!({ "reasonCode": presentation.reason_code }),
        ),
    }
    let _ = logging::flush(Duration::from_millis(500));

    match presentation.kind {
        StartupFailureKind::PrePublicDatabase => {
            eprintln!("ostudy startup blocked: pre-public data requires manual recovery; the database was left unchanged.");
        }
        StartupFailureKind::Internal => eprintln!("ostudy startup blocked: {error}"),
    }
    let handle = app.handle().clone();
    let kind = match presentation.kind {
        StartupFailureKind::PrePublicDatabase => MessageDialogKind::Warning,
        StartupFailureKind::Internal => MessageDialogKind::Error,
    };
    app.dialog()
        .message(presentation.message)
        .title(presentation.title)
        .kind(kind)
        .buttons(MessageDialogButtons::Ok)
        .show(move |_| handle.exit(STARTUP_CONFIGURATION_EXIT_CODE));
}

fn startup_failure_presentation(
    error: &str,
    app_data_directory: Option<&Path>,
) -> StartupFailurePresentation {
    if error.starts_with(database::PRE_PUBLIC_RESET_PREFIX) {
        let location = app_data_directory
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<application data directory>".to_string());
        #[cfg(not(mobile))]
        let recovery_instructions = format!(
            "The exact final pre-public schema is normally adopted automatically with a consistent SQLite backup. This database did not match that verified transition contract.\n\nIf you previously ran the old reset command, restore its timestamped backup with:\n\npython3 ./scripts/restore_pre_public_backup.py --backup-dir \"<timestamped pre-public backup directory>\" --data-dir \"{location}\" --confirm\n\nTo intentionally archive this library and start empty instead, run:\n\n./scripts/reset-pre-public-data.sh --data-dir \"{location}\" --confirm-empty-library"
        );
        #[cfg(mobile)]
        let recovery_instructions = format!(
            "This pre-release database does not match the verified final pre-public transition contract. Preserve the application storage before clearing or reinstalling the development build.\n\nApplication data directory: {location}"
        );
        return StartupFailurePresentation {
            kind: StartupFailureKind::PrePublicDatabase,
            event: "application_startup_blocked",
            reason_code: "pre_public_database_requires_manual_recovery",
            title: "Pre-public data needs manual recovery",
            message: format!(
                "This build uses ostudy's public database baseline. The existing pre-public database could not be adopted automatically and was left unchanged.\n\n{recovery_instructions}"
            ),
        };
    }

    StartupFailurePresentation {
        kind: StartupFailureKind::Internal,
        event: "application_startup_failed",
        reason_code: "native_startup_failed",
        title: "ostudy could not start",
        message: "ostudy could not finish native startup. Your library was not intentionally reset. Review the local diagnostics and restart the application after correcting the reported problem.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_pre_public_database_is_a_manual_recovery_block() {
        let presentation = startup_failure_presentation(
            &format!("{}: legacy database", database::PRE_PUBLIC_RESET_PREFIX),
            Some(Path::new("/tmp/ostudy-public-baseline-test")),
        );
        assert_eq!(presentation.kind, StartupFailureKind::PrePublicDatabase);
        assert_eq!(presentation.event, "application_startup_blocked");
        assert_eq!(
            presentation.reason_code,
            "pre_public_database_requires_manual_recovery"
        );
        assert!(presentation
            .message
            .contains("restore_pre_public_backup.py"));
        assert!(presentation.message.contains("confirm-empty-library"));
        assert!(presentation
            .message
            .contains("/tmp/ostudy-public-baseline-test"));
    }

    #[test]
    fn unexpected_native_startup_failure_stays_generic() {
        let presentation =
            startup_failure_presentation("Could not initialize a native service", None);
        assert_eq!(presentation.kind, StartupFailureKind::Internal);
        assert_eq!(presentation.event, "application_startup_failed");
        assert_eq!(presentation.reason_code, "native_startup_failed");
        assert!(!presentation.message.contains("Could not initialize"));
    }
}
