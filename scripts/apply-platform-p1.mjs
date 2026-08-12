#!/usr/bin/env node

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const write = async (path, content) => {
  const target = join(root, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, content, "utf8");
};
const replace = async (path, from, to) => {
  const source = await read(path);
  if (!source.includes(from)) throw new Error(`${path}: expected source fragment was not found`);
  await write(path, source.replace(from, to));
};

await write("src-tauri/src/platform/mod.rs", String.raw`//! Typed operating-system capability ports and runtime composition.

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop;

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, LazyLock, RwLock,
    },
};

use chrono::NaiveDate;
use serde::Serialize;
use tauri::{AppHandle, State};

use crate::error::CommandError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    Available,
    PermissionRequired,
    Denied,
    Unavailable,
    Degraded,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCapability {
    pub state: CapabilityState,
    pub reason_code: Option<String>,
    pub can_request_permission: bool,
}

impl ServiceCapability {
    pub fn available() -> Self {
        Self {
            state: CapabilityState::Available,
            reason_code: None,
            can_request_permission: false,
        }
    }

    pub fn unavailable(reason_code: &str) -> Self {
        Self {
            state: CapabilityState::Unavailable,
            reason_code: Some(reason_code.to_string()),
            can_request_permission: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeServiceCapabilities {
    pub external_file_import: ServiceCapability,
    pub external_file_export: ServiceCapability,
    pub secure_secrets: ServiceCapability,
    pub lifecycle_events: ServiceCapability,
    pub local_time_zone: ServiceCapability,
    pub resource_observation: ServiceCapability,
    pub native_pdf_render: ServiceCapability,
    pub native_pdf_text_rescue: ServiceCapability,
    pub ocr: ServiceCapability,
    pub camera: ServiceCapability,
    pub qr_scan: ServiceCapability,
    pub haptics: ServiceCapability,
    pub system_share: ServiceCapability,
    pub local_network: ServiceCapability,
    pub background_execution: ServiceCapability,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilitySnapshot {
    pub schema_version: i64,
    pub runtime_family: String,
    pub operating_system: String,
    pub hardware_back_available: bool,
    pub services: NativeServiceCapabilities,
}

pub trait CapabilityProvider: Send + Sync {
    fn snapshot(&self) -> PlatformCapabilitySnapshot;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretLocator {
    pub namespace: &'static str,
    pub key: String,
}

impl SecretLocator {
    pub fn ai_profile(profile_id: &str) -> Self {
        Self {
            namespace: "ai_provider",
            key: format!("profile/{profile_id}/api-key"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretStoreAvailability {
    Available,
    Unavailable,
    Locked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretStoreError {
    Unavailable,
    Locked,
    PermissionDenied,
    Invalidated,
    Failed,
}

pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_utf8(&self) -> Result<&str, CommandError> {
        std::str::from_utf8(&self.0).map_err(|_| {
            CommandError::new(
                "ai_credential_invalid",
                "The saved AI credential is not valid UTF-8 text.",
            )
        })
    }
}

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

pub trait SecureSecretStore: Send + Sync {
    fn availability(&self) -> SecretStoreAvailability;
    fn read(&self, locator: &SecretLocator) -> Result<Option<SecretBytes>, SecretStoreError>;
    fn write(&self, locator: &SecretLocator, value: &[u8]) -> Result<(), SecretStoreError>;
    fn delete(&self, locator: &SecretLocator) -> Result<(), SecretStoreError>;
}

#[derive(Default)]
pub struct UnavailableSecretStore;

impl SecureSecretStore for UnavailableSecretStore {
    fn availability(&self) -> SecretStoreAvailability {
        SecretStoreAvailability::Unavailable
    }

    fn read(&self, _locator: &SecretLocator) -> Result<Option<SecretBytes>, SecretStoreError> {
        Err(SecretStoreError::Unavailable)
    }

    fn write(&self, _locator: &SecretLocator, _value: &[u8]) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::Unavailable)
    }

    fn delete(&self, _locator: &SecretLocator) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::Unavailable)
    }
}

pub trait Clock: Send + Sync {
    fn now_utc_millis(&self) -> Result<i64, String>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeZoneSnapshot {
    pub identifier: String,
    pub utc_offset_minutes: i32,
}

pub trait TimeZoneProvider: Send + Sync {
    fn snapshot_at(&self, utc_millis: i64) -> Result<TimeZoneSnapshot, String>;
    fn local_date_at(&self, utc_millis: i64) -> Result<NaiveDate, String>;
    fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, String>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalState {
    Nominal,
    Fair,
    Serious,
    Critical,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSnapshot {
    pub logical_cpus: usize,
    pub available_memory_bytes: u64,
    pub low_memory: bool,
    pub thermal_state: ThermalState,
    pub battery_saver: bool,
    pub charging: Option<bool>,
    pub foreground: bool,
}

pub trait ResourceProbe: Send + Sync {
    fn snapshot(&self) -> ResourceSnapshot;
}

#[derive(Clone, Debug)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ExternalFileRequest {
    pub title: String,
    pub suggested_filename: Option<String>,
    pub filters: Vec<FileFilter>,
}

#[derive(Clone, Debug)]
pub struct ExternalSource {
    pub path: PathBuf,
    pub display_name: String,
}

#[derive(Clone, Debug)]
pub struct ExternalDestination {
    pub path: PathBuf,
    pub display_name: String,
}

pub trait ExternalFilePort: Send + Sync {
    fn select_import(
        &self,
        app: &AppHandle,
        request: &ExternalFileRequest,
    ) -> Result<Option<ExternalSource>, CommandError>;

    fn select_export(
        &self,
        app: &AppHandle,
        request: &ExternalFileRequest,
    ) -> Result<Option<ExternalDestination>, CommandError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePhase {
    Starting,
    Foreground,
    Background,
    Suspending,
    Resuming,
    TerminationRequested,
    Terminating,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleSnapshot {
    pub schema_version: i64,
    pub sequence: u64,
    pub phase: LifecyclePhase,
    pub observed_at_utc_millis: i64,
}

pub struct LifecycleCoordinator {
    sequence: AtomicU64,
    snapshot: RwLock<LifecycleSnapshot>,
    clock: Arc<dyn Clock>,
}

impl LifecycleCoordinator {
    fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            sequence: AtomicU64::new(0),
            snapshot: RwLock::new(LifecycleSnapshot {
                schema_version: 1,
                sequence: 0,
                phase: LifecyclePhase::Starting,
                observed_at_utc_millis: 0,
            }),
            clock,
        }
    }

    pub fn current(&self) -> LifecycleSnapshot {
        self.snapshot
            .read()
            .map(|value| value.clone())
            .unwrap_or(LifecycleSnapshot {
                schema_version: 1,
                sequence: self.sequence.load(Ordering::Acquire),
                phase: LifecyclePhase::Starting,
                observed_at_utc_millis: 0,
            })
    }

    pub fn transition(&self, phase: LifecyclePhase) -> LifecycleSnapshot {
        let sequence = self.sequence.fetch_add(1, Ordering::AcqRel) + 1;
        let snapshot = LifecycleSnapshot {
            schema_version: 1,
            sequence,
            phase,
            observed_at_utc_millis: self.clock.now_utc_millis().unwrap_or_default(),
        };
        if let Ok(mut current) = self.snapshot.write() {
            *current = snapshot.clone();
        }
        snapshot
    }
}

#[derive(Clone)]
pub struct PlatformRuntime {
    capabilities: Arc<dyn CapabilityProvider>,
    external_files: Arc<dyn ExternalFilePort>,
    secure_secrets: Arc<dyn SecureSecretStore>,
    lifecycle: Arc<LifecycleCoordinator>,
    clock: Arc<dyn Clock>,
    time_zone: Arc<dyn TimeZoneProvider>,
    resources: Arc<dyn ResourceProbe>,
}

impl PlatformRuntime {
    pub fn current() -> Self {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            return desktop::runtime();
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            compile_error!("Mobile PlatformRuntime adapters must be supplied by the generated Tauri mobile project.");
        }
    }

    pub(crate) fn from_parts(
        capabilities: Arc<dyn CapabilityProvider>,
        external_files: Arc<dyn ExternalFilePort>,
        secure_secrets: Arc<dyn SecureSecretStore>,
        clock: Arc<dyn Clock>,
        time_zone: Arc<dyn TimeZoneProvider>,
        resources: Arc<dyn ResourceProbe>,
    ) -> Self {
        let lifecycle = Arc::new(LifecycleCoordinator::new(Arc::clone(&clock)));
        Self {
            capabilities,
            external_files,
            secure_secrets,
            lifecycle,
            clock,
            time_zone,
            resources,
        }
    }

    pub fn capabilities(&self) -> PlatformCapabilitySnapshot {
        self.capabilities.snapshot()
    }

    pub fn external_files(&self) -> Arc<dyn ExternalFilePort> {
        Arc::clone(&self.external_files)
    }

    pub fn secure_secret_store(&self) -> Arc<dyn SecureSecretStore> {
        Arc::clone(&self.secure_secrets)
    }

    pub fn lifecycle(&self) -> Arc<LifecycleCoordinator> {
        Arc::clone(&self.lifecycle)
    }

    pub fn resource_snapshot(&self) -> ResourceSnapshot {
        self.resources.snapshot()
    }

    pub fn now_utc_millis(&self) -> Result<i64, String> {
        self.clock.now_utc_millis()
    }

    pub fn time_zone_snapshot(&self, utc_millis: i64) -> Result<TimeZoneSnapshot, String> {
        self.time_zone.snapshot_at(utc_millis)
    }

    pub fn local_date_at(&self, utc_millis: i64) -> Result<NaiveDate, String> {
        self.time_zone.local_date_at(utc_millis)
    }

    pub fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, String> {
        self.time_zone.first_valid_instant(date)
    }
}

static GLOBAL_RUNTIME: LazyLock<RwLock<PlatformRuntime>> =
    LazyLock::new(|| RwLock::new(PlatformRuntime::current()));

pub fn install_global(runtime: PlatformRuntime) -> Result<(), String> {
    let mut current = GLOBAL_RUNTIME
        .write()
        .map_err(|_| "The platform runtime lock is unavailable.".to_string())?;
    *current = runtime;
    Ok(())
}

fn with_runtime<T>(operation: impl FnOnce(&PlatformRuntime) -> T) -> T {
    match GLOBAL_RUNTIME.read() {
        Ok(runtime) => operation(&runtime),
        Err(_) => operation(&PlatformRuntime::current()),
    }
}

pub fn now_utc_millis() -> Result<i64, String> {
    with_runtime(PlatformRuntime::now_utc_millis)
}

pub fn resource_snapshot() -> ResourceSnapshot {
    with_runtime(PlatformRuntime::resource_snapshot)
}

pub fn time_zone_snapshot(utc_millis: i64) -> Result<TimeZoneSnapshot, String> {
    with_runtime(|runtime| runtime.time_zone_snapshot(utc_millis))
}

pub fn local_date_at(utc_millis: i64) -> Result<NaiveDate, String> {
    with_runtime(|runtime| runtime.local_date_at(utc_millis))
}

pub fn first_valid_instant(date: NaiveDate) -> Result<i64, String> {
    with_runtime(|runtime| runtime.first_valid_instant(date))
}

#[tauri::command]
pub fn get_platform_capabilities(
    runtime: State<'_, PlatformRuntime>,
) -> PlatformCapabilitySnapshot {
    runtime.capabilities()
}

#[cfg(test)]
pub(crate) fn unavailable_secret_store() -> Arc<dyn SecureSecretStore> {
    Arc::new(UnavailableSecretStore)
}
`);

