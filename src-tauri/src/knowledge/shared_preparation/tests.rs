use std::{cell::Cell, pin::Pin, task::Poll};

use super::*;

async fn pending<F: Future>(mut future: Pin<&mut F>) {
    std::future::poll_fn(|context| {
        assert!(future.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
}

#[tokio::test]
async fn matching_subscribers_share_one_producer_and_cancellation_is_local() {
    let registry = Registry::default();
    let leader_signal = CancellationSignal::new();
    let follower_signal = CancellationSignal::new();
    let calls = Cell::new(0);
    let release = tokio::sync::Notify::new();
    let key = (1, "same authorized window".to_string());
    let mut leader = Box::pin(registry.run(key.clone(), &leader_signal, |signal| {
        let calls = &calls;
        let release = &release;
        async move {
            calls.set(calls.get() + 1);
            release.notified().await;
            assert!(!signal.is_canceled());
            Ok(())
        }
    }));
    pending(leader.as_mut()).await;
    let mut follower = Box::pin(registry.run(key.clone(), &follower_signal, |_| async {
        panic!("a subscriber must not start another producer")
    }));
    pending(follower.as_mut()).await;
    assert_eq!(*registry.jobs.lock().unwrap()[&key].subscribers.lock().unwrap(), 2);
    leader_signal.cancel();
    pending(leader.as_mut()).await;
    assert_eq!(*registry.jobs.lock().unwrap()[&key].subscribers.lock().unwrap(), 1);
    release.notify_one();
    let (leader_result, follower_result) = tokio::join!(leader, follower);
    assert_eq!(leader_result.unwrap_err().code, "ai_request_canceled");
    follower_result.unwrap();
    assert_eq!(calls.get(), 1);
    assert!(registry.jobs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn canceling_last_subscriber_stops_work_and_releases_registry() {
    let registry = Registry::default();
    let signal = CancellationSignal::new();
    let work_signal = std::cell::RefCell::new(None);
    let mut task = Box::pin(registry.run((1, "last".into()), &signal, |work| {
        let work_signal = &work_signal;
        async move {
            *work_signal.borrow_mut() = Some(work);
            std::future::pending::<Result<(), CommandError>>().await
        }
    }));
    pending(task.as_mut()).await;
    signal.cancel();
    assert_eq!(task.await.unwrap_err().code, "ai_request_canceled");
    assert!(work_signal.borrow().as_ref().unwrap().is_canceled());
    assert!(registry.jobs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn canceled_follower_cannot_cancel_the_producer() {
    let registry = Registry::default();
    let leader_signal = CancellationSignal::new();
    let follower_signal = CancellationSignal::new();
    let release = tokio::sync::Notify::new();
    let mut leader = Box::pin(registry.run((1, "work".into()), &leader_signal, |work| {
        let release = &release;
        async move {
            release.notified().await;
            assert!(!work.is_canceled());
            Ok(())
        }
    }));
    pending(leader.as_mut()).await;
    let mut follower = Box::pin(registry.run((1, "work".into()), &follower_signal, |_| async {
        panic!("unexpected second producer")
    }));
    pending(follower.as_mut()).await;
    follower_signal.cancel();
    assert_eq!(follower.await.unwrap_err().code, "ai_request_canceled");
    release.notify_one();
    leader.await.unwrap();
}

#[tokio::test]
async fn interrupted_producer_wakes_waiters_without_automatic_retry() {
    let registry = Registry::default();
    let signal = CancellationSignal::new();
    let mut leader = Box::pin(registry.run((1, "drop".into()), &signal, |_| async {
        std::future::pending::<Result<(), CommandError>>().await
    }));
    pending(leader.as_mut()).await;
    let mut follower = Box::pin(registry.run((1, "drop".into()), &signal, |_| async {
        panic!("interruption must not automatically retry provider work")
    }));
    pending(follower.as_mut()).await;
    drop(leader);
    assert_eq!(follower.await.unwrap_err().code, "shared_preparation_interrupted");
    assert!(registry.jobs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn provider_failure_is_shared_but_not_cached_as_a_success() {
    let registry = Registry::default();
    let signal = CancellationSignal::new();
    let release = tokio::sync::Notify::new();
    let mut first = Box::pin(registry.run((1, "failure".into()), &signal, |_| async {
        release.notified().await;
        Err(CommandError::new("provider_failed", "Explicit failure"))
    }));
    pending(first.as_mut()).await;
    let mut second = Box::pin(registry.run((1, "failure".into()), &signal, |_| async {
        panic!("unexpected producer")
    }));
    pending(second.as_mut()).await;
    release.notify_one();
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first.unwrap_err().code, "provider_failed");
    assert_eq!(second.unwrap_err().code, "provider_failed");
    registry.run((1, "failure".into()), &signal, |_| async { Ok(()) }).await.unwrap();
}

#[test]
fn library_authorization_and_capacity_are_separate_admission_boundaries() {
    let registry = Registry::default();
    let (first, producer) = registry.subscribe((1, "window".into())).unwrap();
    let (other_library, other_producer) = registry.subscribe((2, "window".into())).unwrap();
    let (other_scope, scoped_producer) = registry.subscribe((1, "different scope".into())).unwrap();
    assert!(producer.is_some() && other_producer.is_some() && scoped_producer.is_some());
    let mut followers = Vec::new();
    for _ in 1..MAX_SUBSCRIBERS {
        let (subscriber, producer) = registry.subscribe((1, "window".into())).unwrap();
        assert!(producer.is_none());
        followers.push(subscriber);
    }
    assert!(registry.subscribe((1, "window".into())).is_err());
    drop(followers);
    drop(first);
    assert!(registry.subscribe((1, "window".into())).is_err());
    drop(producer);
    drop(other_library);
    drop(other_producer);
    drop(other_scope);
    drop(scoped_producer);
    assert!(registry.jobs.lock().unwrap().is_empty());
}
