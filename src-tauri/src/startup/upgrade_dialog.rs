//! Native recovery UI before the normal library/UI command gate is available.
//! Filesystem work stays on blocking workers. A confirmation is bound to the
//! displayed operation identity; an old dialog cannot replace a newer intent.

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{library_backup::checkpoint, paths::AppPaths};

use super::STARTUP_CONFIGURATION_EXIT_CODE;

pub(super) fn present(app: &tauri::AppHandle, stage: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let details = app.path().app_data_dir()
        .map_err(|error| error.to_string())
        .and_then(|root| AppPaths::new(root).map_err(|error| error.to_string()))
        .and_then(|paths| checkpoint::recovery_details(&paths));
    match details {
        Ok(Some(details)) if !details.activation_unresolved => {
            let message = recovery_message(&details);
            let handle = app.clone();
            if details.checkpoint_available && !details.previous_restored {
                app.dialog().message(message).title("Library update needs attention")
                    .kind(MessageDialogKind::Warning)
                    .buttons(MessageDialogButtons::OkCancelCustom("Recovery options".into(), "Close oStudy".into()))
                    .show(move |open| {
                        if open { choose_recovery(&handle, details.operation_id); }
                        else { handle.exit(STARTUP_CONFIGURATION_EXIT_CODE); }
                    });
            } else {
                prompt_retry(&handle, details.operation_id, message);
            }
        }
        Ok(Some(details)) => close_message(app, "Library recovery is incomplete", recovery_message(&details)),
        Ok(None) => {
            let message = if stage == "upgrade_preflight" {
                "The installed version could not safely admit this library. No library migration was attempted. Use a compatible oStudy version and preserve the library and its journals. Local diagnostics contain the failed stage."
            } else {
                "oStudy could not prepare the library update. No library was intentionally reset. Preserve the library, its journals and recovery files; local diagnostics identify the failed stage."
            };
            close_message(app, "Library update could not start", message.into());
        }
        Err(error) => {
            crate::logging::error_target("library.upgrade", "upgrade_recovery_metadata_invalid",
                "The library recovery UI could not validate its operation metadata.", serde_json::json!({"error": error}));
            close_message(app, "Library recovery needs inspection", "Recovery metadata could not be verified. All files were preserved. Do not delete journals or reset the library. Inspect local diagnostics before retrying.".into());
        }
    }
}

fn recovery_message(details: &checkpoint::RecoveryDetails) -> String {
    if details.activation_unresolved {
        "The activation decision could not be completed or recovered safely. Close oStudy and reopen it to run journal recovery. Do not install an older binary while an activation journal is unresolved, and do not delete recovery files.".into()
    } else if details.previous_restored {
        "The previous library checkpoint has been restored. To use it without upgrading, close oStudy and install its compatible previous application package. This executable will not automatically upgrade the restored library again. You may explicitly allow another upgrade attempt instead.".into()
    } else {
        "The library update did not complete normally. The previous library and available checkpoint/candidate files have been retained. A retry will not happen without your decision. You can inspect diagnostics, keep the existing library, or restore the pre-update checkpoint when available. oStudy does not install or downgrade application packages for you.".into()
    }
}

fn choose_recovery(app: &tauri::AppHandle, operation: String) {
    let handle = app.clone();
    app.dialog()
        .message("Restore the verified pre-update checkpoint, or choose a retry on the next launch. Restoring changes the library, not the installed application package.")
        .title("Choose library recovery")
        .buttons(MessageDialogButtons::OkCancelCustom("Restore checkpoint…".into(), "Retry or close…".into()))
        .show(move |restore| {
            if restore { confirm_restore(&handle, operation); }
            else { prompt_retry(&handle, operation, "Allow one new upgrade attempt the next time you open oStudy? The previous attempt and all recovery files will be retained. Nothing is migrated in this dialog.".into()); }
        });
}

fn confirm_restore(app: &tauri::AppHandle, operation: String) {
    let handle = app.clone();
    app.dialog()
        .message("Restore the pre-update library? Any work made after this checkpoint will no longer be in the active library. The currently active library will also be preserved in the recovery transaction. After restoration, oStudy will close; install a compatible previous application package to use the old library. No reverse SQL migration or application downgrade will run.")
        .title("Restore previous library")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Restore previous library".into(), "Cancel".into()))
        .show(move |confirmed| {
            if confirmed { run_recovery(&handle, operation, true); }
            else { handle.exit(STARTUP_CONFIGURATION_EXIT_CODE); }
        });
}

fn prompt_retry(app: &tauri::AppHandle, operation: String, message: String) {
    let handle = app.clone();
    app.dialog().message(message).title("Library update recovery")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("Allow retry on next launch".into(), "Close oStudy".into()))
        .show(move |confirmed| {
            if confirmed { run_recovery(&handle, operation, false); }
            else { handle.exit(STARTUP_CONFIGURATION_EXIT_CODE); }
        });
}

fn run_recovery(app: &tauri::AppHandle, operation: String, restore: bool) {
    let handle = app.clone();
    let worker = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            let root = worker.path().app_data_dir().map_err(|error| error.to_string())?;
            let paths = AppPaths::new(root).map_err(|error| error.to_string())?;
            if restore {
                if let Some(window) = worker.get_webview_window("main") { let _ = window.show(); }
                checkpoint::restore_checkpoint(&paths, &operation, &mut |message| super::report_upgrade_progress(&worker, message))
            } else {
                checkpoint::allow_retry(&paths, &operation)
            }
        }).await;
        match result {
            Ok(Ok(())) if restore => close_message(&handle, "Previous library restored", "The pre-update library was restored and the replaced library was preserved. oStudy will close now. Install the compatible previous application package before using this library; this new executable will not silently re-upgrade it.".into()),
            Ok(Ok(())) => close_message(&handle, "Retry allowed", "The failed attempt and checkpoint were retained. Close this dialog, then reopen oStudy when you are ready for a new upgrade attempt.".into()),
            result => {
                let error = match result {
                    Ok(Err(error)) => error,
                    Err(error) => error.to_string(),
                    Ok(Ok(())) => unreachable!(),
                };
                crate::logging::error_target("library.upgrade", "explicit_upgrade_recovery_failed",
                    "The explicit recovery action did not complete.", serde_json::json!({"error": error}));
                close_message(&handle, "Recovery could not complete", "Recovery did not complete. All recovery files were preserved. Close oStudy and inspect diagnostics; any unresolved activation journal will be recovered on the next launch. Do not delete or reset the library.".into());
            }
        }
    });
}

fn close_message(app: &tauri::AppHandle, title: &str, message: String) {
    let handle = app.clone();
    app.dialog().message(message).title(title).kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::Ok).show(move |_| handle.exit(STARTUP_CONFIGURATION_EXIT_CODE));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_activation_never_claims_that_the_original_is_already_restored() {
        let details = checkpoint::RecoveryDetails { operation_id: "a".repeat(32), checkpoint_available: true, previous_restored: false, activation_unresolved: true };
        let message = recovery_message(&details);
        assert!(message.contains("journal recovery"));
        assert!(message.contains("Do not install an older binary"));
        assert!(!message.contains("has been restored"));
    }

    #[test]
    fn restored_checkpoint_copy_distinguishes_library_and_executable_downgrade() {
        let details = checkpoint::RecoveryDetails { operation_id: "b".repeat(32), checkpoint_available: true, previous_restored: true, activation_unresolved: false };
        let message = recovery_message(&details);
        assert!(message.contains("has been restored"));
        assert!(message.contains("compatible previous application package"));
        assert!(message.contains("not automatically upgrade"));
    }
}
