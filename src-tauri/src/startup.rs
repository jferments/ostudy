use std::{io, path::Path, time::Duration};

use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{
    ai::AiRuntimeState, application_profile, asset_lifecycle, database, deletion, deletion_purge,
    device_sync, document_artifacts, document_delete_reconciliation, document_operations,
    documents, editor_drafts, external_file_recovery, external_files, library_backup, logging,
    paths::AppPaths, pdf_backend, pdf_render_backend::PdfRenderRuntime, platform,
};

#[path = "startup/progress.rs"]
mod progress;
#[path = "startup/upgrade.rs"]
mod upgrade;
#[path = "startup/upgrade_dialog.rs"]
mod upgrade_dialog;

const STARTUP_CONFIGURATION_EXIT_CODE: i32 = 78;
const STARTUP_STAGE_PREFIX: &str = "OSTUDY_STARTUP_STAGE:";

#[cfg(desktop)]
const MAIN_WINDOW_ICON: tauri::image::Image<'static> = tauri::include_image!("./icons/icon.png");

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
    #[cfg(target_os = "linux")]
    if let Err(error) = create_linux_main_window(app) {
        present_startup_failure(app.handle(), &error.to_string());
        return Ok(());
    }

    progress::install(app);

    // Return control to the native event loop before opening/migrating SQLite
    // or decoding/reconciling managed files. The HTML loading shell can paint
    // immediately; NativeStartupGate keeps product commands behind readiness.
    let handle = app.handle().clone();
    let worker = handle.clone();
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            initialize(&worker).map_err(|error| error.to_string())
        })
        .await;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => present_startup_failure(&handle, &error),
            Err(error) => present_startup_failure(&handle, &error.to_string()),
        }
    });
    // Setup errors must be presented explicitly, not propagated as Tauri panics.
    Ok(())
}

