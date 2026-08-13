//! Android implementations of the typed platform services.
//!
//! Kotlin owns only Android API interactions. Shared Rust remains responsible
//! for transfer ledgers, content validation, scheduling, analytics, authoring,
//! and every SQLite transaction.

use std::{
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chrono::NaiveDate;
use tauri::AppHandle;
use tauri_plugin_ostudy_platform::{
    AcquireImportRequest, OstudyPlatformExt, PublishExportRequest, ShareFileRequest,
};

use crate::{
    error::CommandError,
    platform::{
        CapabilityProvider, Clock, ExternalAcquireResult, ExternalExportSource, ExternalFilePort,
        ExternalFileRequest, ExternalImportTarget, ExternalPublishResult,
        NativeServiceCapabilities, PlatformCapabilitySnapshot, PlatformFuture, PlatformRuntime,
        ResourceProbe, ResourceSnapshot, SecretBytes, SecretLocator, SecretStoreAvailability,
        SecretStoreError, SecureSecretStore, ServiceCapability, ShareLaunchResult, ShareSource,
        SystemSharePort, ThermalState, TimeZoneProvider, TimeZoneSnapshot,
    },
};

pub(super) fn runtime(app: AppHandle) -> PlatformRuntime {
    let clock = Arc::new(AndroidSystemClock);
    PlatformRuntime::from_parts(
        Arc::new(AndroidCapabilityProvider { app: app.clone() }),
        Arc::new(AndroidExternalFilePort { app: app.clone() }),
        Arc::new(AndroidSecretStore { app: app.clone() }),
        clock,
        Arc::new(AndroidTimeZoneProvider { app: app.clone() }),
        Arc::new(AndroidResourceProbe { app: app.clone() }),
        Arc::new(AndroidSystemSharePort { app }),
    )
}

struct AndroidCapabilityProvider {
    app: AppHandle,
}

impl CapabilityProvider for AndroidCapabilityProvider {
    fn snapshot(&self) -> PlatformCapabilitySnapshot {
        let environment_available = self.app.ostudy_platform().environment().is_ok();
        let secrets_available = self
            .app
            .ostudy_platform()
            .secret_availability()
            .map(|value| value.state == "available")
            .unwrap_or(false);
        let environment_capability = || {
            if environment_available {
                ServiceCapability::available()
            } else {
                ServiceCapability::unavailable("android_platform_environment_unavailable")
            }
        };
        PlatformCapabilitySnapshot {
            schema_version: 1,
            runtime_family: "mobile".to_string(),
            operating_system: "android".to_string(),
            hardware_back_available: true,
            services: NativeServiceCapabilities {
                external_file_import: ServiceCapability::available(),
                external_file_export: ServiceCapability::available(),
                secure_secrets: if secrets_available {
                    ServiceCapability::available()
                } else {
                    ServiceCapability::unavailable("android_keystore_unavailable")
                },
                lifecycle_events: ServiceCapability::available(),
                local_time_zone: environment_capability(),
                resource_observation: environment_capability(),
                native_pdf_render: ServiceCapability::unavailable(
                    "android_document_processing_development_stub",
                ),
                native_pdf_text_rescue: ServiceCapability::unavailable(
                    "android_document_processing_development_stub",
                ),
                ocr: ServiceCapability::unavailable("android_document_processing_development_stub"),
                camera: ServiceCapability::unavailable("android_camera_adapter_not_implemented"),
                qr_scan: ServiceCapability::unavailable("android_qr_adapter_not_implemented"),
                haptics: ServiceCapability::unavailable("android_haptics_adapter_not_implemented"),
                system_share: ServiceCapability::available(),
                local_network: environment_capability(),
                background_execution: ServiceCapability::unavailable(
                    "android_background_execution_not_implemented",
                ),
            },
        }
    }
}

struct AndroidExternalFilePort {
    app: AppHandle,
}

impl ExternalFilePort for AndroidExternalFilePort {
    fn acquire_import(
        &self,
        _app: AppHandle,
        request: ExternalFileRequest,
        target: ExternalImportTarget,
    ) -> PlatformFuture<Result<Option<ExternalAcquireResult>, CommandError>> {
        let app = self.app.clone();
        Box::pin(async move {
            let target_path = path_to_utf8(&target.path, "external import staging")?;
            let mime_types = request_mime_types(&request);
            let result = tauri::async_runtime::spawn_blocking(move || {
                app.ostudy_platform().acquire_import(AcquireImportRequest {
                    title: request.title,
                    mime_types,
                    target_path,
                    maximum_bytes: target.maximum_bytes,
                })
            })
            .await
            .map_err(|_| android_plugin_interrupted("external import"))?
            .map_err(|error| android_plugin_error("external import", error))?;
            if result.cancelled {
                return Ok(None);
            }
            let display_name = result.display_name.ok_or_else(|| {
                CommandError::new(
                    "android_external_file_invalid",
                    "Android returned an imported file without a display name.",
                )
            })?;
            Ok(Some(ExternalAcquireResult {
                display_name,
                reported_media_type: result.reported_media_type,
                copied_bytes: result.copied_bytes,
            }))
        })
    }

    fn publish_export(
        &self,
        _app: AppHandle,
        request: ExternalFileRequest,
        source: ExternalExportSource,
    ) -> PlatformFuture<Result<Option<ExternalPublishResult>, CommandError>> {
        let app = self.app.clone();
        Box::pin(async move {
            let source_path = path_to_utf8(&source.path, "prepared export")?;
            let suggested_filename = request
                .suggested_filename
                .unwrap_or_else(|| source.display_name.clone());
            let result = tauri::async_runtime::spawn_blocking(move || {
                app.ostudy_platform().publish_export(PublishExportRequest {
                    title: request.title,
                    suggested_filename,
                    mime_type: source.media_type,
                    source_path,
                    maximum_bytes: source.maximum_bytes,
                })
            })
            .await
            .map_err(|_| android_plugin_interrupted("export publication"))?
            .map_err(|error| android_plugin_error("export publication", error))?;
            if result.cancelled {
                return Ok(None);
            }
            Ok(Some(ExternalPublishResult {
                display_name: result
                    .display_name
                    .unwrap_or_else(|| source.display_name.clone()),
                copied_bytes: result.copied_bytes,
                destination_kind: "android_document".to_string(),
            }))
        })
    }
}

struct AndroidSystemSharePort {
    app: AppHandle,
}

impl SystemSharePort for AndroidSystemSharePort {
    fn share_file(
        &self,
        _app: AppHandle,
        source: ShareSource,
    ) -> PlatformFuture<Result<ShareLaunchResult, CommandError>> {
        let app = self.app.clone();
        Box::pin(async move {
            let source_path = path_to_utf8(&source.path, "prepared share")?;
            let result = tauri::async_runtime::spawn_blocking(move || {
                app.ostudy_platform().share_file(ShareFileRequest {
                    title: source.title,
                    display_name: source.display_name,
                    mime_type: source.media_type,
                    source_path,
                })
            })
            .await
            .map_err(|_| android_plugin_interrupted("system sharing"))?
            .map_err(|error| android_plugin_error("system sharing", error))?;
            Ok(ShareLaunchResult {
                launched: result.launched,
                destination_kind: "android_sharesheet".to_string(),
            })
        })
    }
}

struct AndroidSecretStore {
    app: AppHandle,
}

impl SecureSecretStore for AndroidSecretStore {
    fn availability(&self) -> SecretStoreAvailability {
        match self.app.ostudy_platform().secret_availability() {
            Ok(value) if value.state == "available" => SecretStoreAvailability::Available,
            Ok(value) if value.state == "locked" => SecretStoreAvailability::Locked,
            _ => SecretStoreAvailability::Unavailable,
        }
    }

    fn read(&self, locator: &SecretLocator) -> Result<Option<SecretBytes>, SecretStoreError> {
        let result = self
            .app
            .ostudy_platform()
            .read_secret(render_locator(locator))
            .map_err(|_| SecretStoreError::Failed)?;
        match result.state.as_str() {
            "missing" => Ok(None),
            "available" => {
                let encoded = result.value_base64.ok_or(SecretStoreError::Failed)?;
                let bytes = BASE64_STANDARD
                    .decode(encoded)
                    .map_err(|_| SecretStoreError::Failed)?;
                Ok(Some(SecretBytes::new(bytes)))
            }
            "locked" => Err(SecretStoreError::Locked),
            "invalidated" => Err(SecretStoreError::Invalidated),
            "permission_denied" => Err(SecretStoreError::PermissionDenied),
            "unavailable" => Err(SecretStoreError::Unavailable),
            _ => Err(SecretStoreError::Failed),
        }
    }

    fn write(&self, locator: &SecretLocator, value: &[u8]) -> Result<(), SecretStoreError> {
        self.app
            .ostudy_platform()
            .write_secret(render_locator(locator), BASE64_STANDARD.encode(value))
            .map_err(|_| SecretStoreError::Failed)
    }

    fn delete(&self, locator: &SecretLocator) -> Result<(), SecretStoreError> {
        self.app
            .ostudy_platform()
            .delete_secret(render_locator(locator))
            .map_err(|_| SecretStoreError::Failed)
    }
}

struct AndroidSystemClock;

impl Clock for AndroidSystemClock {
    fn now_utc_millis(&self) -> Result<i64, String> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("The system clock is before the Unix epoch: {error}"))?;
        i64::try_from(duration.as_millis()).map_err(|_| {
            "The current timestamp is outside the supported integer range.".to_string()
        })
    }
}

