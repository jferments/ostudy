mod capabilities;
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