await write("src-tauri/src/platform/desktop/mod.rs", String.raw`mod capabilities;
mod external_files;
mod resources;
mod secrets;
mod time;

use std::sync::Arc;

use crate::platform::PlatformRuntime;

pub(super) fn runtime() -> PlatformRuntime {
    let clock = Arc::new(time::SystemClock);
    PlatformRuntime::from_parts(
        Arc::new(capabilities::DesktopCapabilityProvider),
        Arc::new(external_files::DesktopExternalFilePort),
        Arc::new(secrets::KeyringSecretStore),
        clock,
        Arc::new(time::SystemTimeZoneProvider),
        Arc::new(resources::SysinfoResourceProbe),
    )
}
`);

await write("src-tauri/src/platform/desktop/capabilities.rs", String.raw`use crate::platform::{
    CapabilityProvider, NativeServiceCapabilities, PlatformCapabilitySnapshot, ServiceCapability,
};

pub(super) struct DesktopCapabilityProvider;

impl CapabilityProvider for DesktopCapabilityProvider {
    fn snapshot(&self) -> PlatformCapabilitySnapshot {
        PlatformCapabilitySnapshot {
            schema_version: 1,
            runtime_family: "desktop".to_string(),
            operating_system: std::env::consts::OS.to_string(),
            hardware_back_available: false,
            services: NativeServiceCapabilities {
                external_file_import: ServiceCapability::available(),
                external_file_export: ServiceCapability::available(),
                secure_secrets: ServiceCapability::available(),
                lifecycle_events: ServiceCapability::available(),
                local_time_zone: ServiceCapability::available(),
                resource_observation: ServiceCapability::available(),
                native_pdf_render: ServiceCapability::available(),
                native_pdf_text_rescue: ServiceCapability::available(),
                ocr: ServiceCapability::available(),
                camera: ServiceCapability::unavailable("desktop_camera_adapter_not_configured"),
                qr_scan: ServiceCapability::unavailable("qr_scanner_not_implemented"),
                haptics: ServiceCapability::unavailable("desktop_haptics_not_configured"),
                system_share: ServiceCapability::unavailable("system_share_not_implemented"),
                local_network: ServiceCapability::available(),
                background_execution: ServiceCapability::unavailable(
                    "desktop_background_scheduler_not_configured",
                ),
            },
        }
    }
}
`);

