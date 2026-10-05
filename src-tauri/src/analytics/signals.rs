//! One learning-signal pass per analytics request.
//!
//! The learning signal ranks every review in the library with a window
//! function. Current inventory, the deck table and the tag table all need it,
//! and each statement used to recompute it. It is now materialized once into a
//! connection-local TEMP table that lives only as long as the request.

use rusqlite::Connection;
use serde_json::json;

use crate::{error::CommandError, projections::learning_signals as signal_projection};

/// Joins the request table under the alias `value_sql()` reads.
pub(super) const JOIN_SQL: &str =
    "LEFT JOIN temp.analytics_learning_signal ls ON ls.card_id = c.id";

/// Owns the request's signal table. Loaders that join the table take this
/// guard, so they cannot run without it; dropping the guard drops the table on
/// every exit path, including errors and panics.
pub(super) struct RequestSignals<'connection> {
    connection: &'connection Connection,
}

impl<'connection> RequestSignals<'connection> {
    pub(super) fn materialize(connection: &'connection Connection) -> Result<Self, CommandError> {
        // Build the guard first so a failure part-way through still drops the
        // table. The leading DROP clears anything a lost guard left behind.
        let signals = Self { connection };
        connection
            .execute_batch(&format!(
                "DROP TABLE IF EXISTS temp.analytics_learning_signal;
                 CREATE TEMP TABLE analytics_learning_signal (
                     card_id TEXT PRIMARY KEY NOT NULL,
                     needs_more_practice INTEGER NOT NULL
                 ) WITHOUT ROWID;
                 INSERT INTO temp.analytics_learning_signal (card_id, needs_more_practice)
                 WITH {ctes}
                 SELECT card_id, needs_more_practice FROM learning_signal;",
                ctes = signal_projection::LEARNING_SIGNAL_CTES,
            ))
            .map_err(CommandError::database)?;
        Ok(signals)
    }

    pub(super) const fn join_sql(&self) -> &'static str {
        JOIN_SQL
    }
}

impl Drop for RequestSignals<'_> {
    fn drop(&mut self) {
        // The table is connection-local scratch, not durable state, and the
        // next request drops any leftover before use. Record a failure only.
        if let Err(error) = self
            .connection
            .execute_batch("DROP TABLE IF EXISTS temp.analytics_learning_signal")
        {
            crate::logging::warn_target(
                "analytics.signals",
                "analytics_signal_table_drop_failed",
                "The per-request learning signal table could not be dropped.",
                json!({ "sqliteCode": error.sqlite_error_code().map(|code| format!("{code:?}")) }),
            );
        }
    }
}
