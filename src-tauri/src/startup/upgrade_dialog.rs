//! Native recovery UI before the normal library/UI command gate is available.
//! Filesystem work stays on blocking workers. A confirmation is bound to the
//! displayed operation identity; an old dialog cannot replace a newer intent.

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{library_backup::checkpoint, paths::AppPaths};

use super::STARTUP_CONFIGURATION_EXIT_CODE;

#[path = "upgrade_dialog_copy.rs"]
mod copy;

pub(super) fn present(app: &tauri::AppHandle, stage: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let details = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())
        .and_then(|root| AppPaths::new(root).map_err(|error| error.to_string()))
        .and_then(|paths| checkpoint::recovery_details(&paths));
    match details {
        Ok(Some(details)) if !details.activation_unresolved => {
            let text = recovery_copy(&details);
            let handle = app.clone();
            if details.checkpoint_available && !details.previous_restored {
                app.dialog()
                    .message(text.message)
                    .title(text.title)
                    .kind(MessageDialogKind::Warning)
                    .buttons(MessageDialogButtons::OkCancelCustom(
                        "Recovery options".into(),
                        copy::CLOSE.into(),
                    ))
                    .show(move |open| {
                        if open {
                            choose_recovery(&handle, details.operation_id);
                        } else {
                            handle.exit(STARTUP_CONFIGURATION_EXIT_CODE);
                        }
                    });
            } else {
                prompt_retry(
                    &handle,
                    details.operation_id,
                    text,
                    details.previous_restored,
                );
            }
        }
        Ok(Some(details)) => close_message(app, recovery_copy(&details)),
        Ok(None) => {
            let text = if stage == "upgrade_preflight" {
                copy::UNSUPPORTED
            } else {
                copy::PREPARATION_FAILED
            };
            close_message(app, text);
        }
        Err(error) => {
            crate::logging::error_target(
                "library.upgrade",
                "upgrade_recovery_metadata_invalid",
                "The library recovery UI could not validate its operation metadata.",
                serde_json::json!({"error": error}),
            );
            close_message(app, copy::INFORMATION_UNREADABLE);
        }
    }
}

fn recovery_copy(details: &checkpoint::RecoveryDetails) -> copy::DialogCopy {
    copy::entry(
        details.checkpoint_available,
        details.previous_restored,
        details.activation_unresolved,
    )
}

fn choose_recovery(app: &tauri::AppHandle, operation: String) {
    let handle = app.clone();
    app.dialog()
        .message(copy::OPTIONS.message)
        .title(copy::OPTIONS.title)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Restore backup…".into(),
            "Try update again…".into(),
        ))
        .show(move |restore| {
            if restore {
                confirm_restore(&handle, operation);
            } else {
                prompt_retry(&handle, operation, copy::CONFIRM_RETRY, false);
            }
        });
}

fn confirm_restore(app: &tauri::AppHandle, operation: String) {
    let handle = app.clone();
    app.dialog()
        .message(copy::CONFIRM_RESTORE.message)
        .title(copy::CONFIRM_RESTORE.title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Restore library".into(),
            "Close without restoring".into(),
        ))
        .show(move |confirmed| {
            if confirmed {
                run_recovery(&handle, operation, true);
            } else {
                handle.exit(STARTUP_CONFIGURATION_EXIT_CODE);
            }
        });
}

fn prompt_retry(app: &tauri::AppHandle, operation: String, text: copy::DialogCopy, restored: bool) {
    let handle = app.clone();
    app.dialog()
        .message(text.message)
        .title(text.title)
        .kind(if restored {
            MessageDialogKind::Info
        } else {
            MessageDialogKind::Warning
        })
        .buttons(MessageDialogButtons::OkCancelCustom(
            copy::UPDATE_NEXT_LAUNCH.into(),
            copy::retry_cancel(restored).into(),
        ))
        .show(move |confirmed| {
            if confirmed {
                run_recovery(&handle, operation, false);
            } else {
                handle.exit(STARTUP_CONFIGURATION_EXIT_CODE);
            }
        });
}

fn run_recovery(app: &tauri::AppHandle, operation: String, restore: bool) {
    let handle = app.clone();
    let worker = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            let root = worker
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?;
            let paths = AppPaths::new(root).map_err(|error| error.to_string())?;
            if restore {
                if let Some(window) = worker.get_webview_window("main") {
                    let _ = window.show();
                }
                checkpoint::restore_checkpoint(&paths, &operation, &mut |message| {
                    super::report_upgrade_progress(&worker, message)
                })
            } else {
                checkpoint::allow_retry(&paths, &operation)
            }
        })
        .await;
        match result {
            Ok(Ok(())) if restore => close_message(&handle, copy::RESTORE_DONE),
            Ok(Ok(())) => close_message(&handle, copy::RETRY_SCHEDULED),
            result => {
                let error = match result {
                    Ok(Err(error)) => error,
                    Err(error) => error.to_string(),
                    Ok(Ok(())) => unreachable!(),
                };
                crate::logging::error_target(
                    "library.upgrade",
                    "explicit_upgrade_recovery_failed",
                    "The explicit recovery action did not complete.",
                    serde_json::json!({"error": error}),
                );
                close_message(&handle, copy::RECOVERY_FAILED);
            }
        }
    });
}

fn close_message(app: &tauri::AppHandle, text: copy::DialogCopy) {
    let handle = app.clone();
    app.dialog()
        .message(text.message)
        .title(text.title)
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCustom(copy::CLOSE.into()))
        .show(move |_| handle.exit(STARTUP_CONFIGURATION_EXIT_CODE));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_activation_never_claims_that_the_original_is_already_restored() {
        let details = checkpoint::RecoveryDetails {
            operation_id: "a".repeat(32),
            checkpoint_available: true,
            previous_restored: false,
            activation_unresolved: true,
        };
        let text = recovery_copy(&details);
        assert_eq!(text, copy::UNRESOLVED);
        assert!(text.message.contains("Close and reopen oStudy"));
        assert!(text.message.contains("Do not install an older version"));
        assert!(!text.message.contains("has been restored"));
    }

    #[test]
    fn restored_checkpoint_copy_distinguishes_library_and_executable_downgrade() {
        let details = checkpoint::RecoveryDetails {
            operation_id: "b".repeat(32),
            checkpoint_available: true,
            previous_restored: true,
            activation_unresolved: false,
        };
        let text = recovery_copy(&details);
        assert_eq!(text, copy::RESTORED);
        assert!(text.message.contains("has been restored"));
        assert!(text.message.contains("previous compatible version"));
        assert!(text.message.contains("close without changing it"));
        assert!(text.message.contains("This is optional"));
    }
}
