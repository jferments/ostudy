//! Bounded concurrency policy for independent document-provider windows.
//!
//! Concurrency follows the execution topology, not a list of provider brands.
//! Cloud requests can overlap because every request owns a disjoint bounded
//! source window and durable commits remain source-ordered. Local and LAN model
//! servers remain serial so ostudy does not assume that one user-managed
//! inference process was started with multiple decoding slots.

use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, LazyLock,
    },
};

use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

use crate::{
    error::CommandError,
    task_cancellation::{current_signal, CancellationSignal},
};

pub(crate) const CLOUD_DOCUMENT_PARALLELISM: usize = 4;
pub(crate) const CLOUD_GLOBAL_PROVIDER_PARALLELISM: usize = 6;
const CLOUD_INITIAL_PROVIDER_PARALLELISM: usize = 4;
const CLOUD_MIN_PROVIDER_PARALLELISM: usize = 2;
const CLOUD_SUCCESS_RAMP_REQUESTS: usize = 8;
const USER_MANAGED_GLOBAL_PROVIDER_PARALLELISM: usize = 1;

pub(crate) fn document_request_parallelism(network_scope: &str) -> usize {
    match network_scope {
        "cloud" => CLOUD_DOCUMENT_PARALLELISM,
        _ => 1,
    }
}

#[derive(Debug)]
struct CloudAdaptiveState {
    active: AtomicUsize,
    limit: AtomicUsize,
    maximum: usize,
    successes: AtomicUsize,
    notify: Notify,
}

impl CloudAdaptiveState {
    fn new(maximum: usize) -> Self {
        let maximum = maximum.max(1);
        Self {
            active: AtomicUsize::new(0),
            limit: AtomicUsize::new(CLOUD_INITIAL_PROVIDER_PARALLELISM.min(maximum).max(1)),
            maximum,
            successes: AtomicUsize::new(0),
            notify: Notify::new(),
        }
    }

    async fn reserve(&self, cancellation: Option<&CancellationSignal>) -> Result<(), CommandError> {
        loop {
            let notified = self.notify.notified();
            let limit = self.limit.load(Ordering::Acquire);
            let active = self.active.load(Ordering::Acquire);
            if active < limit
                && self
                    .active
                    .compare_exchange(active, active + 1, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                return Ok(());
            }
            match cancellation {
                Some(signal) => tokio::select! {
                    _ = signal.canceled() => return Err(provider_lane_canceled()),
                    _ = notified => {}
                },
                None => notified.await,
            }
        }
    }

    fn release(&self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
        self.notify.notify_waiters();
    }

    fn record_outcome(&self, succeeded: bool, retryable_failure: bool) -> Option<(usize, usize)> {
        if retryable_failure {
            self.successes.store(0, Ordering::Release);
            let minimum = CLOUD_MIN_PROVIDER_PARALLELISM.min(self.maximum).max(1);
            let previous = self
                .limit
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    Some(current.saturating_sub(1).max(minimum))
                })
                .ok()?;
            let current = self.limit.load(Ordering::Acquire);
            return (current != previous).then_some((previous, current));
        }
        if !succeeded {
            return None;
        }
        let successes = self.successes.fetch_add(1, Ordering::AcqRel) + 1;
        if successes < CLOUD_SUCCESS_RAMP_REQUESTS {
            return None;
        }
        self.successes.store(0, Ordering::Release);
        let previous = self
            .limit
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                Some(current.saturating_add(1).min(self.maximum))
            })
            .ok()?;
        let current = self.limit.load(Ordering::Acquire);
        if current != previous {
            self.notify.notify_waiters();
            Some((previous, current))
        } else {
            None
        }
    }

    #[cfg(test)]
    fn limit(&self) -> usize {
        self.limit.load(Ordering::Acquire)
    }
}

#[derive(Debug)]
pub(crate) struct ProviderSlot {
    _permit: OwnedSemaphorePermit,
    cloud: Option<Arc<CloudAdaptiveState>>,
}

impl Drop for ProviderSlot {
    fn drop(&mut self) {
        if let Some(cloud) = self.cloud.as_ref() {
            cloud.release();
        }
    }
}

#[derive(Debug)]
struct ProviderLanes {
    cloud: Arc<Semaphore>,
    cloud_state: Arc<CloudAdaptiveState>,
    user_managed: Arc<Semaphore>,
}

impl ProviderLanes {
    fn new(cloud_slots: usize) -> Self {
        let cloud_slots = cloud_slots.max(1);
        Self {
            cloud: Arc::new(Semaphore::new(cloud_slots)),
            cloud_state: Arc::new(CloudAdaptiveState::new(cloud_slots)),
            user_managed: Arc::new(Semaphore::new(USER_MANAGED_GLOBAL_PROVIDER_PARALLELISM)),
        }
    }

    fn lane(&self, network_scope: &str) -> Arc<Semaphore> {
        match network_scope {
            "cloud" => self.cloud.clone(),
            _ => self.user_managed.clone(),
        }
    }

