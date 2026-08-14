//! Typed operating-system capability ports and runtime composition.

#[cfg(target_os = "android")]
mod android;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop;
#[cfg(any(target_os = "android", target_os = "ios"))]
mod mobile_stub;

use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
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

#[derive(Clone, Debug)]
pub struct ExternalImportTarget {
    pub path: PathBuf,
    pub maximum_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ExternalAcquireResult {
    pub display_name: String,
    pub reported_media_type: Option<String>,
    pub copied_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ExternalExportSource {
    pub path: PathBuf,
    pub display_name: String,
    pub media_type: String,
    pub maximum_bytes: u64,
}

#[derive(Clone, Debug)]
pub struct ExternalPublishResult {
    pub display_name: String,
    pub copied_bytes: u64,
    pub destination_kind: String,
}

pub type PlatformFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait ExternalFilePort: Send + Sync {
    fn select_import(
        &self,
        _app: AppHandle,
        _request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalSource>, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "external_file_adapter_unavailable",
                "This build does not provide path-based external-file selection.",
            ))
        })
    }

    fn select_export(
        &self,
        _app: AppHandle,
        _request: ExternalFileRequest,
    ) -> PlatformFuture<Result<Option<ExternalDestination>, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "external_file_adapter_unavailable",
                "This build does not provide path-based export destinations.",
            ))
        })
    }

    fn acquire_import(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
        target: ExternalImportTarget,
    ) -> PlatformFuture<Result<Option<ExternalAcquireResult>, CommandError>>;

    fn publish_export(
        &self,
        app: AppHandle,
        request: ExternalFileRequest,
        source: ExternalExportSource,
    ) -> PlatformFuture<Result<Option<ExternalPublishResult>, CommandError>>;
}

#[derive(Clone, Debug)]
pub struct ShareSource {
    pub title: String,
    pub path: PathBuf,
    pub display_name: String,
    pub media_type: String,
}

#[derive(Clone, Debug)]
pub struct ShareLaunchResult {
    pub launched: bool,
    pub destination_kind: String,
}

pub trait SystemSharePort: Send + Sync {
    fn share_file(
        &self,
        app: AppHandle,
        source: ShareSource,
    ) -> PlatformFuture<Result<ShareLaunchResult, CommandError>>;
}

#[derive(Default)]
pub struct UnavailableSystemSharePort;

impl SystemSharePort for UnavailableSystemSharePort {
    fn share_file(
        &self,
        _app: AppHandle,
        _source: ShareSource,
    ) -> PlatformFuture<Result<ShareLaunchResult, CommandError>> {
        Box::pin(async {
            Err(CommandError::new(
                "system_share_unavailable",
                "This build does not provide a native system share sheet.",
            ))
        })
    }
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
    system_share: Arc<dyn SystemSharePort>,
}

impl PlatformRuntime {
    pub fn current() -> Self {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            desktop::runtime()
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        {
            mobile_stub::runtime()
        }
    }

    pub fn for_app(app: AppHandle) -> Self {
        #[cfg(target_os = "android")]
        {
            android::runtime(app)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = app;
            Self::current()
        }
    }

    pub(crate) fn from_parts(
        capabilities: Arc<dyn CapabilityProvider>,
        external_files: Arc<dyn ExternalFilePort>,
        secure_secrets: Arc<dyn SecureSecretStore>,
        clock: Arc<dyn Clock>,
        time_zone: Arc<dyn TimeZoneProvider>,
        resources: Arc<dyn ResourceProbe>,
        system_share: Arc<dyn SystemSharePort>,
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
            system_share,
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

    pub fn system_share(&self) -> Arc<dyn SystemSharePort> {
        Arc::clone(&self.system_share)
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

#[tauri::command]
pub fn set_platform_back_interception(app: AppHandle, enabled: bool) -> Result<(), CommandError> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_ostudy_platform::OstudyPlatformExt;
        app.ostudy_platform()
            .set_back_interception(enabled)
            .map_err(|error| {
                CommandError::new(
                    "android_back_interception_failed",
                    format!("Android Back interception could not be updated: {error}"),
                )
            })
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, enabled);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn unavailable_secret_store() -> Arc<dyn SecureSecretStore> {
    Arc::new(UnavailableSecretStore)
}