await write("src-tauri/src/platform/desktop/external_files.rs", String.raw`use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{
    error::CommandError,
    platform::{
        ExternalDestination, ExternalFilePort, ExternalFileRequest, ExternalSource,
    },
};

pub(super) struct DesktopExternalFilePort;

impl ExternalFilePort for DesktopExternalFilePort {
    fn select_import(
        &self,
        app: &AppHandle,
        request: &ExternalFileRequest,
    ) -> Result<Option<ExternalSource>, CommandError> {
        let mut dialog = app.dialog().file().set_title(&request.title);
        for filter in &request.filters {
            let extensions = filter
                .extensions
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            dialog = dialog.add_filter(&filter.name, &extensions);
        }
        dialog
            .blocking_pick_file()
            .map(source_from_file_path)
            .transpose()
    }

    fn select_export(
        &self,
        app: &AppHandle,
        request: &ExternalFileRequest,
    ) -> Result<Option<ExternalDestination>, CommandError> {
        let mut dialog = app.dialog().file().set_title(&request.title);
        if let Some(filename) = &request.suggested_filename {
            dialog = dialog.set_file_name(filename);
        }
        for filter in &request.filters {
            let extensions = filter
                .extensions
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            dialog = dialog.add_filter(&filter.name, &extensions);
        }
        dialog
            .blocking_save_file()
            .map(destination_from_file_path)
            .transpose()
    }
}

fn source_from_file_path(value: FilePath) -> Result<ExternalSource, CommandError> {
    let path = value.into_path().map_err(|_| {
        CommandError::new(
            "external_file_reference_unsupported",
            "The selected file could not be accessed by this desktop build.",
        )
    })?;
    let display_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("selected file")
        .to_string();
    Ok(ExternalSource { path, display_name })
}

fn destination_from_file_path(value: FilePath) -> Result<ExternalDestination, CommandError> {
    let path = value.into_path().map_err(|_| {
        CommandError::new(
            "external_destination_unsupported",
            "The selected destination could not be accessed by this desktop build.",
        )
    })?;
    let display_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("export")
        .to_string();
    Ok(ExternalDestination { path, display_name })
}
`);

await write("src-tauri/src/platform/desktop/resources.rs", String.raw`use sysinfo::{MemoryRefreshKind, System};

use crate::platform::{ResourceProbe, ResourceSnapshot, ThermalState};

pub(super) struct SysinfoResourceProbe;

impl ResourceProbe for SysinfoResourceProbe {
    fn snapshot(&self) -> ResourceSnapshot {
        let logical_cpus = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1)
            .max(1);
        let mut system = System::new();
        system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        ResourceSnapshot {
            logical_cpus,
            available_memory_bytes: system.available_memory(),
            low_memory: false,
            thermal_state: ThermalState::Unknown,
            battery_saver: false,
            charging: None,
            foreground: true,
        }
    }
}
`);

await write("src-tauri/src/platform/desktop/secrets.rs", String.raw`use keyring::v1::{Entry, Error as KeyringError};

use crate::platform::{
    SecretBytes, SecretLocator, SecretStoreAvailability, SecretStoreError, SecureSecretStore,
};

const KEYRING_SERVICE: &str = "io.github.jferments.ostudy";
const KEYRING_AVAILABILITY_ACCOUNT: &str = "availability-probe";

pub(super) struct KeyringSecretStore;

impl SecureSecretStore for KeyringSecretStore {
    fn availability(&self) -> SecretStoreAvailability {
        if Entry::new(KEYRING_SERVICE, KEYRING_AVAILABILITY_ACCOUNT).is_ok() {
            SecretStoreAvailability::Available
        } else {
            SecretStoreAvailability::Unavailable
        }
    }

    fn read(&self, locator: &SecretLocator) -> Result<Option<SecretBytes>, SecretStoreError> {
        let entry = entry(locator)?;
        match entry.get_secret() {
            Ok(secret) => Ok(Some(SecretBytes::new(secret))),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(classify_error(error)),
        }
    }

    fn write(&self, locator: &SecretLocator, value: &[u8]) -> Result<(), SecretStoreError> {
        entry(locator)?.set_secret(value).map_err(classify_error)
    }

    fn delete(&self, locator: &SecretLocator) -> Result<(), SecretStoreError> {
        match entry(locator)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(classify_error(error)),
        }
    }
}

fn entry(locator: &SecretLocator) -> Result<Entry, SecretStoreError> {
    Entry::new(
        &format!("{KEYRING_SERVICE}.{}", locator.namespace),
        &locator.key,
    )
    .map_err(classify_error)
}

fn classify_error(error: KeyringError) -> SecretStoreError {
    match error {
        KeyringError::NoDefaultStore
        | KeyringError::NoStorageAccess(_)
        | KeyringError::NotSupportedByStore(_) => SecretStoreError::Unavailable,
        _ => SecretStoreError::Failed,
    }
}
`);

await write("src-tauri/src/platform/desktop/time.rs", String.raw`use std::{env, path::Path, time::{SystemTime, UNIX_EPOCH}};

use chrono::{DateTime, Duration, Local, LocalResult, NaiveDate, Offset, TimeZone};

use crate::platform::{Clock, TimeZoneProvider, TimeZoneSnapshot};

const SECONDS_PER_LOCAL_DATE: i64 = 86_400;

pub(super) struct SystemClock;

impl Clock for SystemClock {
    fn now_utc_millis(&self) -> Result<i64, String> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("The system clock is before the Unix epoch: {error}"))?;
        i64::try_from(duration.as_millis())
            .map_err(|_| "The current timestamp is outside the supported integer range.".to_string())
    }
}

pub(super) struct SystemTimeZoneProvider;

impl TimeZoneProvider for SystemTimeZoneProvider {
    fn snapshot_at(&self, utc_millis: i64) -> Result<TimeZoneSnapshot, String> {
        let value = Local
            .timestamp_millis_opt(utc_millis)
            .earliest()
            .ok_or_else(invalid_calendar_date)?;
        Ok(TimeZoneSnapshot {
            identifier: discover_system_time_zone_id(),
            utc_offset_minutes: value.offset().fix().local_minus_utc().div_euclid(60),
        })
    }

    fn local_date_at(&self, utc_millis: i64) -> Result<NaiveDate, String> {
        Local
            .timestamp_millis_opt(utc_millis)
            .earliest()
            .map(|value| value.date_naive())
            .ok_or_else(invalid_calendar_date)
    }

    fn first_valid_instant(&self, date: NaiveDate) -> Result<i64, String> {
        let midnight = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(invalid_calendar_date)?;
        for seconds in 0..SECONDS_PER_LOCAL_DATE {
            let local = midnight
                .checked_add_signed(Duration::seconds(seconds))
                .ok_or_else(invalid_calendar_date)?;
            if local.date() != date {
                break;
            }
            if let Some(value) = resolve_local_result(Local.from_local_datetime(&local)) {
                return Ok(value);
            }
        }
        Err("That local calendar date has no valid instant in the current time zone.".to_string())
    }
}

fn resolve_local_result(result: LocalResult<DateTime<Local>>) -> Option<i64> {
    match result {
        LocalResult::Single(value) => Some(value.timestamp_millis()),
        LocalResult::Ambiguous(first, second) => {
            Some(first.timestamp_millis().min(second.timestamp_millis()))
        }
        LocalResult::None => None,
    }
}

fn discover_system_time_zone_id() -> String {
    if let Ok(value) = env::var("TZ") {
        if let Some(normalized) = normalize_zone_identifier(&value) {
            return normalized;
        }
    }

    #[cfg(unix)]
    {
        if let Ok(target) = std::fs::read_link("/etc/localtime") {
            let rendered = target.to_string_lossy();
            if let Some((_, suffix)) = rendered.rsplit_once("/zoneinfo/") {
                if let Some(normalized) = normalize_zone_identifier(suffix) {
                    return normalized;
                }
            }
        }
        if let Ok(value) = std::fs::read_to_string("/etc/timezone") {
            if let Some(normalized) = normalize_zone_identifier(value.trim()) {
                return normalized;
            }
        }
    }

    "unknown".to_string()
}

fn normalize_zone_identifier(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_start_matches(':');
    if trimmed.is_empty()
        || trimmed.len() > 128
        || Path::new(trimmed).is_absolute()
        || trimmed.chars().any(|character| {
            character.is_control()
                || !(character.is_ascii_alphanumeric()
                    || matches!(character, '/' | '_' | '-' | '+' | '.'))
        })
    {
        return None;
    }
    Some(trimmed.to_string())
}

fn invalid_calendar_date() -> String {
    "The requested study calendar date is outside the supported range.".to_string()
}
`);

