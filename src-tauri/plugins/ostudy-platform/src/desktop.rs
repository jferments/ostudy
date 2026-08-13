use std::marker::PhantomData;
use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};
use crate::Result;

pub struct OstudyPlatform<R: Runtime>(PhantomData<fn() -> R>);

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<OstudyPlatform<R>> {
    Ok(OstudyPlatform(PhantomData))
}
