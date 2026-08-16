//! Non-blocking Tauri command adapters for complete-library backup and restore.
//!
//! The backup implementation intentionally performs substantial SQLite and
//! filesystem work to produce a coherent package. Running that work directly
//! inside a synchronous Tauri command can block the application event loop long
//! enough for the desktop UI to appear frozen. These adapters preserve the
//! existing transactional implementation while moving the blocking work to the
//! Tauri async runtime's blocking pool.

use tauri::{AppHandle, Manager};

use crate::{
    database::Database,
    error::CommandError,
    library_backup::{
        LibraryRestoreResultDto, PrepareLibraryBackupInput, PrepareLibraryRestoreInput,
        PreparedLibraryBackupDto,
    },
    paths::AppPaths,
};

#[tauri::command]
pub async fn prepare_library_backup(
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
pub async fn prepare_library_restore(
    app: AppHandle,
    input: PrepareLibraryRestoreInput,
) -> Result<LibraryRestoreResultDto, CommandError> {
    run_blocking(app, "library_restore_worker_interrupted", move |app| {
        let database = app.state::<Database>();
        let paths = app.state::<AppPaths>();
        crate::library_backup::prepare_library_restore(database, paths, input)
    })
    .await
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