await write("src-tauri/src/ai/secrets.rs", String.raw`use std::{collections::HashMap, sync::{Arc, Mutex}};

use serde::{Deserialize, Serialize};

use crate::{
    error::CommandError,
    logging,
    platform::{
        SecretBytes, SecretLocator, SecretStoreAvailability, SecretStoreError, SecureSecretStore,
        UnavailableSecretStore,
    },
};

use super::catalog::AuthMode;

const MAX_CREDENTIAL_BYTES: usize = 8192;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialPersistence {
    Secure,
    Session,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CredentialStorage {
    None,
    Secure,
    Session,
}

impl CredentialStorage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Secure => "secure",
            Self::Session => "session",
        }
    }

    pub(crate) fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "none" => Ok(Self::None),
            "secure" => Ok(Self::Secure),
            "session" => Ok(Self::Session),
            _ => Err(CommandError::new(
                "invalid_ai_credential_storage",
                "The saved AI credential-storage state is invalid.",
            )),
        }
    }
}

impl From<CredentialPersistence> for CredentialStorage {
    fn from(value: CredentialPersistence) -> Self {
        match value {
            CredentialPersistence::Secure => Self::Secure,
            CredentialPersistence::Session => Self::Session,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialStatus {
    NotRequired,
    Missing,
    SavedSecurely,
    SessionOnly,
    SecureStoreUnavailable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatusDto {
    pub status: CredentialStatus,
    pub authentication_required: bool,
    pub secure_persistence_available: bool,
    pub session_only_available: bool,
}

pub type SecretValue = SecretBytes;

pub struct SecretManager {
    secure: Arc<dyn SecureSecretStore>,
    session: Mutex<HashMap<String, SecretValue>>,
}

impl Default for SecretManager {
    fn default() -> Self {
        Self::new(Arc::new(UnavailableSecretStore))
    }
}

impl SecretManager {
    pub fn new(secure: Arc<dyn SecureSecretStore>) -> Self {
        Self {
            secure,
            session: Mutex::new(HashMap::new()),
        }
    }

    pub fn status(
        &self,
        profile_id: &str,
        auth_mode: AuthMode,
        storage: CredentialStorage,
    ) -> Result<CredentialStatusDto, CommandError> {
        let secure_available = self.secure.availability() == SecretStoreAvailability::Available;
        if !auth_mode.requires_credential() {
            return Ok(CredentialStatusDto {
                status: CredentialStatus::NotRequired,
                authentication_required: false,
                secure_persistence_available: secure_available,
                session_only_available: false,
            });
        }

        match storage {
            CredentialStorage::None => Ok(CredentialStatusDto {
                status: CredentialStatus::Missing,
                authentication_required: true,
                secure_persistence_available: secure_available,
                session_only_available: true,
            }),
            CredentialStorage::Session => Ok(CredentialStatusDto {
                status: if self.session_contains(profile_id)? {
                    CredentialStatus::SessionOnly
                } else {
                    CredentialStatus::Missing
                },
                authentication_required: true,
                secure_persistence_available: secure_available,
                session_only_available: true,
            }),
            CredentialStorage::Secure => match self.secure.read(&SecretLocator::ai_profile(profile_id)) {
                Ok(Some(_secret)) => Ok(CredentialStatusDto {
                    status: CredentialStatus::SavedSecurely,
                    authentication_required: true,
                    secure_persistence_available: true,
                    session_only_available: true,
                }),
                Ok(None) => Ok(CredentialStatusDto {
                    status: CredentialStatus::Missing,
                    authentication_required: true,
                    secure_persistence_available: secure_available,
                    session_only_available: true,
                }),
                Err(SecretStoreError::Unavailable | SecretStoreError::Locked) => {
                    Ok(CredentialStatusDto {
                        status: CredentialStatus::SecureStoreUnavailable,
                        authentication_required: true,
                        secure_persistence_available: false,
                        session_only_available: true,
                    })
                }
                Err(_) => {
                    logging::error("ai_secret_store_error", "ai_secure_store_failed");
                    Ok(CredentialStatusDto {
                        status: CredentialStatus::SecureStoreUnavailable,
                        authentication_required: true,
                        secure_persistence_available: false,
                        session_only_available: true,
                    })
                }
            },
        }
    }

    pub fn resolve(
        &self,
        profile_id: &str,
        auth_mode: AuthMode,
        storage: CredentialStorage,
    ) -> Result<Option<SecretValue>, CommandError> {
        if !auth_mode.requires_credential() {
            return Ok(None);
        }

        match storage {
            CredentialStorage::None => Err(CommandError::new(
                "ai_credential_required",
                "This AI provider profile needs a credential.",
            )),
            CredentialStorage::Session => self
                .take_session_copy(profile_id)?
                .map(Some)
                .ok_or_else(|| {
                    CommandError::new(
                        "ai_credential_required",
                        "This session-only AI credential expired when ostudy closed. Enter it again.",
                    )
                }),
            CredentialStorage::Secure => match self.secure.read(&SecretLocator::ai_profile(profile_id)) {
                Ok(Some(secret)) => Ok(Some(secret)),
                Ok(None) => Err(CommandError::new(
                    "ai_credential_required",
                    "The securely saved AI credential is missing. Enter it again.",
                )),
                Err(SecretStoreError::Unavailable | SecretStoreError::Locked) => Err(
                    CommandError::new(
                        "ai_secure_store_unavailable",
                        "The operating-system credential store is unavailable.",
                    ),
                ),
                Err(_) => Err(secret_command_error(
                    "ai_secure_store_failed",
                    "The operating-system credential store could not be read.",
                )),
            },
        }
    }

    pub fn set(
        &self,
        profile_id: &str,
        credential: String,
        persistence: CredentialPersistence,
        previous_storage: CredentialStorage,
    ) -> Result<(), CommandError> {
        validate_credential(&credential)?;
        let locator = SecretLocator::ai_profile(profile_id);
        match persistence {
            CredentialPersistence::Secure => {
                self.secure
                    .write(&locator, credential.as_bytes())
                    .map_err(map_write_error)?;
                self.remove_session(profile_id)?;
            }
            CredentialPersistence::Session => {
                if previous_storage == CredentialStorage::Secure {
                    self.secure.delete(&locator).map_err(map_delete_error)?;
                }
                self.insert_session(profile_id, credential.into_bytes())?;
            }
        }
        Ok(())
    }

    pub fn clear(&self, profile_id: &str, storage: CredentialStorage) -> Result<(), CommandError> {
        match storage {
            CredentialStorage::None | CredentialStorage::Session => self.remove_session(profile_id),
            CredentialStorage::Secure => {
                self.secure
                    .delete(&SecretLocator::ai_profile(profile_id))
                    .map_err(map_delete_error)?;
                self.remove_session(profile_id)
            }
        }
    }

    fn session_contains(&self, profile_id: &str) -> Result<bool, CommandError> {
        let guard = self.session.lock().map_err(|_| secret_lock_error())?;
        Ok(guard.contains_key(profile_id))
    }

    fn take_session_copy(&self, profile_id: &str) -> Result<Option<SecretValue>, CommandError> {
        let guard = self.session.lock().map_err(|_| secret_lock_error())?;
        Ok(guard
            .get(profile_id)
            .map(|value| SecretValue::new(value.as_bytes().to_vec())))
    }

    fn insert_session(&self, profile_id: &str, bytes: Vec<u8>) -> Result<(), CommandError> {
        let mut guard = self.session.lock().map_err(|_| secret_lock_error())?;
        guard.insert(profile_id.to_string(), SecretValue::new(bytes));
        Ok(())
    }

    fn remove_session(&self, profile_id: &str) -> Result<(), CommandError> {
        let mut guard = self.session.lock().map_err(|_| secret_lock_error())?;
        guard.remove(profile_id);
        Ok(())
    }
}

fn validate_credential(value: &str) -> Result<(), CommandError> {
    if value.trim().is_empty() || value.len() > MAX_CREDENTIAL_BYTES {
        return Err(CommandError::new(
            "invalid_ai_credential",
            format!("Enter a non-empty AI credential no longer than {MAX_CREDENTIAL_BYTES} bytes."),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(CommandError::new(
            "invalid_ai_credential",
            "AI credentials cannot contain control characters.",
        ));
    }
    Ok(())
}

fn map_write_error(error: SecretStoreError) -> CommandError {
    match error {
        SecretStoreError::Unavailable | SecretStoreError::Locked => CommandError::new(
            "ai_secure_store_unavailable",
            "Secure credential storage is unavailable. Use a session-only credential instead.",
        ),
        _ => secret_command_error(
            "ai_secure_store_failed",
            "The operating-system credential could not be saved.",
        ),
    }
}

fn map_delete_error(error: SecretStoreError) -> CommandError {
    match error {
        SecretStoreError::Unavailable | SecretStoreError::Locked => CommandError::new(
            "ai_secure_store_unavailable",
            "The operating-system credential store is unavailable, so ostudy cannot confirm credential removal.",
        ),
        _ => secret_command_error(
            "ai_secure_store_failed",
            "The operating-system credential could not be removed.",
        ),
    }
}

fn secret_lock_error() -> CommandError {
    CommandError::new(
        "ai_secret_lock_poisoned",
        "The in-memory AI credential state is unavailable. Restart ostudy.",
    )
}

fn secret_command_error(code: &str, message: &str) -> CommandError {
    logging::error("ai_secret_store_error", code);
    CommandError::new(code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_credentials_are_never_exposed_by_status() {
        let manager = SecretManager::default();
        manager
            .set(
                "0123456789abcdef0123456789abcdef",
                "sentinel-secret".to_string(),
                CredentialPersistence::Session,
                CredentialStorage::None,
            )
            .expect("session credential should save");
        let status = manager
            .status(
                "0123456789abcdef0123456789abcdef",
                AuthMode::Bearer,
                CredentialStorage::Session,
            )
            .expect("status should load");
        assert_eq!(status.status, CredentialStatus::SessionOnly);
        let serialized = serde_json::to_string(&status).expect("status should serialize");
        assert!(!serialized.contains("sentinel-secret"));
        assert!(manager
            .resolve(
                "0123456789abcdef0123456789abcdef",
                AuthMode::Bearer,
                CredentialStorage::Session,
            )
            .is_ok());
    }

    #[test]
    fn session_storage_reports_missing_after_restart_without_reading_secure_status() {
        let manager = SecretManager::default();
        let status = manager
            .status(
                "fedcba9876543210fedcba9876543210",
                AuthMode::Bearer,
                CredentialStorage::Session,
            )
            .expect("status should load without touching the secure store");
        assert_eq!(status.status, CredentialStatus::Missing);
    }

    #[test]
    fn credential_storage_round_trips_database_values() {
        for storage in [
            CredentialStorage::None,
            CredentialStorage::Secure,
            CredentialStorage::Session,
        ] {
            assert_eq!(
                CredentialStorage::from_db(storage.as_str()).unwrap(),
                storage
            );
        }
        assert!(CredentialStorage::from_db("plaintext").is_err());
    }

    #[test]
    fn rejects_empty_and_control_character_credentials() {
        assert!(validate_credential("").is_err());
        assert!(validate_credential("bad\nkey").is_err());
    }
}
`);

