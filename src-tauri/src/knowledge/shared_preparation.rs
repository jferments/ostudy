//! Join exact, authorized interpretation work; durable results stay in analysis_store.
//! The driving future borrows the existing native service. Canceling its subscriber
//! detaches that interest, not other subscribers. Dropping the driving future wakes
//! waiters with an interrupted result; it never silently retries a provider call.

use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, LazyLock, Mutex},
};

use tokio::sync::watch;

use crate::{
    database::Database, error::CommandError, task_activity::ActivityGuard,
    task_cancellation::CancellationSignal,
};

static WORK: LazyLock<Registry> = LazyLock::new(Registry::default);
const MAX_ACTIVE_WINDOWS: usize = 16;
const MAX_SUBSCRIBERS: usize = 64;
type Key = (usize, String);
type Outcome = Result<(), (String, String)>;

#[derive(Default)]
struct Registry {
    jobs: Mutex<HashMap<Key, Arc<Job>>>,
}

struct Job {
    signal: CancellationSignal,
    subscribers: Mutex<usize>,
    result: watch::Sender<Option<Outcome>>,
}

struct Subscription {
    job: Arc<Job>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        match self.job.subscribers.lock() {
            Ok(mut count) => {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.job.signal.cancel();
                }
            }
            Err(_) => self.job.signal.cancel(),
        }
    }
}

struct Producer<'a> {
    registry: &'a Registry,
    key: Key,
    job: Arc<Job>,
    _activity: ActivityGuard,
    finished: bool,
}

impl Producer<'_> {
    fn publish(&mut self, outcome: Outcome) {
        let _ = self.job.result.send_replace(Some(outcome));
        if let Ok(mut jobs) = self.registry.jobs.lock() {
            if jobs
                .get(&self.key)
                .is_some_and(|job| Arc::ptr_eq(job, &self.job))
            {
                jobs.remove(&self.key);
            }
        }
        self.finished = true;
    }
}

impl Drop for Producer<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.job.signal.cancel();
            self.publish(Err((
                "shared_preparation_interrupted".to_string(),
                "Shared preparation stopped. Saved results remain available; retry explicitly."
                    .to_string(),
            )));
        }
    }
}

/// `identity` is a native fingerprint of the exact evidence and captured provider
/// authorization, including repair permission. Never accept an untrusted IPC key.
pub(crate) async fn join_or_run<F, Fut>(
    database: &Database,
    identity: &str,
    subscriber: &CancellationSignal,
    operation: F,
) -> Result<(), CommandError>
where
    F: FnOnce(CancellationSignal) -> Fut,
    Fut: Future<Output = Result<(), CommandError>>,
{
    if !super::contracts::is_hash(identity) {
        return Err(unavailable());
    }
    // This process-local namespace cannot grant access between separate libraries.
    // Both callers retain a borrow of the Database until their subscription ends.
    let key = (std::ptr::from_ref(database).addr(), identity.to_string());
    WORK.run(key, subscriber, operation).await
}

impl Registry {
    fn subscribe(&self, key: Key) -> Result<(Subscription, Option<Producer<'_>>), CommandError> {
        let mut jobs = self.jobs.lock().map_err(|_| unavailable())?;
        if let Some(job) = jobs.get(&key) {
            let mut count = job.subscribers.lock().map_err(|_| unavailable())?;
            if job.signal.is_canceled() || *count == 0 {
                return Err(CommandError::new(
                    "shared_preparation_stopping",
                    "The previous preparation is stopping. Its saved results are retained.",
                ));
            }
            if *count >= MAX_SUBSCRIBERS {
                return Err(capacity());
            }
            *count += 1;
            return Ok((Subscription { job: job.clone() }, None));
        }
        if jobs.len() >= MAX_ACTIVE_WINDOWS {
            return Err(capacity());
        }
        let activity = ActivityGuard::begin()?;
        let (result, _) = watch::channel(None);
        let job = Arc::new(Job {
            signal: CancellationSignal::new(),
            subscribers: Mutex::new(1),
            result,
        });
        jobs.insert(key.clone(), job.clone());
        Ok((
            Subscription { job: job.clone() },
            Some(Producer {
                registry: self,
                key,
                job,
                _activity: activity,
                finished: false,
            }),
        ))
    }

    async fn run<F, Fut>(
        &self,
        key: Key,
        subscriber: &CancellationSignal,
        operation: F,
    ) -> Result<(), CommandError>
    where
        F: FnOnce(CancellationSignal) -> Fut,
        Fut: Future<Output = Result<(), CommandError>>,
    {
        super::runtime::check_canceled(subscriber)?;
        let (subscription, producer) = self.subscribe(key)?;
        let job = subscription.job.clone();
        let Some(mut producer) = producer else {
            let mut result = job.result.subscribe();
            loop {
                super::runtime::check_canceled(subscriber)?;
                let completed = result.borrow().clone();
                if let Some(outcome) = completed {
                    return outcome.map_err(|(code, message)| CommandError::new(code, message));
                }
                tokio::select! {
                    biased;
                    _ = subscriber.canceled() => return Err(canceled()),
                    changed = result.changed() => {
                        if changed.is_err() {
                            return Err(unavailable());
                        }
                    }
                }
            }
        };
        let work = operation(job.signal.clone());
        tokio::pin!(work);
        let mut interest = Some(subscription);
        let outcome = loop {
            tokio::select! {
                biased;
                _ = subscriber.canceled(), if interest.is_some() => {
                    // Continue driving the borrowed producer for remaining callers.
                    drop(interest.take());
                }
                _ = job.signal.canceled() => break Err(canceled()),
                outcome = &mut work => break outcome,
            }
        };
        producer.publish(
            outcome
                .as_ref()
                .map(|_| ())
                .map_err(|error| (error.code.clone(), error.message.clone())),
        );
        super::runtime::check_canceled(subscriber)?;
        outcome
    }
}

fn canceled() -> CommandError {
    CommandError::new(
        "ai_request_canceled",
        "This preparation subscription was canceled.",
    )
}

fn unavailable() -> CommandError {
    CommandError::new(
        "learning_task_registry_unavailable",
        "Shared preparation is unavailable.",
    )
}

fn capacity() -> CommandError {
    CommandError::new(
        "learning_task_capacity",
        "Shared preparation is at its bounded capacity.",
    )
}

#[cfg(test)]
mod tests;
