//! Bounded non-spawned concurrency retains the caller's task-local cancellation
//! and telemetry. Every started sibling finishes/checkpoints before an error is
//! returned; dropping unrelated successful work is not a retry strategy.
use std::future::Future;

pub(crate) async fn four<F, Fut, T>(len: usize, run: F) -> Vec<T>
where
    F: Fn(usize) -> Fut,
    Fut: Future<Output = T>,
{
    let run = &run;
    let slot = |index| async move {
        if index < len {
            Some(run(index).await)
        } else {
            None
        }
    };
    let (a, b, c, d) = tokio::join!(slot(0), slot(1), slot(2), slot(3));
    [a, b, c, d].into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn independent_requests_overlap_and_results_keep_source_order() {
        let barrier = tokio::sync::Barrier::new(4);
        let active = AtomicUsize::new(0);
        let barrier = &barrier;
        let active = &active;
        let result = four(4, |index| async move {
            active.fetch_add(1, Ordering::SeqCst);
            barrier.wait().await;
            assert_eq!(active.load(Ordering::SeqCst), 4);
            index
        })
        .await;
        assert_eq!(result, vec![0, 1, 2, 3]);
    }
    #[tokio::test]
    async fn serial_topology_runs_only_one_slot() {
        let calls = AtomicUsize::new(0);
        let result = four(1, |index| {
            calls.fetch_add(1, Ordering::SeqCst);
            std::future::ready(index)
        })
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(result, vec![0]);
    }
}