await write("src-tauri/src/system_resources.rs", String.raw`//! Resource-aware concurrency planning for CPU-heavy native document work.
//!
//! Platform adapters observe CPU and memory. The shared planner applies task,
//! CPU, RAM, thermal, lifecycle, and optional operator bounds without knowing
//! which operating system produced the snapshot.

const MIB: u64 = 1024 * 1024;
const MIN_MEMORY_RESERVE_BYTES: u64 = 512 * MIB;
const MEMORY_RESERVE_PERCENT: u64 = 25;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SystemResourceSnapshot {
    pub logical_cpus: usize,
    pub available_memory_bytes: u64,
    pub low_memory: bool,
    pub thermal_limited: bool,
    pub foreground: bool,
}

impl SystemResourceSnapshot {
    pub(crate) fn capture() -> Self {
        let snapshot = crate::platform::resource_snapshot();
        Self {
            logical_cpus: snapshot.logical_cpus,
            available_memory_bytes: snapshot.available_memory_bytes,
            low_memory: snapshot.low_memory,
            thermal_limited: matches!(
                snapshot.thermal_state,
                crate::platform::ThermalState::Serious | crate::platform::ThermalState::Critical
            ),
            foreground: snapshot.foreground,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParallelWorkPlan {
    pub logical_cpus: usize,
    pub available_memory_bytes: u64,
    pub working_memory_budget_bytes: u64,
    pub per_worker_memory_bytes: u64,
    pub worker_count: usize,
}

pub(crate) fn plan_parallel_workers(
    task_count: usize,
    per_worker_memory_bytes: u64,
    environment_cap: &str,
) -> ParallelWorkPlan {
    calculate_parallel_workers(
        SystemResourceSnapshot::capture(),
        task_count,
        per_worker_memory_bytes,
        environment_worker_cap(environment_cap),
    )
}

pub(crate) fn queue_capacity(
    plan: ParallelWorkPlan,
    task_count: usize,
    per_queue_item_bytes: u64,
    target_items_per_worker: usize,
) -> usize {
    if task_count == 0 || plan.worker_count == 0 {
        return 0;
    }
    let cpu_pipeline_target = plan
        .worker_count
        .saturating_mul(target_items_per_worker.max(1))
        .min(task_count)
        .max(1);
    if per_queue_item_bytes == 0 || plan.working_memory_budget_bytes == 0 {
        return cpu_pipeline_target;
    }
    let worker_memory = plan
        .per_worker_memory_bytes
        .saturating_mul(u64::try_from(plan.worker_count).unwrap_or(u64::MAX));
    let queue_budget = plan
        .working_memory_budget_bytes
        .saturating_sub(worker_memory);
    let memory_slots = usize::try_from(queue_budget / per_queue_item_bytes)
        .unwrap_or(usize::MAX)
        .max(1);
    cpu_pipeline_target.min(memory_slots).max(1)
}

fn calculate_parallel_workers(
    snapshot: SystemResourceSnapshot,
    task_count: usize,
    per_worker_memory_bytes: u64,
    explicit_cap: Option<usize>,
) -> ParallelWorkPlan {
    let logical_cpus = snapshot.logical_cpus.max(1);
    let working_memory_budget_bytes = working_memory_budget(snapshot.available_memory_bytes);
    let memory_worker_limit = if working_memory_budget_bytes == 0 || per_worker_memory_bytes == 0 {
        usize::MAX
    } else {
        usize::try_from(working_memory_budget_bytes / per_worker_memory_bytes)
            .unwrap_or(usize::MAX)
            .max(1)
    };
    let environmental_limit = if snapshot.low_memory || snapshot.thermal_limited || !snapshot.foreground {
        1
    } else {
        usize::MAX
    };
    let mut worker_count = if task_count == 0 {
        0
    } else {
        task_count
            .min(logical_cpus)
            .min(memory_worker_limit)
            .min(environmental_limit)
            .max(1)
    };
    if let Some(cap) = explicit_cap.filter(|cap| *cap > 0) {
        worker_count = worker_count.min(cap);
    }
    ParallelWorkPlan {
        logical_cpus,
        available_memory_bytes: snapshot.available_memory_bytes,
        working_memory_budget_bytes,
        per_worker_memory_bytes,
        worker_count,
    }
}

fn working_memory_budget(available_memory_bytes: u64) -> u64 {
    if available_memory_bytes == 0 {
        return 0;
    }
    let percentage_reserve = available_memory_bytes.saturating_mul(MEMORY_RESERVE_PERCENT) / 100;
    let minimum_reserve = MIN_MEMORY_RESERVE_BYTES.min(available_memory_bytes / 2);
    available_memory_bytes.saturating_sub(percentage_reserve.max(minimum_reserve))
}

fn environment_worker_cap(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gib(value: u64) -> u64 {
        value * 1024 * MIB
    }

    fn snapshot(logical_cpus: usize, memory: u64) -> SystemResourceSnapshot {
        SystemResourceSnapshot {
            logical_cpus,
            available_memory_bytes: memory,
            low_memory: false,
            thermal_limited: false,
            foreground: true,
        }
    }

    #[test]
    fn worker_plan_can_be_cpu_limited_without_a_fixed_ceiling() {
        assert_eq!(
            calculate_parallel_workers(snapshot(64, gib(128)), 100, 256 * MIB, None).worker_count,
            64
        );
    }

    #[test]
    fn worker_plan_is_memory_limited_before_machine_exhaustion() {
        let plan = calculate_parallel_workers(snapshot(64, gib(4)), 100, 512 * MIB, None);
        assert_eq!(plan.working_memory_budget_bytes, gib(3));
        assert_eq!(plan.worker_count, 6);
    }

    #[test]
    fn worker_plan_is_task_limited_and_accepts_an_operator_cap() {
        let value = snapshot(64, gib(128));
        assert_eq!(calculate_parallel_workers(value, 3, 128 * MIB, None).worker_count, 3);
        assert_eq!(calculate_parallel_workers(value, 100, 128 * MIB, Some(12)).worker_count, 12);
    }

    #[test]
    fn low_memory_thermal_and_background_states_are_conservative() {
        for mut value in [snapshot(16, gib(32)), snapshot(16, gib(32)), snapshot(16, gib(32))] {
            if value == snapshot(16, gib(32)) {
                value.low_memory = true;
            }
            assert_eq!(calculate_parallel_workers(value, 20, 128 * MIB, None).worker_count, 1);
        }
        let mut thermal = snapshot(16, gib(32));
        thermal.thermal_limited = true;
        assert_eq!(calculate_parallel_workers(thermal, 20, 128 * MIB, None).worker_count, 1);
        let mut background = snapshot(16, gib(32));
        background.foreground = false;
        assert_eq!(calculate_parallel_workers(background, 20, 128 * MIB, None).worker_count, 1);
    }

    #[test]
    fn missing_memory_information_falls_back_to_cpu_and_task_bounds() {
        let plan = calculate_parallel_workers(snapshot(24, 0), 100, 512 * MIB, None);
        assert_eq!(plan.worker_count, 24);
        assert_eq!(plan.working_memory_budget_bytes, 0);
    }

    #[test]
    fn queue_capacity_respects_remaining_memory_and_pipeline_depth() {
        let plan = calculate_parallel_workers(snapshot(8, gib(8)), 20, 512 * MIB, None);
        assert_eq!(queue_capacity(plan, 20, 256 * MIB, 2), 8);
    }
}
`);