fn initialize(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let app_data_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| startup_stage_error("app_data_dir", error))?;
    #[cfg(desktop)]
    let desktop_profile = application_profile::validate_desktop_runtime(
        app.config().identifier.as_str(),
        &app_data_directory,
        cfg!(debug_assertions),
    )
    .map_err(|error| startup_stage_error("application_profile", error))?;
    #[cfg(desktop)]
    apply_main_window_profile(app, desktop_profile);

    crate::library_lock::acquire_for_process(&app_data_directory)
        .map_err(|error| startup_stage_error("library_lock", error))?;
    let resource_directory = app.path().resource_dir().ok();
    let paths = AppPaths::new(app_data_directory)
        .map_err(|error| startup_stage_error("app_paths", error))?;
    logging::initialize(paths.app_data_dir());
    #[cfg(desktop)]
    apply_main_window_icon(app);
    let mut pending_restore = library_backup::apply_pending_restore(&paths)
        .map_err(|error| startup_stage_error("restore_recovery", error))?;
    library_backup::checkpoint::admit_startup(&paths)
        .map_err(|error| startup_stage_error("upgrade_recovery", error))?;
    // Declare the activation guard before the Database: error unwinding closes
    // every SQLite connection before restoring the previous library entries.
    let mut pending_upgrade = None;
    if let Some(plan) = upgrade::inspect(paths.database_path())
        .map_err(|error| startup_stage_error("upgrade_preflight", error))?
    {
        logging::info_target(
            "app.startup",
            "library_upgrade_required",
            "Read-only admission identified a supported library upgrade.",
            serde_json::json!(plan),
        );
        pending_upgrade = Some(
            library_backup::checkpoint::begin_upgrade(
                &paths,
                &app.config().identifier,
                (&plan.source_lineage, plan.source_version),
                &mut |message| report_upgrade_progress(app, message),
            )
            .map_err(|error| startup_stage_error("upgrade_candidate", error))?,
        );
    }
    let upgrading = pending_upgrade.is_some();
    let stage = |normal, during_upgrade| if upgrading { during_upgrade } else { normal };
    let database = database::Database::open(paths.database_path()).map_err(|error| {
        startup_stage_error(stage("database_open", "upgrade_database_open"), error)
    })?;
    let platform_runtime = platform::PlatformRuntime::for_app(app.clone());
    platform::install_global(platform_runtime.clone()).map_err(|error| {
        startup_stage_error(stage("platform_runtime", "upgrade_platform_runtime"), error)
    })?;
    let ai_runtime = AiRuntimeState::new(platform_runtime.secure_secret_store())
        .map_err(|error| startup_stage_error(stage("ai_runtime", "upgrade_ai_runtime"), error))?;
    let pdf_render_runtime = PdfRenderRuntime::new(resource_directory)
        .map_err(|error| startup_stage_error(stage("pdf_runtime", "upgrade_pdf_runtime"), error))?;

    reconcile_library(&database, &paths).map_err(|error| {
        startup_stage_error(
            stage("reconciliation", "upgrade_reconciliation"),
            error.message,
        )
    })?;
    if let Some(restore) = pending_restore.as_mut() {
        restore
            .try_commit()
            .map_err(|error| startup_stage_error("restore_commit", error))?;
    }

    if let Some(upgrade) = pending_upgrade.as_mut() {
        report_upgrade_progress(app, "Verifying library recovery and startup…");
        library_backup::checkpoint::commit_upgrade(&paths, upgrade)
            .map_err(|error| startup_stage_error("upgrade_commit", error))?;
        progress::publish(app, "Library updated; recovery checkpoint retained.", true);
    }
    if let Err(error) =
        library_backup::checkpoint::record_current_build(&paths, &app.config().identifier)
    {
        logging::warn_target(
            "library.upgrade",
            "library_build_receipt_unavailable",
            "The library opened, but its nonsecret build receipt could not be recorded.",
            serde_json::json!({"error": error}),
        );
    }
    #[cfg(desktop)]
    apply_main_window_profile(app, desktop_profile);

    if !app.manage(paths.clone()) {
        return Err(startup_stage_error(
            "state_publish_paths",
            "Application paths were already initialized.",
        )
        .into());
    }
    if !app.manage(platform_runtime.clone()) {
        return Err(startup_stage_error(
            "state_publish_platform",
            "The platform runtime was already initialized.",
        )
        .into());
    }
    if !app.manage(ai_runtime) {
        return Err(startup_stage_error(
            "state_publish_ai",
            "The AI runtime state was already initialized.",
        )
        .into());
    }
    if !app.manage(pdf_render_runtime) {
        return Err(startup_stage_error(
            "state_publish_pdf",
            "The PDF renderer state was already initialized.",
        )
        .into());
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
            "documentArtifactReconciliation": "complete",
            "aiGateway": "ready",
            "pdfExtraction": pdf_backend::active_backend_id(),
            "pdfRenderer": crate::pdf_render_backend::active_backend_id(),
        }),
    );
    // Flush early diagnostics before releasing the existing getSettings-based
    // readiness gate. Database is published LAST so the gate cannot mount the
    // product while another managed runtime or durable reconciliation is missing.
    let _ = logging::flush(Duration::from_millis(500));
    if !app.manage(database) {
        return Err(startup_stage_error(
            "state_publish_database",
            "The database state was already initialized.",
        )
        .into());
    }
    Ok(())
}

fn report_upgrade_progress(app: &tauri::AppHandle, message: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(&format!("oStudy — {message}"));
    }
    progress::publish(app, message, false);
    logging::info_target(
        "library.upgrade",
        "library_upgrade_progress",
        message,
        serde_json::json!({}),
    );
}

fn startup_stage_error(stage: &'static str, error: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("{STARTUP_STAGE_PREFIX}{stage}:{error}"))
}

fn split_startup_stage(error: &str) -> (Option<&str>, &str) {
    let Some(rest) = error.strip_prefix(STARTUP_STAGE_PREFIX) else {
        return (None, error);
    };
    let Some((stage, cause)) = rest.split_once(':') else {
        return (None, error);
    };
    (Some(stage), cause)
}

