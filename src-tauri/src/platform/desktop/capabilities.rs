use crate::platform::{
    speech_recognition, speech_synthesis, CapabilityProvider, NativeServiceCapabilities,
    PlatformCapabilitySnapshot, ServiceCapability,
};

pub(super) struct DesktopCapabilityProvider;

impl CapabilityProvider for DesktopCapabilityProvider {
    fn snapshot(&self) -> PlatformCapabilitySnapshot {
        PlatformCapabilitySnapshot {
            schema_version: 2,
            runtime_family: "desktop".to_string(),
            operating_system: std::env::consts::OS.to_string(),
            hardware_back_available: false,
            services: NativeServiceCapabilities {
                window_maximize_restore: ServiceCapability::available(),
                window_full_screen: ServiceCapability::available(),
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
                browser_sign_in: ServiceCapability::available(),
                on_device_speech: on_device_speech(),
            },
        }
    }
}

fn on_device_speech() -> ServiceCapability {
    if speech_synthesis::SUPPORTED && speech_recognition::SUPPORTED {
        ServiceCapability::available()
    } else {
        ServiceCapability::unavailable("on_device_speech_not_qualified")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::CapabilityState;

    #[test]
    fn desktop_reports_native_window_controls() {
        let snapshot = DesktopCapabilityProvider.snapshot();

        assert_eq!(snapshot.schema_version, 2);
        assert_eq!(snapshot.runtime_family, "desktop");
        assert_eq!(
            snapshot.services.window_maximize_restore.state,
            CapabilityState::Available
        );
        assert_eq!(
            snapshot.services.window_full_screen.state,
            CapabilityState::Available
        );
    }

    #[test]
    fn desktop_reports_on_device_speech_where_the_build_has_it() {
        let state = DesktopCapabilityProvider
            .snapshot()
            .services
            .on_device_speech
            .state;

        if cfg!(target_os = "windows") {
            assert_eq!(state, CapabilityState::Unavailable);
        } else {
            assert_eq!(state, CapabilityState::Available);
        }
    }
}