await write("src-tauri/src/util.rs", String.raw`use rusqlite::Connection;

use crate::error::CommandError;

pub fn unix_time_millis() -> Result<i64, CommandError> {
    crate::platform::now_utc_millis().map_err(|message| {
        CommandError::new("invalid_system_clock", message)
    })
}

pub fn generate_id(connection: &Connection) -> Result<String, CommandError> {
    connection
        .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
        .map_err(CommandError::database)
}

pub fn validate_entity_id(value: &str, label: &str) -> Result<(), CommandError> {
    let valid = value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(CommandError::new(
            "invalid_identifier",
            format!("The {label} identifier is invalid."),
        ))
    }
}

pub fn bool_to_integer(value: bool) -> i64 {
    i64::from(value)
}
`);

let calendar = await read("src-tauri/src/study_calendar.rs");
let calendarStart = calendar
  .replace("use std::{env, path::Path};\n\n", "")
  .replace(
    "use chrono::{DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, Offset, TimeZone};",
    "use chrono::{DateTime, Datelike, Duration, NaiveDate};",
  )
  .replace(
    "    fn identifier(&self) -> String {\n        match self {\n            Self::System => discover_system_time_zone_id(),",
    "    fn identifier(&self, now_utc_millis: i64) -> Result<String, CommandError> {\n        match self {\n            Self::System => crate::platform::time_zone_snapshot(now_utc_millis)\n                .map(|snapshot| snapshot.identifier)\n                .map_err(platform_calendar_error),",
  )
  .replace(
    "            Self::Pacific2026 => \"America/Los_Angeles\".to_string(),\n        }\n    }",
    "            Self::Pacific2026 => Ok(\"America/Los_Angeles\".to_string()),\n        }\n    }",
  )
  .replace(
    "            Self::System => Local\n                .timestamp_millis_opt(utc_millis)\n                .earliest()\n                .map(|value| value.date_naive())\n                .ok_or_else(invalid_calendar_date),",
    "            Self::System => crate::platform::local_date_at(utc_millis)\n                .map_err(platform_calendar_error),",
  )
  .replace(
    "            Self::System => Local\n                .timestamp_millis_opt(utc_millis)\n                .earliest()\n                .map(|value| value.offset().fix().local_minus_utc().div_euclid(60))\n                .ok_or_else(invalid_calendar_date),",
    "            Self::System => crate::platform::time_zone_snapshot(utc_millis)\n                .map(|snapshot| snapshot.utc_offset_minutes)\n                .map_err(platform_calendar_error),",
  )
  .replace(
    "            Self::System => first_valid_system_instant(date),",
    "            Self::System => crate::platform::first_valid_instant(date)\n                .map_err(platform_calendar_error),",
  )
  .replace(
    "            time_zone_id: zone.identifier(),",
    "            time_zone_id: zone.identifier(now_utc_millis)?,",
  );

