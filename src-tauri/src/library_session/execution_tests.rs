use super::*;
use crate::{database::Database, library_session::Session, task_activity::TestGate};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver},
        Mutex,
    },
    time::Duration,
};

const DEADLINE: Duration = Duration::from_secs(10);

fn receive<T>(receiver: &Receiver<T>) -> T {
    receiver.recv_timeout(DEADLINE).expect("worker handshake")
}

#[test]
fn bounded_fifo_dispatch_never_runs_work_on_the_submitting_thread() {
    let executor = Executor::new("test-command-order", 2).unwrap();
    let caller = thread::current().id();
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let (done_tx, done_rx) = channel();
    let order = Arc::new(Mutex::new(Vec::new()));
    executor
        .submit(
            move || {
                assert_ne!(thread::current().id(), caller);
                started_tx.send(()).unwrap();
                receive(&release_rx);
            },
            || panic!("unexpected worker panic"),
        )
        .unwrap();
    receive(&started_rx);
    for value in [1, 2] {
        let order = Arc::clone(&order);
        let done = done_tx.clone();
        executor
            .submit(
                move || {
                    order.lock().unwrap().push(value);
                    done.send(()).unwrap();
                },
                || panic!("unexpected worker panic"),
            )
            .unwrap();
    }
    assert_eq!(
        executor
            .submit(|| panic!("must not run"), || {})
            .unwrap_err()
            .code,
        "native_command_queue_full"
    );
    release_tx.send(()).unwrap();
    receive(&done_rx);
    receive(&done_rx);
    assert_eq!(*order.lock().unwrap(), vec![1, 2]);
}

#[test]
fn blocked_database_work_does_not_block_control_dispatch() {
    let database = Database::open_in_memory().unwrap();
    let blocker_database = database.clone();
    let (locked_tx, locked_rx) = channel();
    let (unlock_tx, unlock_rx) = channel();
    let blocker = thread::spawn(move || {
        blocker_database
            .with_connection(|_| {
                locked_tx.send(()).unwrap();
                receive(&unlock_rx);
                Ok(())
            })
            .unwrap();
    });
    receive(&locked_rx);
    let ordinary = Executor::new("test-blocked-command", 1).unwrap();
    let controls = Executor::new("test-responsive-control", 1).unwrap();
    let (started_tx, started_rx) = channel();
    let (done_tx, done_rx) = channel();
    ordinary
        .submit(
            move || {
                started_tx.send(()).unwrap();
                let value = database
                    .with_connection(|connection| {
                        connection
                            .query_row("SELECT 42", [], |row| row.get::<_, i64>(0))
                            .map_err(CommandError::database)
                    })
                    .unwrap();
                done_tx.send(value).unwrap();
            },
            || panic!("unexpected worker panic"),
        )
        .unwrap();
    receive(&started_rx);
    let (control_tx, control_rx) = channel();
    controls
        .submit(move || control_tx.send(()).unwrap(), || {})
        .unwrap();
    receive(&control_rx);
    assert!(done_rx.try_recv().is_err());
    unlock_tx.send(()).unwrap();
    assert_eq!(receive(&done_rx), 42);
    blocker.join().unwrap();
}