    async fn acquire(
        &self,
        network_scope: &str,
        cancellation: Option<&CancellationSignal>,
    ) -> Result<ProviderSlot, CommandError> {
        let signal = cancellation.cloned().or_else(current_signal);
        let acquire = self.lane(network_scope).acquire_owned();
        let permit = match signal.as_ref() {
            Some(signal) => tokio::select! {
                _ = signal.canceled() => return Err(provider_lane_canceled()),
                permit = acquire => permit.map_err(|_| provider_lane_closed())?,
            },
            None => acquire.await.map_err(|_| provider_lane_closed())?,
        };
        let cloud = (network_scope == "cloud").then(|| self.cloud_state.clone());
        if let Some(state) = cloud.as_ref() {
            state.reserve(signal.as_ref()).await?;
        }
        Ok(ProviderSlot {
            _permit: permit,
            cloud,
        })
    }

    fn record_outcome(
        &self,
        network_scope: &str,
        succeeded: bool,
        retryable_failure: bool,
    ) -> Option<(usize, usize)> {
        (network_scope == "cloud")
            .then(|| {
                self.cloud_state
                    .record_outcome(succeeded, retryable_failure)
            })
            .flatten()
    }
}

static PROVIDER_LANES: LazyLock<ProviderLanes> =
    LazyLock::new(|| ProviderLanes::new(CLOUD_GLOBAL_PROVIDER_PARALLELISM));

pub(crate) async fn acquire_provider_slot(
    network_scope: &str,
    cancellation: Option<&CancellationSignal>,
) -> Result<ProviderSlot, CommandError> {
    PROVIDER_LANES.acquire(network_scope, cancellation).await
}

pub(crate) fn record_provider_outcome(
    network_scope: &str,
    succeeded: bool,
    retryable_failure: bool,
) {
    if let Some((previous, current)) =
        PROVIDER_LANES.record_outcome(network_scope, succeeded, retryable_failure)
    {
        crate::logging::debug_target(
            "ai.concurrency",
            "cloud_provider_parallelism_adapted",
            "Adjusted the global cloud-provider lane from content-free request outcomes.",
            serde_json::json!({
                "previousLimit": previous,
                "currentLimit": current,
                "maximumLimit": CLOUD_GLOBAL_PROVIDER_PARALLELISM,
                "cause": if retryable_failure { "retryable_failure" } else { "success_ramp" },
            }),
        );
    }
}

fn provider_lane_canceled() -> CommandError {
    CommandError::new(
        "ai_request_canceled",
        "The AI request was canceled before a provider slot became available.",
    )
}