const firstSystem = calendarStart.indexOf("fn first_valid_system_instant(");
const parseDate = calendarStart.indexOf("pub fn parse_local_date", firstSystem);
if (firstSystem < 0 || parseDate < 0) throw new Error("study_calendar.rs: system calendar helper range not found");
let calendarMiddle = calendarStart.slice(0, firstSystem) + calendarStart.slice(parseDate);
const discover = calendarMiddle.indexOf("fn discover_system_time_zone_id()");
const checkedAdd = calendarMiddle.indexOf("fn checked_add_days", discover);
if (discover < 0 || checkedAdd < 0) throw new Error("study_calendar.rs: zone discovery range not found");
let calendarFinal = calendarMiddle.slice(0, discover)
  + "fn platform_calendar_error(message: String) -> CommandError {\n    CommandError::new(\"invalid_study_calendar_date\", message)\n}\n\n"
  + calendarMiddle.slice(checkedAdd);
await write("src-tauri/src/study_calendar.rs", calendarFinal);

await replace(
  "src-tauri/src/ai/mod.rs",
  "use std::sync::Mutex;\n\nuse gateway::AiGateway;\nuse secrets::SecretManager;",
  "use std::sync::{Arc, Mutex};\n\nuse gateway::AiGateway;\nuse secrets::SecretManager;\nuse crate::platform::SecureSecretStore;",
);
await replace(
  "src-tauri/src/ai/mod.rs",
  "    pub fn new() -> Result<Self, String> {\n        Ok(Self {\n            secrets: SecretManager::default(),",
  "    pub fn new(secret_store: Arc<dyn SecureSecretStore>) -> Result<Self, String> {\n        Ok(Self {\n            secrets: SecretManager::new(secret_store),",
);
await replace(
  "src-tauri/src/ai/mod.rs",
  "        AiRuntimeState::new().expect(\"AI runtime should initialize\");",
  "        AiRuntimeState::new(crate::platform::unavailable_secret_store())\n            .expect(\"AI runtime should initialize\");",
);

await replace(
  "src-tauri/Cargo.toml",
  "keyring = \"=4.1.6\"\n",
  "",
);
await replace(
  "src-tauri/Cargo.toml",
  "sysinfo = { version = \"=0.39.6\", default-features = false, features = [\"system\"] }\n",
  "",
);
await replace(
  "src-tauri/Cargo.toml",
  "[target.'cfg(not(any(target_os = \"android\", target_os = \"ios\")))'.dependencies]\nimage =",
  "[target.'cfg(not(any(target_os = \"android\", target_os = \"ios\")))'.dependencies]\nkeyring = \"=4.1.6\"\nsysinfo = { version = \"=0.39.6\", default-features = false, features = [\"system\"] }\nimage =",
);

let lib = await read("src-tauri/src/lib.rs");
let libUpdated = lib
  .replace("mod paths;\n", "mod paths;\nmod platform;\n")
  .replace("use tauri::Manager;", "use tauri::{Emitter, Manager};")
  .replace(
    "            let database = Database::open(paths.database_path()).map_err(io::Error::other)?;\n            let ai_runtime = AiRuntimeState::new().map_err(io::Error::other)?;",
    "            let database = Database::open(paths.database_path()).map_err(io::Error::other)?;\n            let platform_runtime = platform::PlatformRuntime::current();\n            platform::install_global(platform_runtime.clone()).map_err(io::Error::other)?;\n            let ai_runtime = AiRuntimeState::new(platform_runtime.secure_secret_store())\n                .map_err(io::Error::other)?;",
  )
  .replace(
    "            if !app.manage(ai_runtime) {",
    "            if !app.manage(platform_runtime.clone()) {\n                return Err(io::Error::other(\"The platform runtime was already initialized.\").into());\n            }\n            if !app.manage(ai_runtime) {",
  )
  .replace(
    "            logging::info(\n                \"application_started\",",
    "            let lifecycle = platform_runtime\n                .lifecycle()\n                .transition(platform::LifecyclePhase::Foreground);\n            let _ = app.emit(\"ostudy-lifecycle\", lifecycle);\n\n            logging::info(\n                \"application_started\",",
  )
  .replace(
    "            let tauri::WindowEvent::CloseRequested { api, .. } = event else {",
    "            let tauri::WindowEvent::CloseRequested { api, .. } = event else {",
  )
  .replace(
    "            if window.label() != \"main\" || !task_cancellation::has_active_document_tasks() {\n                return;\n            }",
    "            let lifecycle = window.state::<platform::PlatformRuntime>().lifecycle();\n            let requested = lifecycle.transition(platform::LifecyclePhase::TerminationRequested);\n            let _ = window.emit(\"ostudy-lifecycle\", requested);\n            if window.label() != \"main\" || !task_cancellation::has_active_document_tasks() {\n                let terminating = lifecycle.transition(platform::LifecyclePhase::Terminating);\n                let _ = window.emit(\"ostudy-lifecycle\", terminating);\n                return;\n            }",
  )
  .replace(
    "            let window = window.clone();\n            window",
    "            let window = window.clone();\n            let lifecycle_after = lifecycle.clone();\n            window",
  )
  .replace(
    "                    if should_close {\n                        let _ = window.destroy();\n                    }",
    "                    if should_close {\n                        let terminating = lifecycle_after.transition(platform::LifecyclePhase::Terminating);\n                        let _ = window.emit(\"ostudy-lifecycle\", terminating);\n                        let _ = window.destroy();\n                    } else {\n                        let foreground = lifecycle_after.transition(platform::LifecyclePhase::Foreground);\n                        let _ = window.emit(\"ostudy-lifecycle\", foreground);\n                    }",
  )
  .replace(
    "            settings::get_app_info,",
    "            settings::get_app_info,\n            platform::get_platform_capabilities,",
  );
if (libUpdated === lib) throw new Error("src-tauri/src/lib.rs was not updated");
await write("src-tauri/src/lib.rs", libUpdated);

await write("src/ipc/platform.ts", String.raw`import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type CapabilityState =
  | "available"
  | "permission_required"
  | "denied"
  | "unavailable"
  | "degraded";

export interface ServiceCapability {
  state: CapabilityState;
  reasonCode: string | null;
  canRequestPermission: boolean;
}

export interface NativeServiceCapabilities {
  externalFileImport: ServiceCapability;
  externalFileExport: ServiceCapability;
  secureSecrets: ServiceCapability;
  lifecycleEvents: ServiceCapability;
  localTimeZone: ServiceCapability;
  resourceObservation: ServiceCapability;
  nativePdfRender: ServiceCapability;
  nativePdfTextRescue: ServiceCapability;
  ocr: ServiceCapability;
  camera: ServiceCapability;
  qrScan: ServiceCapability;
  haptics: ServiceCapability;
  systemShare: ServiceCapability;
  localNetwork: ServiceCapability;
  backgroundExecution: ServiceCapability;
}

export interface NativePlatformSnapshot {
  schemaVersion: number;
  runtimeFamily: "desktop" | "mobile";
  operatingSystem: string;
  hardwareBackAvailable: boolean;
  services: NativeServiceCapabilities;
}

export type LifecyclePhase =
  | "starting"
  | "foreground"
  | "background"
  | "suspending"
  | "resuming"
  | "termination_requested"
  | "terminating";

export interface LifecycleSnapshot {
  schemaVersion: number;
  sequence: number;
  phase: LifecyclePhase;
  observedAtUtcMillis: number;
}

export async function getNativePlatformSnapshot(): Promise<NativePlatformSnapshot> {
  return invoke<NativePlatformSnapshot>("get_platform_capabilities");
}

export async function listenToLifecycle(
  listener: (snapshot: LifecycleSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<LifecycleSnapshot>("ostudy-lifecycle", (event) => listener(event.payload));
}
`);

