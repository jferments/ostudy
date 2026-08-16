use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::{models::*, qr_models::*, Result};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "io.github.jferments.ostudy.platform";

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> Result<OstudyPlatform<R>> {
    #[cfg(target_os = "android")]
    let platform = api.register_android_plugin(PLUGIN_IDENTIFIER, "OstudyPlatformPlugin")?;
    #[cfg(target_os = "android")]
    let documents = api.register_android_plugin(PLUGIN_IDENTIFIER, "OstudyDocumentPlugin")?;
    #[cfg(target_os = "android")]
    let qr = api.register_android_plugin(PLUGIN_IDENTIFIER, "OstudyQrPlugin")?;
    #[cfg(target_os = "ios")]
    compile_error!("The ostudy platform plugin does not yet provide an iOS implementation.");
    Ok(OstudyPlatform {
        platform,
        documents,
        qr,
    })
}

pub struct OstudyPlatform<R: Runtime> {
    platform: PluginHandle<R>,
    documents: PluginHandle<R>,
    qr: PluginHandle<R>,
}

impl<R: Runtime> OstudyPlatform<R> {
    pub fn environment(&self) -> Result<EnvironmentSnapshot> {
        self.platform
            .run_mobile_plugin("getEnvironment", ())
            .map_err(Into::into)
    }

    pub fn time_zone_snapshot(&self, utc_millis: i64) -> Result<TimeZoneSnapshotResult> {
        self.platform
            .run_mobile_plugin("timeZoneSnapshot", TimestampRequest { utc_millis })
            .map_err(Into::into)
    }

    pub fn local_date_at(&self, utc_millis: i64) -> Result<LocalDateResult> {
        self.platform
            .run_mobile_plugin("localDateAt", LocalDateRequest { utc_millis })
            .map_err(Into::into)
    }

    pub fn first_valid_instant(&self, date: String) -> Result<FirstValidInstantResult> {
        self.platform
            .run_mobile_plugin("firstValidInstant", FirstValidInstantRequest { date })
            .map_err(Into::into)
    }

    pub fn acquire_import(&self, request: AcquireImportRequest) -> Result<AcquireImportResult> {
        self.platform
            .run_mobile_plugin("acquireImport", request)
            .map_err(Into::into)
    }

    pub fn publish_export(&self, request: PublishExportRequest) -> Result<PublishExportResult> {
        self.platform
            .run_mobile_plugin("publishExport", request)
            .map_err(Into::into)
    }

    pub fn share_file(&self, request: ShareFileRequest) -> Result<ShareFileResult> {
        self.platform
            .run_mobile_plugin("shareFile", request)
            .map_err(Into::into)
    }

    pub fn set_back_interception(&self, enabled: bool) -> Result<()> {
        self.platform
            .run_mobile_plugin("setBackInterception", BackInterceptionRequest { enabled })
            .map_err(Into::into)
    }

    pub fn secret_availability(&self) -> Result<SecretAvailabilityResult> {
        self.platform
            .run_mobile_plugin("secretAvailability", ())
            .map_err(Into::into)
    }

    pub fn read_secret(&self, locator: String) -> Result<ReadSecretResult> {
        self.platform
            .run_mobile_plugin("readSecret", SecretRequest { locator })
            .map_err(Into::into)
    }

    pub fn write_secret(&self, locator: String, value_base64: String) -> Result<()> {
        self.platform
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
        self.platform
            .run_mobile_plugin("deleteSecret", SecretRequest { locator })
            .map_err(Into::into)
    }

    pub fn list_incoming_shares(&self) -> Result<Vec<IncomingShareSummary>> {
        let result: IncomingShareListResult = self
            .platform
            .run_mobile_plugin("listIncomingShares", ())
            .map_err(crate::Error::from)?;
        Ok(result.items)
    }

    pub fn take_incoming_share(&self, token: String) -> Result<TakenIncomingShare> {
        self.platform
            .run_mobile_plugin("takeIncomingShare", IncomingShareRequest { token })
            .map_err(Into::into)
    }

    pub fn dismiss_incoming_share(&self, token: String) -> Result<()> {
        self.platform
            .run_mobile_plugin("dismissIncomingShare", IncomingShareRequest { token })
            .map_err(Into::into)
    }

    pub fn scan_qr_code(&self, request: ScanQrCodeRequest) -> Result<ScanQrCodeResult> {
        self.qr
            .run_mobile_plugin("scanQrCode", request)
            .map_err(Into::into)
    }

    pub fn document_capabilities(&self) -> Result<DocumentCapabilitiesResult> {
        self.documents
            .run_mobile_plugin("documentCapabilities", ())
            .map_err(Into::into)
    }

    pub fn open_pdf(&self, request: OpenPdfRequest) -> Result<OpenPdfResult> {
        self.documents
            .run_mobile_plugin("openPdf", request)
            .map_err(Into::into)
    }

    pub fn render_pdf_page(&self, request: RenderPdfPageRequest) -> Result<RenderPdfPageResult> {
        self.documents
            .run_mobile_plugin("renderPdfPage", request)
            .map_err(Into::into)
    }

    pub fn close_pdf(&self, token: String) -> Result<()> {
        self.documents
            .run_mobile_plugin("closePdf", ClosePdfRequest { token })
            .map_err(Into::into)
    }

    pub fn recognize_image(&self, request: RecognizeImageRequest) -> Result<RecognizeImageResult> {
        self.documents
            .run_mobile_plugin("recognizeImage", request)
            .map_err(Into::into)
    }

    pub fn begin_background_execution(
        &self,
        request: BeginBackgroundExecutionRequest,
    ) -> Result<()> {
        self.documents
            .run_mobile_plugin("beginBackgroundExecution", request)
            .map_err(Into::into)
    }

    pub fn update_background_execution(
        &self,
        request: UpdateBackgroundExecutionRequest,
    ) -> Result<()> {
        self.documents
            .run_mobile_plugin("updateBackgroundExecution", request)
            .map_err(Into::into)
    }

    pub fn end_background_execution(&self, job_id: String) -> Result<()> {
        self.documents
            .run_mobile_plugin(
                "endBackgroundExecution",
                EndBackgroundExecutionRequest { job_id },
            )
            .map_err(Into::into)
    }
}
