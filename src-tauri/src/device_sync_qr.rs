//! Native QR acquisition boundary for device synchronization.

use tauri::AppHandle;

use crate::error::CommandError;

#[cfg(target_os = "android")]
use tauri_plugin_ostudy_platform::OstudyPlatformExt;

#[tauri::command]
pub async fn scan_device_sync_qr(app: AppHandle) -> Result<Option<String>, CommandError> {
    #[cfg(target_os = "android")]
    {
        let result = tauri::async_runtime::spawn_blocking(move || {
            app.ostudy_platform().scan_qr_code()
        })
        .await
        .map_err(|_| {
            CommandError::new(
                "sync_qr_interrupted",
                "The Android QR scan ended before returning a result.",
            )
        })?
        .map_err(|error| {
            CommandError::new(
                "sync_qr_failed",
                format!("Android could not scan the synchronization QR code: {error}"),
            )
        })?;
        if result.cancelled {
            Ok(None)
        } else {
            result.value.map(Some).ok_or_else(|| {
                CommandError::new(
                    "sync_qr_invalid",
                    "Android returned a QR scan without an invitation value.",
                )
            })
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err(CommandError::new(
            "sync_qr_camera_unavailable",
            "Use the Android camera scanner or paste the ostudy synchronization invitation.",
        ))
    }
}
