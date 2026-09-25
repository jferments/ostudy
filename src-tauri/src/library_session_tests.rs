use super::*;
use std::{sync::mpsc, thread};

#[test]
fn pending_old_future_cannot_enter_after_replacement() {
    let session = Arc::new(Session::default());
    let dispatch = session.enter(Some(0)).unwrap();
    drop(dispatch);
    let mut replace = session.replace(Some(0)).unwrap();
    replace.switching();
    assert_eq!(replace.finish(true).unwrap(), 1);
    assert_eq!(session.enter(Some(0)).err().unwrap().code, "library_generation_changed");
    assert!(session.enter(None).is_err());
    assert!(session.enter(Some(1)).is_ok());
}

#[test]
fn existing_async_work_drains_before_switch_and_new_work_is_refused() {
    let session = Arc::new(Session::default());
    let existing = session.enter(Some(0)).unwrap();
    let worker = Arc::clone(&session);
    let (started_tx, started_rx) = mpsc::channel();
    let (drained_tx, drained_rx) = mpsc::channel();
    let join = thread::spawn(move || {
        started_tx.send(()).unwrap();
        let replacement = worker.replace(Some(0)).unwrap();
        drained_tx.send(()).unwrap();
        replacement.finish(true).unwrap();
    });
    started_rx.recv().unwrap();
    // Inspect the admission latch, not a timing assumption or arbitrary sleep.
    loop {
        if session.status().unwrap().restoring {
            break;
        }
        thread::yield_now();
    }
    assert!(drained_rx.try_recv().is_err());
    assert_eq!(session.enter(Some(0)).err().unwrap().code, "library_restore_in_progress");
    drop(existing);
    drained_rx.recv().unwrap();
    join.join().unwrap();
    assert_eq!(session.status().unwrap().generation, 1);
}

#[test]
fn cancellation_before_switch_preserves_current_generation() {
    let session = Arc::new(Session::default());
    drop(session.replace(Some(0)).unwrap());
    assert!(session.enter(Some(0)).is_ok());
    assert_eq!(session.status().unwrap().generation, 0);
}

#[test]
fn panic_or_failure_after_connection_close_keeps_data_commands_closed() {
    let session = Arc::new(Session::default());
    let mut replacement = session.replace(Some(0)).unwrap();
    replacement.switching();
    drop(replacement);
    assert!(session.status().unwrap().recovery_required);
    assert_eq!(session.enter(Some(0)).err().unwrap().code, "library_recovery_required");
    assert!(session.replace(Some(0)).is_err());
}

#[test]
fn verified_rollback_can_reopen_without_replaying_old_work() {
    let session = Arc::new(Session::default());
    let mut replacement = session.replace(Some(0)).unwrap();
    replacement.switching();
    replacement.finish(true).unwrap();
    assert!(session.enter(Some(1)).is_ok());
    assert!(session.enter(Some(0)).is_err());
}
