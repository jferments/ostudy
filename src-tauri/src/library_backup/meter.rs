//! Advisory progress through a long library copy or check, counted in bytes.
//!
//! A library update copies and hashes the managed files several times. The
//! update plans its total once it knows the library's size, and the file
//! routines count what they read or write. The count only drives a progress
//! bar: it never decides whether an update succeeds, and nothing is counted
//! while no update is planned. The state is per thread because an update
//! runs on one thread from start to finish.

use std::cell::RefCell;

/// The bar never claims to be finished before the work says it is.
const MAX_BEFORE_FINISH: u16 = 990;
/// Report at most once per half percent.
const STEP: u16 = 5;

type Report = Box<dyn FnMut(u16)>;

#[derive(Default)]
struct State {
    report: Option<Report>,
    total: u64,
    done: u64,
    shown: u16,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

/// Sends progress in thousandths to `report` until the returned guard drops.
pub(crate) fn attach(report: impl FnMut(u16) + 'static) -> Attached {
    STATE.with(|state| {
        *state.borrow_mut() = State {
            report: Some(Box::new(report)),
            ..State::default()
        }
    });
    Attached(())
}

/// Stops reporting when dropped, however the update ends.
#[must_use]
pub(crate) struct Attached(());

impl Drop for Attached {
    fn drop(&mut self) {
        STATE.with(|state| *state.borrow_mut() = State::default());
    }
}

/// Starts counting toward `total` units.
pub(crate) fn plan(total: u64) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.total = total.max(1);
        state.done = 0;
        state.shown = 0;
    });
}

/// Counts `units` of planned work as done.
pub(crate) fn advance(units: u64) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.total == 0 {
            return;
        }
        state.done = state.done.saturating_add(units);
        let permille = u16::try_from(state.done.saturating_mul(1000) / state.total)
            .unwrap_or(u16::MAX)
            .min(MAX_BEFORE_FINISH);
        if permille >= state.shown.saturating_add(STEP) {
            state.shown = permille;
            if let Some(report) = state.report.as_mut() {
                report(permille);
            }
        }
    });
}

/// Ends the planned work, reporting it complete.
pub(crate) fn finish() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.total == 0 {
            return;
        }
        state.total = 0;
        if let Some(report) = state.report.as_mut() {
            report(1000);
        }
    });
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[test]
    fn progress_rises_in_steps_stays_short_of_done_and_finishes_once() {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let record = Rc::clone(&seen);
        {
            let _attached = attach(move |permille| record.borrow_mut().push(permille));
            // Nothing counts before a plan.
            advance(500);
            plan(1000);
            advance(2);
            advance(4);
            advance(500);
            // More work than planned stays short of done.
            advance(5000);
            advance(10);
            finish();
            // Nothing counts after the work ends.
            advance(10);
            finish();
        }
        assert_eq!(*seen.borrow(), vec![6, 506, MAX_BEFORE_FINISH, 1000]);
    }

    #[test]
    fn counting_outside_an_observed_update_does_nothing() {
        plan(10);
        advance(10);
        finish();
        let _attached = attach(|_| panic!("an earlier plan must not leak into this update"));
        advance(10);
        finish();
    }
}