struct AndroidTimeZoneProvider {
    app: AppHandle,
}

impl TimeZoneProvider for AndroidTimeZoneProvider {
    fn snapshot_at(&self, utc_millis: i64) -> Result<TimeZoneSnapshot, String> {
        self.app
            .ostudy_platform()
            .time_zone_snapshot(utc_millis)
            .map(|value| TimeZoneSnapshot {
                identifier: value.identifier,
                utc_offset_minutes: value.utc_offset_minutes,
            })
            .map_err(|error| error.to_string())
    }

    fn local_date_at(&self, utc_millis: i64) -> Result<NaiveDate, String> {
        let result = self
            .app
            .ostudy_platform()
            .local_date_at(utc_millis)
            .map_err(|error| error.to_string())?;
        NaiveDate::parse_from_str(&result.date, "%Y-%m-%d")
            .map_err(|_| "Android returned an invalid local calendar date.".to_string())
    }

    fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, String> {
        self.app
            .ostudy_platform()
            .first_valid_instant(date.format("%Y-%m-%d").to_string())
            .map(|value| value.utc_millis)
            .map_err(|error| error.to_string())
    }
}

struct AndroidResourceProbe {
    app: AppHandle,
}

impl ResourceProbe for AndroidResourceProbe {
    fn snapshot(&self) -> ResourceSnapshot {
        let fallback = || ResourceSnapshot {
            logical_cpus: std::thread::available_parallelism()
                .map(std::num::NonZeroUsize::get)
                .unwrap_or(1),
            available_memory_bytes: 0,
            low_memory: true,
            thermal_state: ThermalState::Unknown,
            battery_saver: false,
            charging: None,
            foreground: true,
        };
        let Ok(environment) = self.app.ostudy_platform().environment() else {
            return fallback();
        };
        ResourceSnapshot {
            logical_cpus: usize::try_from(environment.logical_cpu_count)
                .unwrap_or(1)
                .max(1),
            available_memory_bytes: environment.available_memory_bytes.unwrap_or_default(),
            low_memory: environment.low_memory,
            thermal_state: match environment.thermal_state.as_str() {
                "nominal" => ThermalState::Nominal,
                "fair" => ThermalState::Fair,
                "serious" => ThermalState::Serious,
                "critical" => ThermalState::Critical,
                _ => ThermalState::Unknown,
            },
            battery_saver: environment.battery_saver,
            charging: environment.charging,
            foreground: true,
        }
    }
}

