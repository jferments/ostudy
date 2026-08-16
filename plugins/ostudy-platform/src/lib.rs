use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

#[cfg(desktop)]
mod desktop;
mod error;
#[cfg(mobile)]
mod mobile;
mod models;
mod qr_models;

#[cfg(desktop)]
pub use desktop::OstudyPlatform;
pub use error::{Error, Result};
#[cfg(mobile)]
pub use mobile::OstudyPlatform;
pub use models::*;
pub use qr_models::*;

pub trait OstudyPlatformExt<R: Runtime> {
    fn ostudy_platform(&self) -> &OstudyPlatform<R>;
}

impl<R: Runtime, T: Manager<R>> OstudyPlatformExt<R> for T {
    fn ostudy_platform(&self) -> &OstudyPlatform<R> {
        self.state::<OstudyPlatform<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("ostudy-platform")
        .setup(|app, api| {
            #[cfg(mobile)]
            let platform = mobile::init(app, api)?;
            #[cfg(desktop)]
            let platform = desktop::init(app, api)?;
            app.manage(platform);
            Ok(())
        })
        .build()
}

#[cfg(mobile)]
pub fn init_documents<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("ostudy-documents")
        .setup(|app, api| {
            let documents = mobile::init_documents(app, api)?;
            app.manage(documents);
            Ok(())
        })
        .build()
}

#[cfg(mobile)]
pub fn init_qr<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("ostudy-qr")
        .setup(|app, api| {
            let qr = mobile::init_qr(app, api)?;
            app.manage(qr);
            Ok(())
        })
        .build()
}
