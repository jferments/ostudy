//! Platform foreground-execution envelope for durable document jobs.
//!
//! Android needs a visible foreground service while a user-initiated PDF job
//! continues after the WebView is backgrounded. Desktop and iOS currently use a
//! no-op guard. Durable checkpoints and process-death recovery remain owned by
//! the document domain; this module only manages the operating-system execution
//! lease.

#[cfg(target_os = "android")]
use tauri::AppHandle;
#[cfg(target_os = "android")]
use tauri_plugin_ostudy_platform::{
    BeginBackgroundExecutionRequest, OstudyPlatformExt, UpdateBackgroundExecutionRequest,
};

#[derive(Debug)]
pub(crate) struct DocumentExecutionLease {
    #[cfg(target_os = "android")]
    app: AppHandle,
    #[cfg(target_os = "android")]
    job_id: String,
}

impl DocumentExecutionLease {
    #[cfg(target_os = "android")]
    pub(crate) fn begin(
        app: AppHandle,
        job_id: impl Into<String>,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<Self, String> {
        let job_id = job_id.into();
        app.ostudy_platform()
            .begin_background_execution(BeginBackgroundExecutionRequest {
                job_id: job_id.clone(),
                title: title.into(),
                detail: detail.into(),
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { app, job_id })
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn begin(
        _app: tauri::AppHandle,
        _job_id: impl Into<String>,
        _title: impl Into<String>,
        _detail: impl Into<String>,
    ) -> Result<Self, String> {
        Ok(Self {})
    }

    #[cfg(target_os = "android")]
    pub(crate) fn update(&self, detail: impl Into<String>, progress_percent: Option<u8>) {
        let _ = self
            .app
            .ostudy_platform()
            .update_background_execution(UpdateBackgroundExecutionRequest {
                job_id: self.job_id.clone(),
                detail: detail.into(),
                progress_percent,
            });
    }

    #[cfg(not(target_os = "android"))]
    pub(crate) fn update(&self, _detail: impl Into<String>, _progress_percent: Option<u8>) {}
}

#[cfg(target_os = "android")]
impl Drop for DocumentExecutionLease {
    fn drop(&mut self) {
        let _ = self
            .app
            .ostudy_platform()
            .end_background_execution(self.job_id.clone());
    }
}