/// The same durable reconciliation is used by startup and live restoration.
pub(crate) fn reconcile_library(
    database: &database::Database,
    paths: &AppPaths,
) -> Result<(), crate::error::CommandError> {
    device_sync::reconcile_startup(paths)?;
    external_file_recovery::reconcile_interrupted_staging(database, paths)?;
    external_files::reconcile_startup(database, paths)?;
    editor_drafts::reconcile_startup(database)?;
    deletion::finalize_expired_deletions(database)?;
    deletion_purge::purge_finalized_deletions(database)?;
    asset_lifecycle::reconcile_startup(database, paths)?;
    document_delete_reconciliation::restore_interrupted_deletions(database, paths)?;
    documents::reconcile_startup(database, paths)?;
    document_artifacts::reconcile_interrupted_document_artifact_jobs(database)?;
    crate::ai::generation::reconcile_startup(database)?;
    crate::study_materials::reconcile_startup(database)?;
    document_operations::reconcile_deleted_document_trash(paths)?;
    crate::knowledge::reconcile_startup(database)?;
    crate::tutor::reconcile_startup(database)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn create_linux_main_window(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if app.get_webview_window("main").is_some() {
        return Ok(());
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .ok_or_else(|| io::Error::other("The Linux main-window configuration is missing."))?;
    tauri::WebviewWindowBuilder::from_config(app.handle(), config)?
        .icon(MAIN_WINDOW_ICON.clone())?
        .build()?;
    Ok(())
}

#[cfg(desktop)]
fn apply_main_window_profile(
    app: &tauri::AppHandle,
    profile: application_profile::DesktopApplicationProfile,
) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if let Err(error) = window.set_title(profile.window_title()) {
        logging::warn_target(
            "app.startup",
            "application_profile_title_failed",
            "The desktop environment rejected the application profile title.",
            serde_json::json!({
                "profile": profile.as_str(),
                "error": error.to_string(),
            }),
        );
    }
}

#[cfg(desktop)]
fn apply_main_window_icon(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        logging::warn_target(
            "app.startup",
            "main_window_icon_skipped",
            "The main window was unavailable while applying the embedded application icon.",
            serde_json::json!({ "window": "main" }),
        );
        return;
    };
    if let Err(error) = window.set_icon(MAIN_WINDOW_ICON.clone()) {
        logging::warn_target(
            "app.startup",
            "main_window_icon_failed",
            "The desktop environment rejected the embedded application icon.",
            serde_json::json!({ "window": "main", "error": error.to_string() }),
        );
        return;
    }
    logging::info_target(
        "app.startup",
        "main_window_icon_applied",
        "The raccoon mascot icon was applied to the main desktop window.",
        serde_json::json!({ "window": "main", "source": "embedded_raccoon_mascot" }),
    );
}

