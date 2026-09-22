//! Bounded concurrency policy for independent document-provider windows.
//!
//! Concurrency follows the execution topology, not a list of provider brands.
//! Cloud requests can overlap because every request owns a disjoint bounded
//! source window and durable commits remain source-ordered. Local and LAN model
//! servers remain serial so ostudy does not assume that one user-managed
//! inference process was started with multiple decoding slots.

use std::{
    future::Future,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(crate) const CLOUD_DOCUMENT_PARALLELISM: usize = 4;

pub(crate) fn document_request_parallelism(network_scope: &str) -> usize {
    match network_scope {
        "cloud" => CLOUD_DOCUMENT_PARALLELISM,
        _ => 1,
    }
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
