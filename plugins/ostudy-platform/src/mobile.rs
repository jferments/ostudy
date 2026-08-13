use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::{models::*, Result};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "io.github.jferments.ostudy.platform";

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> Result<OstudyPlatform<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "OstudyPlatformPlugin")?;
    #[cfg(target_os = "ios")]
    compile_error!("The ostudy platform plugin does not yet provide an iOS implementation.");
    Ok(OstudyPlatform(handle))
}

pub struct OstudyPlatform<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> OstudyPlatform<R> {
    pub fn environment(&self) -> Result<EnvironmentSnapshot> {
        self.0
            .run_mobile_plugin("getEnvironment", ())
            .map_err(Into::into)
    }

    pub fn time_zone_snapshot(&self, utc_millis: i64) -> Result<TimeZoneSnapshotResult> {
        self.0
            .run_mobile_plugin("timeZoneSnapshot", TimestampRequest { utc_millis })
            .map_err(Into::into)
    }

    pub fn local_date_at(&self, utc_millis: i64) -> Result<LocalDateResult> {
        self.0
            .run_mobile_plugin("localDateAt", LocalDateRequest { utc_millis })
            .map_err(Into::into)
    }

    pub fn first_valid_instant(&self, date: String) -> Result<FirstValidInstantResult> {
        self.0
            .run_mobile_plugin("firstValidInstant", FirstValidInstantRequest { date })
            .map_err(Into::into)
    }

    pub fn acquire_import(&self, request: AcquireImportRequest) -> Result<AcquireImportResult> {
        self.0
            .run_mobile_plugin("acquireImport", request)
            .map_err(Into::into)
    }

    pub fn publish_export(&self, request: PublishExportRequest) -> Result<PublishExportResult> {
        self.0
            .run_mobile_plugin("publishExport", request)
            .map_err(Into::into)
    }

    pub fn share_file(&self, request: ShareFileRequest) -> Result<ShareFileResult> {
        self.0
            .run_mobile_plugin("shareFile", request)
            .map_err(Into::into)
    }

    pub fn set_back_interception(&self, enabled: bool) -> Result<()> {
        self.0
            .run_mobile_plugin("setBackInterception", BackInterceptionRequest { enabled })
            .map_err(Into::into)
    }

    pub fn secret_availability(&self) -> Result<SecretAvailabilityResult> {
        self.0
            .run_mobile_plugin("secretAvailability", ())
            .map_err(Into::into)
    }

    pub fn read_secret(&self, locator: String) -> Result<ReadSecretResult> {
        self.0
            .run_mobile_plugin("readSecret", SecretRequest { locator })
            .map_err(Into::into)
    }

    pub fn write_secret(&self, locator: String, value_base64: String) -> Result<()> {
        self.0
            .run_mobile_plugin(
                "writeSecret",
                WriteSecretRequest {
                    locator,
                    value_base64,
                },
            )
            .map_err(Into::into)
    }

    pub fn delete_secret(&self, locator: String) -> Result<()> {
        self.0
            .run_mobile_plugin("deleteSecret", SecretRequest { locator })
            .map_err(Into::into)
    }

    pub fn list_incoming_shares(&self) -> Result<Vec<IncomingShareSummary>> {
        self.0
            .run_mobile_plugin("listIncomingShares", ())
            .map_err(Into::into)
    }

    pub fn take_incoming_share(&self, token: String) -> Result<TakenIncomingShare> {
        self.0
            .run_mobile_plugin("takeIncomingShare", IncomingShareRequest { token })
            .map_err(Into::into)
    }

    pub fn dismiss_incoming_share(&self, token: String) -> Result<()> {
        self.0
            .run_mobile_plugin("dismissIncomingShare", IncomingShareRequest { token })
            .map_err(Into::into)
    }
}