fn present_startup_failure(app: &tauri::AppHandle, error: &str) {
    let (startup_stage, cause) = split_startup_stage(error);
    if let Some(stage) = startup_stage.filter(|stage| {
        stage.starts_with("upgrade_") && !cause.starts_with(database::PRE_PUBLIC_RESET_PREFIX)
    }) {
        logging::error_target(
            "library.upgrade",
            "library_upgrade_blocked",
            "The library upgrade or recovery stopped before ordinary commands were admitted.",
            serde_json::json!({"startupStage": stage}),
        );
        eprintln!("oStudy library update blocked: {error}");
        let _ = logging::flush(Duration::from_millis(500));
        // These stages are reached only after this process acquired the library
        // lock. A second process or profile failure must never enter recovery UI.
        upgrade_dialog::present(app, stage);
        return;
    }
    let app_data_directory = app.path().app_data_dir().ok();
    let presentation = startup_failure_presentation(cause, app_data_directory.as_deref());

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    match presentation.kind {
        StartupFailureKind::PrePublicDatabase => logging::warn_target(
            "app.startup",
            presentation.event,
            "ostudy startup is blocked because the database is outside the current public lineage.",
            serde_json::json!({
                "reasonCode": presentation.reason_code,
                "startupStage": startup_stage,
            }),
        ),
        StartupFailureKind::Internal => logging::error_target(
            "app.startup",
            presentation.event,
            "ostudy could not finish native startup.",
            serde_json::json!({
                "reasonCode": presentation.reason_code,
                "startupStage": startup_stage,
            }),
        ),
    }
    let _ = logging::flush(Duration::from_millis(500));

    match presentation.kind {
        StartupFailureKind::PrePublicDatabase => {
            eprintln!(
                "ostudy startup blocked: pre-public data requires manual recovery; the database was left unchanged."
            );
        }
        StartupFailureKind::Internal => eprintln!("ostudy startup blocked: {error}"),
    }
    let handle = app.clone();
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
    if error.starts_with(crate::library_lock::IN_USE) {
        return StartupFailurePresentation {
            kind: StartupFailureKind::Internal,
            event: "application_startup_blocked",
            reason_code: "library_already_open",
            title: "This library is already open",
            message: "Another oStudy process owns this library. Close that instance before reopening it. The library and any pending restore were left unchanged. Do not delete the ownership lock file.".to_string(),
        };
    }
    if error.starts_with(database::PRE_PUBLIC_RESET_PREFIX) {
        let location = app_data_directory
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<application data directory>".to_string());
        #[cfg(not(mobile))]
        let recovery_instructions = format!(
            "Automatic pre-public conversion has been retired from normal startup. The database was left unchanged.\n\nTo explicitly recover an exact final-v26 library in place from a source checkout, run:\n\nbash ./scripts/transition-pre-public-data.sh --data-dir \"{location}\" --confirm\n\nTo recover a separate archived v26 directory, add:\n\n--source-backup \"<timestamped pre-public backup directory>\"\n\nTo intentionally archive this library and start empty instead, run:\n\n./scripts/reset-pre-public-data.sh --data-dir \"{location}\" --confirm-empty-library"
        );
        #[cfg(mobile)]
        let recovery_instructions = format!(
            "This database is outside the current public schema lineage. Automatic pre-public conversion is no longer part of the application runtime. Preserve the application storage before clearing or reinstalling the development build.\n\nApplication data directory: {location}"
        );
        return StartupFailurePresentation {
            kind: StartupFailureKind::PrePublicDatabase,
            event: "application_startup_blocked",
            reason_code: "pre_public_database_requires_manual_recovery",
            title: "Pre-public data needs manual recovery",
            message: format!(
                "This build uses ostudy's current public database lineage. The existing non-public database was left unchanged.\n\n{recovery_instructions}"
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
            .contains("transition-pre-public-data.sh"));
        assert!(presentation.message.contains("confirm-empty-library"));
        assert!(presentation
            .message
            .contains("/tmp/ostudy-public-baseline-test"));
    }

    #[test]
    fn an_existing_library_owner_has_actionable_non_destructive_guidance() {
        let presentation = startup_failure_presentation(crate::library_lock::IN_USE, None);
        assert_eq!(presentation.reason_code, "library_already_open");
        assert!(presentation.message.contains("Do not delete"));
        assert!(!presentation.message.contains("reset-pre-public"));
    }

    #[test]
    fn startup_stage_annotation_exposes_only_the_safe_stage() {
        let error = startup_stage_error(
            "database_open",
            "sensitive filesystem/provider detail should stay out of structured fields",
        )
        .to_string();
        let (stage, cause) = split_startup_stage(&error);
        assert_eq!(stage, Some("database_open"));
        assert!(cause.contains("sensitive filesystem/provider detail"));
        let presentation = startup_failure_presentation(cause, None);
        assert_eq!(presentation.reason_code, "native_startup_failed");
        assert!(!presentation
            .message
            .contains("sensitive filesystem/provider detail"));
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