await write("src/platform/capabilities.ts", String.raw`import {
  getNativePlatformSnapshot,
  type NativePlatformSnapshot,
} from "../ipc/platform";

export interface PlatformCapabilities {
  coarsePointer: boolean;
  hoverAvailable: boolean;
  hardwareBackAvailable: boolean;
  audioAvailable: boolean;
  hapticsAvailable: boolean;
  nativeFilePickerAvailable: boolean;
  native: NativePlatformSnapshot | null;
}

let nativeSnapshot: NativePlatformSnapshot | null = null;
const listeners = new Set<() => void>();

function matches(query: string): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia(query).matches
    : false;
}

export async function initializePlatformCapabilities(): Promise<void> {
  nativeSnapshot = await getNativePlatformSnapshot();
  for (const listener of listeners) listener();
}

export function subscribePlatformCapabilities(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getPlatformCapabilities(): PlatformCapabilities {
  return {
    coarsePointer: matches("(pointer: coarse)"),
    hoverAvailable: matches("(hover: hover)"),
    hardwareBackAvailable: nativeSnapshot?.hardwareBackAvailable ?? false,
    audioAvailable: typeof Audio !== "undefined",
    hapticsAvailable: nativeSnapshot?.services.haptics.state === "available",
    nativeFilePickerAvailable:
      nativeSnapshot?.services.externalFileImport.state === "available",
    native: nativeSnapshot,
  };
}
`);

await write("src/platform/PlatformProvider.tsx", String.raw`import {
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useState,
} from "react";

import { getErrorMessage } from "../ipc/common";
import {
  getPlatformCapabilities,
  initializePlatformCapabilities,
  subscribePlatformCapabilities,
  type PlatformCapabilities,
} from "./capabilities";

const PlatformContext = createContext<PlatformCapabilities>(getPlatformCapabilities());

export function PlatformProvider({ children }: { children: ReactNode }) {
  const [snapshot, setSnapshot] = useState(getPlatformCapabilities);

  useEffect(() => {
    const unsubscribe = subscribePlatformCapabilities(() => setSnapshot(getPlatformCapabilities()));
    void initializePlatformCapabilities().catch((error: unknown) => {
      console.error("Unable to initialize native platform capabilities:", getErrorMessage(error));
    });
    return unsubscribe;
  }, []);

  return <PlatformContext.Provider value={snapshot}>{children}</PlatformContext.Provider>;
}

export function usePlatformCapabilities(): PlatformCapabilities {
  return useContext(PlatformContext);
}
`);

await replace(
  "src/main.tsx",
  'import { OverlayProvider } from "./ui/overlays/OverlayProvider";',
  'import { OverlayProvider } from "./ui/overlays/OverlayProvider";\nimport { PlatformProvider } from "./platform/PlatformProvider";',
);
await replace(
  "src/main.tsx",
  "  <StrictMode>\n    <OverlayProvider>",
  "  <StrictMode>\n    <PlatformProvider>\n      <OverlayProvider>",
);
await replace(
  "src/main.tsx",
  "    </OverlayProvider>\n  </StrictMode>,",
  "      </OverlayProvider>\n    </PlatformProvider>\n  </StrictMode>,",
);

const policy = await read("scripts/verify-platform-boundaries.mjs");
await write(
  "scripts/verify-platform-boundaries.mjs",
  policy
    .replace(
      '    if (/\\bkeyring(?:::|\\b)/u.test(text)) report("rust-keyring-outside-adapter", displayPath);\n    if (/\\bsysinfo(?:::|\\b)/u.test(text)) report("rust-sysinfo-outside-adapter", displayPath);\n    if (/\\bSystemTime(?:::|\\b)/u.test(text)) report("rust-direct-system-clock", displayPath);',
      '    const desktopAdapter = displayPath.startsWith("src-tauri/src/platform/desktop/");\n    if (!desktopAdapter && /\\bkeyring(?:::|\\b)/u.test(text)) report("rust-keyring-outside-adapter", displayPath);\n    if (!desktopAdapter && /\\bsysinfo(?:::|\\b)/u.test(text)) report("rust-sysinfo-outside-adapter", displayPath);\n    if (!desktopAdapter && /\\bSystemTime(?:::|\\b)/u.test(text)) report("rust-direct-system-clock", displayPath);',
    )
    .replace(
      '    if (\n      /\\bchrono::(?:[^;\\n]*\\bLocal\\b|\\{[^}]*\\bLocal\\b)/u.test(text)\n      || /\\/etc\\/(?:localtime|timezone)/u.test(text)\n    ) {',
      '    if (!desktopAdapter && (\n      /\\bchrono::(?:[^;\\n]*\\bLocal\\b|\\{[^}]*\\bLocal\\b)/u.test(text)\n      || /\\/etc\\/(?:localtime|timezone)/u.test(text)\n    )) {',
    )
    .replace(
      '    if (/\\btarget_os\\s*=\\s*"/u.test(text)) report("rust-target-os-branch", displayPath);\n    if (/\\b(?:std::process::Command|Command::new)\\b/u.test(text)) {',
      '    if (!desktopAdapter && /\\btarget_os\\s*=\\s*"/u.test(text)) report("rust-target-os-branch", displayPath);\n    if (!desktopAdapter && /\\b(?:std::process::Command|Command::new)\\b/u.test(text)) {',
    ),
);

const exceptions = JSON.parse(await read("scripts/policy/platform-boundary-exceptions.json"));
const retiredP1 = new Set([
  "frontend-user-agent-platform-detection:src/platform/capabilities.ts",
  "rust-keyring-outside-adapter:src-tauri/src/ai/secrets.rs",
  "rust-sysinfo-outside-adapter:src-tauri/src/system_resources.rs",
  "rust-direct-system-clock:src-tauri/src/util.rs",
  "rust-direct-local-time-zone:src-tauri/src/study_calendar.rs",
]);
exceptions.exceptions = exceptions.exceptions.filter(
  (entry) => !retiredP1.has(`${entry.rule}:${entry.path}`),
);
await write("scripts/policy/platform-boundary-exceptions.json", `${JSON.stringify(exceptions, null, 2)}\n`);

await replace(
  "scripts/verify-project-manifests.mjs",
  '["verify:contracts", "node ./scripts/verify-ipc-contract.mjs && node ./scripts/verify-repository-policy.mjs"],',
  '["verify:contracts", "node ./scripts/verify-ipc-contract.mjs && node ./scripts/verify-repository-policy.mjs && node ./scripts/verify-platform-boundaries.mjs"],',
);

await rm(join(root, "scripts", "apply-platform-p1.mjs"), { force: true });
await rm(join(root, ".github", "workflows", "apply-platform-p1.yml"), { force: true });
console.log("Applied P1 runtime ports and removed the temporary applicator.");
