//! Resource-aware concurrency planning for CPU-heavy native document work.
//!
//! Document extraction and OCR should use the machine that the learner has
//! available without hard-coding a workstation-specific thread count. Plans are
//! bounded by logical CPUs, currently available RAM, task count, and an optional
//! operator cap. Memory values are conservative working-set estimates, not
//! allocations or promises that an external process will stay below them.

use sysinfo::{MemoryRefreshKind, System};

const MIB: u64 = 1024 * 1024;
const MIN_MEMORY_RESERVE_BYTES: u64 = 512 * MIB;
const MEMORY_RESERVE_PERCENT: u64 = 25;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SystemResourceSnapshot {
    pub logical_cpus: usize,
    pub available_memory_bytes: u64,
}

impl SystemResourceSnapshot {
    pub(crate) fn capture() -> Self {
        let logical_cpus = std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(1);
        let mut system = System::new();
        system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        Self {
            logical_cpus: logical_cpus.max(1),
            available_memory_bytes: system.available_memory(),
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
    let cap = environment_worker_cap(environment_cap);
    calculate_parallel_workers(
        SystemResourceSnapshot::capture(),
        task_count,
        per_worker_memory_bytes,
        cap,
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

    let mut worker_count = if task_count == 0 {
        0
    } else {
        task_count.min(logical_cpus).min(memory_worker_limit).max(1)
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
    // On small-memory systems, reserving a full 512 MiB could consume virtually
    // everything. Reserve at least 25%, and aim for 512 MiB only when doing so
    // still leaves at least half the currently available RAM for the workload.
    let minimum_reserve = MIN_MEMORY_RESERVE_BYTES.min(available_memory_bytes / 2);
    let reserve = percentage_reserve.max(minimum_reserve);
    available_memory_bytes.saturating_sub(reserve)
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

    #[test]
    fn worker_plan_can_be_cpu_limited_without_a_fixed_ceiling() {
        let plan = calculate_parallel_workers(
            SystemResourceSnapshot {
                logical_cpus: 64,
                available_memory_bytes: gib(128),
            },
            100,
            256 * MIB,
            None,
        );
        assert_eq!(plan.worker_count, 64);
    }

    #[test]
    fn worker_plan_is_memory_limited_before_machine_exhaustion() {
        let plan = calculate_parallel_workers(
            SystemResourceSnapshot {
                logical_cpus: 64,
                available_memory_bytes: gib(4),
            },
            100,
            512 * MIB,
            None,
        );
        assert_eq!(plan.working_memory_budget_bytes, gib(3));
        assert_eq!(plan.worker_count, 6);
    }

    #[test]
    fn worker_plan_is_task_limited_and_accepts_an_operator_cap() {
        let snapshot = SystemResourceSnapshot {
            logical_cpus: 64,
            available_memory_bytes: gib(128),
        };
        assert_eq!(
            calculate_parallel_workers(snapshot, 3, 128 * MIB, None).worker_count,
            3
        );
        assert_eq!(
            calculate_parallel_workers(snapshot, 100, 128 * MIB, Some(12)).worker_count,
            12
        );
    }

    #[test]
    fn missing_memory_information_falls_back_to_cpu_and_task_bounds() {
        let plan = calculate_parallel_workers(
            SystemResourceSnapshot {
                logical_cpus: 24,
                available_memory_bytes: 0,
            },
            100,
            512 * MIB,
            None,
        );
        assert_eq!(plan.worker_count, 24);
        assert_eq!(plan.working_memory_budget_bytes, 0);
    }

    #[test]
    fn queue_capacity_respects_remaining_memory_and_pipeline_depth() {
        let plan = calculate_parallel_workers(
            SystemResourceSnapshot {
                logical_cpus: 8,
                available_memory_bytes: gib(8),
            },
            20,
            512 * MIB,
            None,
        );
        // 8 GiB available -> 6 GiB working budget; 8 workers reserve 4 GiB,
        // leaving 2 GiB. At 256 MiB per queued image, memory permits 8 slots,
        // even though a two-deep worker pipeline would otherwise target 16.
        assert_eq!(queue_capacity(plan, 20, 256 * MIB, 2), 8);
    }
}
