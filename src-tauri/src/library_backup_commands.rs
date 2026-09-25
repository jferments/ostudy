//! Non-blocking Tauri command adapters for complete-library backup and restore.
//!
//! The backup implementation intentionally performs substantial SQLite and
//! filesystem work to produce a coherent package. Running that work directly
//! inside a synchronous Tauri command can block the application event loop long
//! enough for the desktop UI to appear frozen. These adapters preserve the
//! existing transactional implementation while moving the blocking work to the
//! Tauri async runtime's blocking pool.

use tauri::{AppHandle, Emitter, Manager};

use crate::{
    database::Database,
    error::CommandError,
    library_backup::{
        live::LiveRestoreResult, PrepareLibraryBackupInput, PrepareLibraryRestoreInput,
        PreparedLibraryBackupDto,
    },
    paths::AppPaths,
};

#[tauri::command]
pub async fn prepare_library_backup_async(
    app: AppHandle,
    input: PrepareLibraryBackupInput,
) -> Result<PreparedLibraryBackupDto, CommandError> {
    run_blocking(app, "library_backup_worker_interrupted", move |app| {
        let database = app.state::<Database>();
        let paths = app.state::<AppPaths>();
        crate::library_backup::prepare_library_backup(database, paths, input)
    })
    .await
}

#[tauri::command]
pub async fn prepare_library_restore_async(
    app: AppHandle,
    input: PrepareLibraryRestoreInput,
    library_generation: Option<u64>,
) -> Result<LiveRestoreResult, CommandError> {
    let events = app.clone();
    let result = run_blocking(app, "library_restore_worker_interrupted", move |app| {
        let database = app.state::<Database>();
        let paths = app.state::<AppPaths>();
        crate::library_backup::live::restore(
            &database,
            &paths,
            input,
            library_generation,
            || app.state::<crate::ai::AiRuntimeState>().clear_session_credentials(),
        )
    })
    .await;
    // A lost IPC response does not hide committed activation from other views.
    // The payload contains counts/operation identity, never learner text/keys.
    if let Ok(restored) = &result {
        let _ = events.emit("ostudy-library-restored", restored);
    } else if let Err(error) = &result {
        if error.code == "library_restore_rolled_back" {
            let _ = events.emit(
                "ostudy-library-restore-rolled-back",
                serde_json::json!({ "message": error.message }),
            );
        }
    }
    result
}

async fn run_blocking<T, F>(
    app: AppHandle,
    error_code: &'static str,
    work: F,
) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce(AppHandle) -> Result<T, CommandError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || work(app))
        .await
        .map_err(|error| {
            CommandError::new(
                error_code,
                format!("The background library transfer worker stopped unexpectedly: {error}"),
            )
        })?
}
