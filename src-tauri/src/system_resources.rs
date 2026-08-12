//! Resource-aware concurrency planning for CPU-heavy native document work.
//!
//! Platform adapters observe CPU and memory. The shared planner applies task,
//! CPU, RAM, thermal, lifecycle, and optional operator bounds without knowing
//! which operating system produced the snapshot.

const MIB: u64 = 1024 * 1024;
const MIN_MEMORY_RESERVE_BYTES: u64 = 512 * MIB;
const MEMORY_RESERVE_PERCENT: u64 = 25;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SystemResourceSnapshot {
    pub logical_cpus: usize,
    pub available_memory_bytes: u64,
    pub low_memory: bool,
    pub thermal_limited: bool,
    pub foreground: bool,
}

impl SystemResourceSnapshot {
    pub(crate) fn capture() -> Self {
        let snapshot = crate::platform::resource_snapshot();
        Self {
            logical_cpus: snapshot.logical_cpus,
            available_memory_bytes: snapshot.available_memory_bytes,
            low_memory: snapshot.low_memory,
            thermal_limited: matches!(
                snapshot.thermal_state,
                crate::platform::ThermalState::Serious | crate::platform::ThermalState::Critical
            ),
            foreground: snapshot.foreground,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParallelWorkPlan {
    pub logical_cpus: usize,
    pub available_memory_bytes: u64,
    pub working_memory_budget_bytes: u64,
    pub per_worker_memory_bytes: u64,
    pub worker_count: usize,
}

pub(crate) fn plan_parallel_workers(
    task_count: usize,
    per_worker_memory_bytes: u64,
    environment_cap: &str,
) -> ParallelWorkPlan {
    calculate_parallel_workers(
        SystemResourceSnapshot::capture(),
        task_count,
        per_worker_memory_bytes,
        environment_worker_cap(environment_cap),
    )
}

pub(crate) fn queue_capacity(
    plan: ParallelWorkPlan,
    task_count: usize,
    per_queue_item_bytes: u64,
    target_items_per_worker: usize,
) -> usize {
    if task_count == 0 || plan.worker_count == 0 {
        return 0;
    }
    let cpu_pipeline_target = plan
        .worker_count
        .saturating_mul(target_items_per_worker.max(1))
        .min(task_count)
        .max(1);
    if per_queue_item_bytes == 0 || plan.working_memory_budget_bytes == 0 {
        return cpu_pipeline_target;
    }
    let worker_memory = plan
        .per_worker_memory_bytes
        .saturating_mul(u64::try_from(plan.worker_count).unwrap_or(u64::MAX));
    let queue_budget = plan
        .working_memory_budget_bytes
        .saturating_sub(worker_memory);
    let memory_slots = usize::try_from(queue_budget / per_queue_item_bytes)
        .unwrap_or(usize::MAX)
        .max(1);
    cpu_pipeline_target.min(memory_slots).max(1)
}

fn calculate_parallel_workers(
    snapshot: SystemResourceSnapshot,
    task_count: usize,
    per_worker_memory_bytes: u64,
    explicit_cap: Option<usize>,
) -> ParallelWorkPlan {
    let logical_cpus = snapshot.logical_cpus.max(1);
    let working_memory_budget_bytes = working_memory_budget(snapshot.available_memory_bytes);
    let memory_worker_limit = if working_memory_budget_bytes == 0 || per_worker_memory_bytes == 0 {
        usize::MAX
    } else {
        usize::try_from(working_memory_budget_bytes / per_worker_memory_bytes)
            .unwrap_or(usize::MAX)
            .max(1)
    };
    let environmental_limit =
        if snapshot.low_memory || snapshot.thermal_limited || !snapshot.foreground {
            1
        } else {
            usize::MAX
        };
    let mut worker_count = if task_count == 0 {
        0
    } else {
        task_count
            .min(logical_cpus)
            .min(memory_worker_limit)
            .min(environmental_limit)
            .max(1)
    };
    if let Some(cap) = explicit_cap.filter(|cap| *cap > 0) {
        worker_count = worker_count.min(cap);
    }
    ParallelWorkPlan {
        logical_cpus,
        available_memory_bytes: snapshot.available_memory_bytes,
        working_memory_budget_bytes,
        per_worker_memory_bytes,
        worker_count,
    }
}

fn working_memory_budget(available_memory_bytes: u64) -> u64 {
    if available_memory_bytes == 0 {
        return 0;
    }
    let percentage_reserve = available_memory_bytes.saturating_mul(MEMORY_RESERVE_PERCENT) / 100;
    let minimum_reserve = MIN_MEMORY_RESERVE_BYTES.min(available_memory_bytes / 2);
    available_memory_bytes.saturating_sub(percentage_reserve.max(minimum_reserve))
}

fn environment_worker_cap(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gib(value: u64) -> u64 {
        value * 1024 * MIB
    }

    fn snapshot(logical_cpus: usize, memory: u64) -> SystemResourceSnapshot {
        SystemResourceSnapshot {
            logical_cpus,
            available_memory_bytes: memory,
            low_memory: false,
            thermal_limited: false,
            foreground: true,
        }
    }

    #[test]
    fn worker_plan_can_be_cpu_limited_without_a_fixed_ceiling() {
        assert_eq!(
            calculate_parallel_workers(snapshot(64, gib(128)), 100, 256 * MIB, None).worker_count,
            64
        );
    }

    #[test]
    fn worker_plan_is_memory_limited_before_machine_exhaustion() {
        let plan = calculate_parallel_workers(snapshot(64, gib(4)), 100, 512 * MIB, None);
        assert_eq!(plan.working_memory_budget_bytes, gib(3));
        assert_eq!(plan.worker_count, 6);
    }

    #[test]
    fn worker_plan_is_task_limited_and_accepts_an_operator_cap() {
        let value = snapshot(64, gib(128));
        assert_eq!(
            calculate_parallel_workers(value, 3, 128 * MIB, None).worker_count,
            3
        );
        assert_eq!(
            calculate_parallel_workers(value, 100, 128 * MIB, Some(12)).worker_count,
            12
        );
    }

    #[test]
    fn low_memory_thermal_and_background_states_are_conservative() {
        for mut value in [
            snapshot(16, gib(32)),
            snapshot(16, gib(32)),
            snapshot(16, gib(32)),
        ] {
            if value == snapshot(16, gib(32)) {
                value.low_memory = true;
            }
            assert_eq!(
                calculate_parallel_workers(value, 20, 128 * MIB, None).worker_count,
                1
            );
        }
        let mut thermal = snapshot(16, gib(32));
        thermal.thermal_limited = true;
        assert_eq!(
            calculate_parallel_workers(thermal, 20, 128 * MIB, None).worker_count,
            1
        );
        let mut background = snapshot(16, gib(32));
        background.foreground = false;
        assert_eq!(
            calculate_parallel_workers(background, 20, 128 * MIB, None).worker_count,
            1
        );
    }

    #[test]
    fn missing_memory_information_falls_back_to_cpu_and_task_bounds() {
        let plan = calculate_parallel_workers(snapshot(24, 0), 100, 512 * MIB, None);
        assert_eq!(plan.worker_count, 24);
        assert_eq!(plan.working_memory_budget_bytes, 0);
    }

    #[test]
    fn queue_capacity_respects_remaining_memory_and_pipeline_depth() {
        let plan = calculate_parallel_workers(snapshot(8, gib(8)), 20, 512 * MIB, None);
        assert_eq!(queue_capacity(plan, 20, 256 * MIB, 2), 8);
    }
}