#[test]
fn lost_reply_and_dropped_producer_keep_worker_leases_until_commit_and_restore_drain() {
    let database = Database::open_in_memory().unwrap();
    let worker_database = database.clone();
    let session = Arc::new(Session::default());
    let activity_gate = TestGate::default();
    let lease = session.enter(Some(0)).unwrap();
    let activity = activity_gate.activity().unwrap();
    let executor = Executor::new("test-lost-command-reply", 1).unwrap();
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let (reply_tx, reply_rx) = channel();
    executor
        .submit(
            move || {
                let _lease = lease;
                let _activity = activity;
                started_tx.send(()).unwrap();
                receive(&release_rx);
                worker_database
                    .with_connection(|connection| {
                        let transaction = connection.transaction().map_err(CommandError::database)?;
                        transaction
                            .execute_batch("CREATE TEMP TABLE execution_probe(value INTEGER); INSERT INTO execution_probe VALUES(7);")
                            .map_err(CommandError::database)?;
                        transaction.commit().map_err(CommandError::database)
                    })
                    .unwrap();
                // A vanished recipient does not cancel or repeat the commit.
                let _ = reply_tx.send(());
            },
            || panic!("unexpected worker panic"),
        )
        .unwrap();
    receive(&started_rx);
    drop(reply_rx);
    drop(executor);
    assert!(activity_gate.has_work());
    assert!(activity_gate.maintenance().is_err());
    let replacing = Arc::clone(&session);
    let (drained_tx, drained_rx) = channel();
    let restore = thread::spawn(move || {
        let replacement = replacing.replace(Some(0)).unwrap();
        drained_tx.send(()).unwrap();
        replacement.finish(true).unwrap();
    });
    let deadline = Instant::now() + DEADLINE;
    while !session.status().unwrap().restoring {
        assert!(Instant::now() < deadline, "restore admission did not close");
        thread::yield_now();
    }
    assert!(drained_rx.try_recv().is_err());
    assert_eq!(
        session.enter(Some(0)).err().unwrap().code,
        "library_restore_in_progress"
    );
    release_tx.send(()).unwrap();
    receive(&drained_rx);
    restore.join().unwrap();
    assert!(!activity_gate.has_work());
    assert!(activity_gate.maintenance().is_ok());
    assert_eq!(session.status().unwrap().generation, 1);
    assert_eq!(
        session.enter(Some(0)).err().unwrap().code,
        "library_generation_changed"
    );
    database
        .with_connection(|connection| {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM execution_probe WHERE value=7",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn queue_rejection_drops_the_original_lease_without_running_the_request() {
    let executor = Executor::new("test-rejected-command", 1).unwrap();
    let session = Arc::new(Session::default());
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let (done_tx, done_rx) = channel();
    executor
        .submit(
            move || {
                started_tx.send(()).unwrap();
                receive(&release_rx);
            },
            || {},
        )
        .unwrap();
    receive(&started_rx);
    executor
        .submit(move || done_tx.send(()).unwrap(), || {})
        .unwrap();
    let lease = session.enter(Some(0)).unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    let rejected_ran = Arc::clone(&ran);
    let result = executor.submit(
        move || {
            let _lease = lease;
            rejected_ran.store(true, Ordering::Release);
        },
        || {},
    );
    assert_eq!(result.unwrap_err().code, "native_command_queue_full");
    assert_eq!(session.state.lock().unwrap().readers, 0);
    release_tx.send(()).unwrap();
    receive(&done_rx);
    assert!(!ran.load(Ordering::Acquire));
}

#[test]
fn worker_panic_releases_lease_but_does_not_clear_database_poison() {
    let database = Database::open_in_memory().unwrap();
    let worker_database = database.clone();
    let session = Arc::new(Session::default());
    let lease = session.enter(Some(0)).unwrap();
    let executor = Executor::new("test-panicking-command", 1).unwrap();
    let (panicked_tx, panicked_rx) = channel();
    executor
        .submit(
            move || {
                let _lease = lease;
                let _: Result<(), CommandError> = worker_database.with_connection(|_| {
                    panic!("injected worker failure inside connection owner");
                });
            },
            move || panicked_tx.send(()).unwrap(),
        )
        .unwrap();
    receive(&panicked_rx);
    assert_eq!(session.state.lock().unwrap().readers, 0);
    assert_eq!(
        database.with_connection(|_| Ok(())).unwrap_err().code,
        "database_lock_poisoned"
    );
    let (done_tx, done_rx) = channel();
    executor
        .submit(move || done_tx.send(()).unwrap(), || {})
        .unwrap();
    receive(&done_rx);
}

#[test]
fn only_explicit_control_commands_use_the_reserved_lane() {
    for command in [
        "get_library_session",
        "set_platform_back_interception",
        "cancel_document_flashcard_generation",
        "cancel_document_study_guide_generation",
        "cancel_learning_task",
        "cancel_study_target_bundle",
        "cancel_device_sync_session",
    ] {
        assert!(is_control(command));
    }
    for command in [
        "create_deck",
        "get_analytics",
        "delete_document",
        "cancel_unregistered",
    ] {
        assert!(!is_control(command));
    }
}
