//! Operation-owner results are independent of request/authoring observations.
//! Store a bounded native code, never an error message, source or paid response.
use rusqlite::{params, Connection};
use serde_json::json;

use super::{safe_code, Context};
use crate::{ai::generation::Operation, database::Database, error::CommandError};

impl Operation {
    /// Observe the actual owner's Result. Recording it must not change that
    /// Result, publish an artifact, or fabricate a physical provider request.
    pub(crate) fn finish_result<T>(&mut self, result: &Result<T, CommandError>) {
        if self.finished {
            return;
        }
        self.finished = true;
        let (state, error_code) = match result {
            Ok(_) => ("complete", None),
            Err(error) => ("failed", Some(safe_code(&error.code))),
        };
        let outcome = json!({
            "schemaVersion": 1,
            "state": state,
            "errorCode": error_code,
            "resultObserved": true,
            "artifactPublication": "not_observed",
        });
        super::super::warn_accounting(close(&self.context, state, &outcome.to_string()));
        crate::logging::info_target(
            "ai.generation",
            "generation_operation_finished",
            "The generation operation owner returned; diagnostic persistence is best effort.",
            json!({"operation":self.context.operation_id,"state":state,"errorCode":error_code}),
        );
    }
}

fn close(context: &Context, state: &str, outcome: &str) -> Result<(), CommandError> {
    context.database.with_connection(|connection| {
        let now = crate::util::unix_time_millis()?;
        // State, timestamp and cause advance together. A second terminal write
        // or a stale owner after startup reconciliation cannot rewrite history.
        connection
            .execute(
                "UPDATE generation_operations SET state=?1,finished_at=?2,outcome_json=?3
                 WHERE id=?4 AND state='running'",
                params![state, now, outcome, context.operation_id],
            )
            .map_err(CommandError::database)?;
        Ok(())
    })
}

/// Establish the existing task-owned identity before source/provider preparation.
/// An unavailable diagnostic write must not replace the product's own error.
pub(crate) fn bind_document(database: &Database) {
    super::super::warn_accounting(
        crate::task_cancellation::generation_context(database).map(|_| ()),
    );
}

pub(super) fn reconcile_in(connection: &Connection, now: i64) -> Result<(), CommandError> {
    let outcome = json!({
        "schemaVersion": 1,
        "state": "interrupted",
        "errorCode": "native_process_interrupted",
        "resultObserved": false,
        "artifactPublication": "not_observed",
    });
    connection
        .execute(
            "UPDATE generation_operations SET state='interrupted',finished_at=?1,outcome_json=?2
             WHERE state='running'",
            params![now, outcome.to_string()],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

#[cfg(test)]
mod tests;
