use sysinfo::{MemoryRefreshKind, System};

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
