//! Async Tauri adapter for the callback-based mobile QR scanner.
//!
//! Android's scan Activity completes through an ActivityResult callback. As with
//! the document picker, waiting synchronously for that callback from a Tauri
//! command can monopolize the command/event-loop thread needed to process native
//! lifecycle work. Keep the existing platform abstraction, but run the blocking
//! mobile-plugin bridge on Tauri's blocking pool.

use tauri::AppHandle;

use crate::error::CommandError;

#[tauri::command]
pub async fn scan_device_pairing_qr_async(
    app: AppHandle,
) -> Result<Option<String>, CommandError> {
    #[cfg(mobile)]
    {
        use tauri_plugin_ostudy_platform::{OstudyPlatformExt, ScanQrCodeRequest};

        tauri::async_runtime::spawn_blocking(move || {
            let result = app
                .ostudy_platform()
                .scan_qr_code(ScanQrCodeRequest {
                    prompt: "Center the entire ostudy pairing code inside the frame and hold steady"
                        .to_string(),
                })
                .map_err(|error| {
                    CommandError::new(
                        "device_sync_qr_scan_failed",
                        format!("The native QR scanner failed: {error}"),
                    )
                })?;
            Ok(if result.cancelled {
                None
            } else {
                result.contents
            })
        })
        .await
        .map_err(|error| {
            CommandError::new(
                "device_sync_qr_scan_interrupted",
                format!("The QR scanner worker stopped before returning a result: {error}"),
            )
        })?
    }
    #[cfg(not(mobile))]
    {
        let _ = app;
        Err(CommandError::new(
            "device_sync_qr_scan_unavailable",
            "Use the pairing invitation text or scan it with the receiving mobile device.",
        ))
    }
}