fn provider_lane_closed() -> CommandError {
    CommandError::new(
        "ai_provider_lane_closed",
        "The provider concurrency lane closed unexpectedly.",
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueueControl {
    Continue,
    Stop,
}

/// Run bounded provider work without batch barriers.
///
/// A completed slot is consumed before that slot receives another item. Once a
/// consumer requests a stop, no new work is dispatched, but already-started
/// siblings are drained so callers can checkpoint any successful paid work.
pub(crate) async fn run_bounded_provider_queue<Run, RunFuture, Item, Consume, Error>(
    work_items: usize,
    parallelism: usize,
    run: Run,
    mut consume: Consume,
) -> Result<(), Error>
where
    Run: Fn(usize) -> RunFuture,
    RunFuture: Future<Output = Item>,
    Consume: FnMut(Item) -> Result<QueueControl, Error>,
{
    if work_items == 0 {
        return Ok(());
    }

    let worker_count = parallelism
        .clamp(1, CLOUD_DOCUMENT_PARALLELISM)
        .min(work_items);
    let next = AtomicUsize::new(0);
    let (sender, mut receiver) =
        tokio::sync::mpsc::unbounded_channel::<(Item, tokio::sync::oneshot::Sender<bool>)>();
    let run = &run;
    let (worker0, worker1, worker2, worker3) = {
        let worker = |slot: usize| {
            let sender = sender.clone();
            let next = &next;
            async move {
                if slot >= worker_count {
                    return;
                }
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= work_items {
                        return;
                    }
                    let item = run(index).await;
                    let (acknowledge, acknowledged) = tokio::sync::oneshot::channel();
                    if sender.send((item, acknowledge)).is_err() {
                        return;
                    }
                    if !matches!(acknowledged.await, Ok(true)) {
                        return;
                    }
                }
            }
        };
        (worker(0), worker(1), worker(2), worker(3))
    };
    drop(sender);

    let collector = async move {
        let mut stopping = false;
        let mut first_consumer_error = None;
        while let Some((item, acknowledge)) = receiver.recv().await {
            let control = if first_consumer_error.is_some() {
                QueueControl::Stop
            } else {
                match consume(item) {
                    Ok(control) => control,
                    Err(error) => {
                        first_consumer_error = Some(error);
                        QueueControl::Stop
                    }
                }
            };
            if control == QueueControl::Stop {
                stopping = true;
            }
            let _ = acknowledge.send(!stopping);
        }
        first_consumer_error.map_or(Ok(()), Err)
    };

    let (_, _, _, _, result) = tokio::join!(worker0, worker1, worker2, worker3, collector);
    result
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, sync::Mutex};

    use super::*;

    #[test]
    fn cloud_windows_overlap_while_user_managed_servers_stay_serial() {
        assert_eq!(document_request_parallelism("cloud"), 4);
        assert_eq!(document_request_parallelism("local"), 1);
        assert_eq!(document_request_parallelism("lan"), 1);
    }

    #[tokio::test]
    async fn global_provider_lane_bounds_cloud_overlap_and_serializes_user_managed_servers() {
        async fn observed_max(lanes: &ProviderLanes, scope: &str) -> usize {
            let active = AtomicUsize::new(0);
            let maximum = AtomicUsize::new(0);
            run_bounded_provider_queue(
                6,
                4,
                |_| {
                    let active = &active;
                    let maximum = &maximum;
                    async move {
                        let _permit = lanes.acquire(scope, None).await.unwrap();
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        maximum.fetch_max(now, Ordering::SeqCst);
                        for _ in 0..32 {
                            tokio::task::yield_now().await;
                        }
                        active.fetch_sub(1, Ordering::SeqCst);
                    }
                },
                |_| Ok::<QueueControl, ()>(QueueControl::Continue),
            )
            .await
            .unwrap();
            maximum.load(Ordering::SeqCst)
        }

        let lanes = ProviderLanes::new(2);
        assert_eq!(observed_max(&lanes, "cloud").await, 2);
        assert_eq!(observed_max(&lanes, "local").await, 1);
        assert_eq!(observed_max(&lanes, "lan").await, 1);
    }

    #[test]
    fn cloud_lane_backs_off_on_retryable_failures_and_ramps_after_successes() {
        let lanes = ProviderLanes::new(6);
        assert_eq!(lanes.cloud_state.limit(), 4);
        assert_eq!(lanes.record_outcome("cloud", false, true), Some((4, 3)));
        assert_eq!(lanes.record_outcome("cloud", false, true), Some((3, 2)));
        assert_eq!(lanes.record_outcome("cloud", false, true), None);
        for _ in 0..CLOUD_SUCCESS_RAMP_REQUESTS {
            let result = lanes.record_outcome("cloud", true, false);
            if result.is_some() {
                assert_eq!(result, Some((2, 3)));
            }
        }
        assert_eq!(lanes.cloud_state.limit(), 3);
        assert_eq!(lanes.record_outcome("local", false, true), None);
    }

    #[tokio::test]
    async fn canceling_while_waiting_for_a_provider_lane_does_not_start_work() {
        let lanes = ProviderLanes::new(1);
        let _held = lanes.acquire("cloud", None).await.unwrap();
        let signal = CancellationSignal::new();
        signal.cancel();
        let error = lanes.acquire("cloud", Some(&signal)).await.unwrap_err();
        assert_eq!(error.code, "ai_request_canceled");
    }

    #[tokio::test]
    async fn completed_slot_refills_before_slowest_sibling_finishes() {
        let events = Mutex::new(Vec::<(&'static str, usize)>::new());
        run_bounded_provider_queue(
            4,
            2,
            |index| {
                let events = &events;
                async move {
                    events.lock().unwrap().push(("start", index));
                    if index == 0 {
                        for _ in 0..64 {
                            tokio::task::yield_now().await;
                        }
                    }
                    events.lock().unwrap().push(("finish", index));
                    index
                }
            },
            |_| Ok::<QueueControl, ()>(QueueControl::Continue),
        )
        .await
        .unwrap();

        let events = events.into_inner().unwrap();
        let start_two = events
            .iter()
            .position(|event| *event == ("start", 2))
            .expect("third item should start");
        let finish_zero = events
            .iter()
            .position(|event| *event == ("finish", 0))
            .expect("slow item should finish");
        assert!(
            start_two < finish_zero,
            "a free slot must refill without waiting for the slow sibling"
        );
    }

    #[tokio::test]
    async fn completion_is_consumed_before_a_serial_slot_refills() {
        let consumed = Cell::new(0_usize);
        run_bounded_provider_queue(
            3,
            1,
            |index| {
                assert_eq!(consumed.get(), index);
                std::future::ready(index)
            },
            |index| {
                assert_eq!(consumed.get(), index);
                consumed.set(index + 1);
                Ok::<QueueControl, ()>(QueueControl::Continue)
            },
        )
        .await
        .unwrap();
        assert_eq!(consumed.get(), 3);
    }

    #[tokio::test]
    async fn stop_prevents_new_dispatch_but_drains_started_siblings() {
        let started = Mutex::new(Vec::new());
        run_bounded_provider_queue(
            4,
            2,
            |index| {
                let started = &started;
                async move {
                    started.lock().unwrap().push(index);
                    if index == 1 {
                        for _ in 0..16 {
                            tokio::task::yield_now().await;
                        }
                    }
                    index
                }
            },
            |index| {
                Ok::<QueueControl, ()>(if index == 0 {
                    QueueControl::Stop
                } else {
                    QueueControl::Continue
                })
            },
        )
        .await
        .unwrap();

        let mut started = started.into_inner().unwrap();
        started.sort_unstable();
        assert_eq!(started, vec![0, 1]);
    }
}
