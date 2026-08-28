//! Conservative mobile fallback used when no target-specific native adapter exists.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, NaiveDate, Utc};
use tauri::AppHandle;

use crate::{
    error::CommandError,
    platform::{
        CapabilityProvider, ExternalFilePort, ExternalFileRequest, NativeServiceCapabilities,
        PlatformCapabilitySnapshot, PlatformFuture, PlatformRuntime, ResourceProbe,
        ResourceSnapshot, ServiceCapability, ThermalState, TimeZoneProvider, TimeZoneSnapshot,
        UnavailableSecretStore, UnavailableSystemSharePort,
    },
};

pub(super) fn runtime() -> PlatformRuntime {
    let clock = Arc::new(MobileSystemClock);
    PlatformRuntime::from_parts(
        Arc::new(MobileCapabilityProvider),
        Arc::new(UnavailableExternalFilePort),
        Arc::new(UnavailableSecretStore),
        clock,
        Arc::new(UtcFallbackTimeZoneProvider),
        Arc::new(ConservativeMobileResourceProbe),
        Arc::new(UnavailableSystemSharePort),
    )
}

struct MobileCapabilityProvider;

impl CapabilityProvider for MobileCapabilityProvider {
    fn snapshot(&self) -> PlatformCapabilitySnapshot {
        let unavailable =
            || ServiceCapability::unavailable("native_mobile_adapter_not_initialized");
        PlatformCapabilitySnapshot {
            schema_version: 2,
            runtime_family: "mobile".to_string(),
            operating_system: std::env::consts::OS.to_string(),
            hardware_back_available: cfg!(target_os = "android"),
            services: NativeServiceCapabilities {
                window_maximize_restore: ServiceCapability::unavailable(
                    "native_mobile_window_controls_not_supported",
                ),
                window_full_screen: ServiceCapability::unavailable(
                    "native_mobile_window_controls_not_supported",
                ),
                external_file_import: unavailable(),
                external_file_export: unavailable(),
                secure_secrets: unavailable(),
                lifecycle_events: unavailable(),
                local_time_zone: ServiceCapability::unavailable(
                    "native_time_zone_adapter_not_initialized",
                ),
                resource_observation: ServiceCapability::unavailable(
                    "native_resource_adapter_not_initialized",
                ),
                native_pdf_render: unavailable(),
                native_pdf_text_rescue: unavailable(),
                ocr: unavailable(),
                camera: unavailable(),
                qr_scan: unavailable(),
                haptics: unavailable(),
                system_share: unavailable(),
                local_network: unavailable(),
                background_execution: unavailable(),
            },
        }
    }
}

struct UnavailableExternalFilePort;

impl ExternalFilePort for UnavailableExternalFilePort {
    fn acquire_import(
        &self,
        _app: AppHandle,
        _request: ExternalFileRequest,
        _target: crate::platform::ExternalImportTarget,
    ) -> PlatformFuture<Result<Option<crate::platform::ExternalAcquireResult>, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "external_file_adapter_unavailable",
                "This mobile build does not include its native file-selection adapter.",
            ))
        })
    }

    fn publish_export(
        &self,
        _app: AppHandle,
        _request: ExternalFileRequest,
        _source: crate::platform::ExternalExportSource,
    ) -> PlatformFuture<Result<Option<crate::platform::ExternalPublishResult>, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "external_file_adapter_unavailable",
                "This mobile build does not include its native export adapter.",
            ))
        })
    }
}

struct MobileSystemClock;

impl crate::platform::Clock for MobileSystemClock {
    fn now_utc_millis(&self) -> Result<i64, String> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("The system clock is before the Unix epoch: {error}"))?;
        i64::try_from(duration.as_millis()).map_err(|_| {
            "The current timestamp is outside the supported integer range.".to_string()
        })
    }
}

struct UtcFallbackTimeZoneProvider;

impl TimeZoneProvider for UtcFallbackTimeZoneProvider {
    fn snapshot_at(&self, _utc_millis: i64) -> Result<TimeZoneSnapshot, String> {
        Ok(TimeZoneSnapshot {
            identifier: "UTC".to_string(),
            utc_offset_minutes: 0,
        })
    }

    fn local_date_at(&self, utc_millis: i64) -> Result<NaiveDate, String> {
        DateTime::<Utc>::from_timestamp_millis(utc_millis)
            .map(|value| value.date_naive())
            .ok_or_else(|| {
                "The requested study calendar date is outside the supported range.".to_string()
            })
    }

    fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, String> {
        date.and_hms_opt(0, 0, 0)
            .map(|value| value.and_utc().timestamp_millis())
            .ok_or_else(|| {
                "The requested study calendar date is outside the supported range.".to_string()
            })
    }
}

struct ConservativeMobileResourceProbe;

impl ResourceProbe for ConservativeMobileResourceProbe {
    fn snapshot(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            logical_cpus: std::thread::available_parallelism()
                .map(std::num::NonZeroUsize::get)
                .unwrap_or(1),
            available_memory_bytes: 0,
            low_memory: true,
            thermal_state: ThermalState::Unknown,
            battery_saver: false,
            charging: None,
            foreground: true,
        }
    }
}
