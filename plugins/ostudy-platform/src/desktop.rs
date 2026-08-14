use std::marker::PhantomData;

use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::Result;

/// Desktop placeholder for the private mobile platform plugin.
///
/// The desktop implementation does not own an `R`; it only carries the runtime
/// type at compile time so the extension trait has one shape on desktop and
/// mobile. `PhantomData<fn() -> R>` intentionally avoids inheriting `R`'s
/// `Send`/`Sync` auto-trait requirements. Tauri managed state itself must be
/// `Send + Sync`, and there is no runtime value here that needs those bounds.
pub struct OstudyPlatform<R: Runtime>(PhantomData<fn() -> R>);

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<OstudyPlatform<R>> {
    Ok(OstudyPlatform(PhantomData))
}

#[cfg(test)]
mod tests {
    use super::OstudyPlatform;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn desktop_platform_state_is_safe_for_tauri_managed_state() {
        assert_send_sync::<OstudyPlatform<tauri::Wry>>();
    }
}
