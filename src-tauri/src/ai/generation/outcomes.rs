//! Native outcomes are separate from provider status and usage. A producer scope
//! captures physical request IDs directly; no latest-request/time heuristic links
//! concurrent windows. Diagnostic failures never invalidate paid or committed work.
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex},
};

use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{json, Value};

use super::{Context, RecoveryReason};
use crate::error::CommandError;

pub(crate) mod authoring;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub accepted_units: u32,
    pub rejected_proposals: u32,
    pub unaccounted_sources: u32,
    pub unresolved_regions: Option<u32>,
    pub protocol_complete: bool,
    pub rejection_rules: BTreeMap<String, u32>,
}

impl Snapshot {
    fn gain(&self, before: &Self) -> Value {
        let accepted = i64::from(self.accepted_units) - i64::from(before.accepted_units);
        let rejected = i64::from(self.rejected_proposals) - i64::from(before.rejected_proposals);
        let sources = i64::from(self.unaccounted_sources) - i64::from(before.unaccounted_sources);
        let regions = self
            .unresolved_regions
            .zip(before.unresolved_regions)
            .map(|(after, before)| i64::from(after) - i64::from(before));
        json!({
            "acceptedUnitsDelta": accepted,
            "rejectedProposalsDelta": rejected,
            "unaccountedSourcesDelta": sources,
            "unresolvedRegionsDelta": regions,
            "hasNativeProgress": accepted > 0 || rejected < 0 || sources < 0 || regions.is_some_and(|n| n < 0),
            "hasNativeRegression": accepted < 0 || rejected > 0 || sources > 0 || regions.is_some_and(|n| n > 0),
        })
    }
}

struct State {
    context: Context,
    stage: String,
    key: String,
    last_request: Option<String>,
    snapshot: Option<Snapshot>,
    before_request: Option<Snapshot>,
    pending_recovery: Option<Value>,
    linkage_incomplete: bool,
}

tokio::task_local! {
    static CURRENT: Option<Arc<Mutex<State>>>;
}

/// One producer, not its subscribers or an entire multi-window operation.
/// Absence of a physical request produces no fabricated request/usage record.
pub(crate) struct Task {
    state: Option<Arc<Mutex<State>>>,
    finished: bool,
}

impl Task {
    pub(crate) fn new(context: Option<&Context>, stage: &str, key: Option<&str>) -> Self {
        Self {
            state: context.map(|context| {
                Arc::new(Mutex::new(State {
                    context: context.clone(),
                    stage: stage.to_string(),
                    key: context.logical_request_key(stage, key),
                    last_request: None,
                    snapshot: None,
                    before_request: None,
                    pending_recovery: None,
                    linkage_incomplete: false,
                }))
            }),
            finished: false,
        }
    }

    pub(crate) fn scope<F: Future>(&self, future: F) -> impl Future<Output = F::Output> {
        // Box the child before placing it in another task-local state machine.
        CURRENT.scope(self.state.clone(), Box::pin(future))
    }

    pub(crate) fn finish<T>(&mut self, result: &Result<T, CommandError>) {
        let (status, code) = match result {
            Ok(_) => ("returned", None),
            Err(error) => ("failed", Some(safe_code(&error.code))),
        };
        self.close(status, code);
    }

    fn close(&mut self, status: &str, code: Option<&str>) {
        if self.finished {
            return;
        }
        self.finished = true;
        let Some(state) = self.state.as_ref().and_then(|state| state.lock().ok()) else {
            return;
        };
        let Some(id) = &state.last_request else {
            return;
        };
        write(
            &state.context,
            id,
            "$.nativeTask",
            json!({
            "schemaVersion": 1, "state": status, "errorCode": code,
            "lastCheckpoint": state.snapshot,
            }),
        );
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        self.close("interrupted", Some("native_task_interrupted"));
    }
}

fn current() -> Option<Arc<Mutex<State>>> {
    CURRENT.try_with(Clone::clone).ok().flatten()
}

fn same(state: &State, context: &Context, stage: &str, key: &str) -> bool {
    Arc::ptr_eq(&state.context.recovery, &context.recovery)
        && state.context.operation_id == context.operation_id
        && state.stage == stage
        && state.key == key
}