fn request_mime_types(request: &ExternalFileRequest) -> Vec<String> {
    let mut values = request
        .filters
        .iter()
        .flat_map(|filter| filter.extensions.iter())
        .filter_map(|extension| match extension.to_ascii_lowercase().as_str() {
            "png" => Some("image/png"),
            "jpg" | "jpeg" => Some("image/jpeg"),
            "gif" => Some("image/gif"),
            "webp" => Some("image/webp"),
            "pdf" => Some("application/pdf"),
            "json" => Some("application/json"),
            "studydeck" => Some("application/vnd.ostudy.studydeck"),
            "ostudybackup" => Some("application/vnd.ostudy.backup"),
            _ => None,
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    if values.is_empty() {
        values.push("application/octet-stream".to_string());
    }
    values
}

fn render_locator(locator: &SecretLocator) -> String {
    format!("{}:{}", locator.namespace, locator.key)
}

fn path_to_utf8(path: &PathBuf, label: &str) -> Result<String, CommandError> {
    path.to_str().map(str::to_string).ok_or_else(|| {
        CommandError::new(
            "android_path_unsupported",
            format!("The app-private {label} path is not valid UTF-8."),
        )
    })
}

fn android_plugin_interrupted(operation: &str) -> CommandError {
    CommandError::new(
        "android_platform_operation_interrupted",
        format!("The Android {operation} operation ended before returning a result."),
    )
}

fn android_plugin_error(
    operation: &str,
    error: tauri_plugin_ostudy_platform::Error,
) -> CommandError {
    CommandError::new(
        "android_platform_operation_failed",
        format!("The Android {operation} operation failed: {error}"),
    )
}