pub(super) fn started(context: &Context, stage: &str, key: &str, id: &str) {
    let Some(current) = current() else {
        return;
    };
    let Ok(mut state) = current.lock() else {
        return;
    };
    if !same(&state, context, stage, key) {
        return;
    }
    let previous = state.last_request.replace(id.to_string());
    state.before_request = state.snapshot.clone();
    let recovery = state.pending_recovery.take();
    write(
        context,
        id,
        "$.nativeLineage",
        json!({
        "schemaVersion": 1, "previousRequestId": previous, "recovery": recovery,
        "linkageIncomplete": state.linkage_incomplete,
        }),
    );
    write(
        context,
        id,
        "$.nativeTask",
        json!({"schemaVersion":1,"state":"running"}),
    );
    if let Some(previous) = previous {
        write(
            context,
            &previous,
            "$.nativeTask",
            json!({
            "schemaVersion":1,"state":"continued","nextRequestId":id,
            }),
        );
    }
}

pub(super) fn accounting_gap() {
    let Some(current) = current() else {
        return;
    };
    let Ok(mut state) = current.lock() else {
        return;
    };
    if let Some(id) = state.last_request.take() {
        write(
            &state.context,
            &id,
            "$.nativeTask",
            json!({
            "schemaVersion":1,"state":"linkage_lost","errorCode":"request_accounting_failed",
            }),
        );
    }
    state.pending_recovery = None;
    state.linkage_incomplete = true;
}

pub(super) fn recovery(
    context: &Context,
    stage: &str,
    key: &str,
    reason: RecoveryReason,
    code: &str,
    allowed: bool,
) {
    let Some(current) = current() else {
        return;
    };
    let Ok(mut state) = current.lock() else {
        return;
    };
    if !same(&state, context, stage, key) {
        return;
    }
    let value = json!({"schemaVersion":1,"reason":reason.as_str(),"triggerCode":safe_code(code),
        "allowed":allowed,"triggerRequestId":state.last_request});
    if let Some(id) = &state.last_request {
        // Eight bounded slots (four native reasons x allowed/denied), not an
        // unbounded event array. A denial never erases the earlier allowance.
        let disposition = if allowed { "allowed" } else { "denied" };
        let path = format!(
            "$.nativeRecoveryDecisions.{}_{}",
            reason.as_str(),
            disposition
        );
        write(context, id, &path, value.clone());
    }
    if allowed {
        state.pending_recovery = Some(value);
    }
}

pub(crate) fn baseline(snapshot: Snapshot) {
    let Some(current) = current() else {
        return;
    };
    let Ok(mut state) = current.lock() else {
        return;
    };
    if state.last_request.is_none() {
        state.snapshot = Some(snapshot);
    }
}

/// Called only after the owning checkpoint transaction commits, outside its DB
/// lock. The immutable pre-request snapshot prevents a repeated final save from
/// erasing the repair's measured gains.
pub(crate) fn checkpoint(snapshot: Snapshot) {
    let Some(current) = current() else {
        return;
    };
    let Ok(mut state) = current.lock() else {
        return;
    };
    if let Some(id) = &state.last_request {
        write(
            &state.context,
            id,
            "$.nativeCheckpoint",
            json!({
            "schemaVersion":1, "before":state.before_request, "after":snapshot,
            "gain":state.before_request.as_ref().map(|before| snapshot.gain(before)),
            }),
        );
    }
    state.snapshot = Some(snapshot);
}

fn safe_code(code: &str) -> &str {
    if !code.is_empty()
        && code.len() <= 80
        && code
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    {
        code
    } else {
        "unclassified_native_error"
    }
}

fn write(context: &Context, id: &str, path: &str, value: Value) {
    super::warn_accounting(context.database.with_connection(|connection| {
        connection.execute(
            "UPDATE generation_requests SET payload_bytes_json=json_set(payload_bytes_json,?1,json(?2))
             WHERE id=?3 AND operation_id=?4",
            params![path, value.to_string(), id, context.operation_id],
        ).map_err(CommandError::database)?;
        Ok(())
    }));
}

pub(super) fn reconcile_in(connection: &Connection, now: i64) -> Result<(), CommandError> {
    connection
        .execute(
            "UPDATE generation_requests SET payload_bytes_json=json_set(payload_bytes_json,
           '$.nativeTask.state','interrupted','$.nativeTask.errorCode','native_process_interrupted',
           '$.nativeTask.reconciledAt',?1)
         WHERE json_extract(payload_bytes_json,'$.nativeTask.state')='running'",
            [now],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

#[cfg(test)]
mod tests;
