//! Durable, content-free accounting for each physical generation request.
//! Provider usage is authoritative. Payload sizes are separate diagnostics.
use std::{fmt, future::Future};

use rusqlite::{params, TransactionBehavior};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

mod payload;
#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(crate) struct Context {
    database: Database,
    pub(crate) operation_id: String,
}

#[derive(Clone)]
struct RequestScope {
    context: Context,
    stage: String,
    logical_key: Option<String>,
    provider: String,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationContext")
            .field("operation_id", &self.operation_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(crate) struct Operation {
    context: Context,
    finished: bool,
}

impl Operation {
    pub(crate) fn begin(
        database: &Database,
        operation_id: Option<&str>,
        document_id: Option<&str>,
        collection_id: Option<&str>,
        purpose: &str,
    ) -> Result<Self, CommandError> {
        if let Some(id) = operation_id {
            validate_entity_id(id, "generation operation")?;
        }
        if let Some(id) = document_id.or(collection_id) {
            validate_entity_id(id, "generation target")?;
        }
        let operation_id = database.with_connection(|connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(CommandError::database)?;
            let running: i64 = tx.query_row(
                "SELECT COUNT(*) FROM generation_operations WHERE state='running'
                 AND ((?1 IS NOT NULL AND document_id=?1)
                   OR (?2 IS NOT NULL AND collection_id=?2))",
                params![document_id, collection_id],
                |row| row.get(0),
            ).map_err(CommandError::database)?;
            if running != 0 {
                return Err(CommandError::new(
                    "generation_already_running",
                    "Generation is already running for this source. No duplicate provider work was started.",
                ));
            }
            let id = match operation_id {
                Some(id) => id.to_string(),
                None => generate_id(&tx)?,
            };
            tx.execute(
                "INSERT INTO generation_operations
                 (id,purpose,document_id,collection_id,started_at,state)
                 VALUES(?1,?2,?3,?4,?5,'running')",
                params![id, purpose, document_id, collection_id, unix_time_millis()?],
            ).map_err(CommandError::database)?;
            tx.commit().map_err(CommandError::database)?;
            Ok(id)
        })?;
        crate::logging::info_target(
            "ai.generation",
            "generation_operation_started",
            "Started durable generation accounting; source and prompt content are not recorded.",
            json!({"operationId": operation_id, "purpose": purpose}),
        );
        Ok(Self {
            context: Context {
                database: database.clone(),
                operation_id,
            },
            finished: false,
        })
    }

    pub(crate) fn context(&self) -> Context {
        self.context.clone()
    }

    // Final accounting must never turn a committed artifact into a failed write.
    pub(crate) fn finish(&mut self, succeeded: bool) {
        self.close(if succeeded { "complete" } else { "failed" });
    }

    fn close(&mut self, state: &str) {
        if self.finished {
            return;
        }
        self.finished = true;
        let result = self.context.database.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE generation_operations SET state=?1,finished_at=?2
                 WHERE id=?3 AND state='running'",
                    params![state, unix_time_millis()?, self.context.operation_id],
                )
                .map_err(CommandError::database)?;
            Ok(())
        });
        warn_accounting(result);
        crate::logging::info_target(
            "ai.generation",
            "generation_operation_finished",
            "Generation accounting is available through scripts/generation-usage.py.",
            json!({"operationId": self.context.operation_id, "state": state}),
        );
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        self.close("interrupted");
    }
}

pub(crate) struct Attempt {
    context: Context,
    id: String,
    finished: bool,
}

impl Context {
    pub(crate) fn start_attempt(
        &self,
        stage: &str,
        body: &Value,
        logical_key: Option<&str>,
        provider: &str,
    ) -> Result<Attempt, CommandError> {
        let bytes = serde_json::to_vec(body).map_err(CommandError::serialization)?;
        let request_key = logical_key
            .map(str::to_string)
            .unwrap_or_else(|| hash(&bytes));
        let sizes = payload::measure(body)?;
        let id = self.database.with_connection(|connection| {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(CommandError::database)?;
            let id = generate_id(&tx)?;
            let attempt: i64 = tx
                .query_row(
                    "SELECT COALESCE(MAX(attempt),0)+1 FROM generation_requests
                 WHERE operation_id=?1 AND stage=?2 AND request_key=?3",
                    params![self.operation_id, stage, request_key],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            tx.execute(
                "INSERT INTO generation_requests
                 (id,operation_id,stage,request_key,attempt,provider,model,status,
                  started_at,payload_bytes_json,max_output_tokens)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,'started',?8,?9,?10)",
                params![
                    id,
                    self.operation_id,
                    stage,
                    request_key,
                    attempt,
                    provider,
                    body.get("model")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown"),
                    unix_time_millis()?,
                    sizes.to_string(),
                    body.get("max_tokens").and_then(Value::as_i64).unwrap_or(0)
                ],
            )
            .map_err(CommandError::database)?;
            tx.commit().map_err(CommandError::database)?;
            Ok(id)
        })?;
        Ok(Attempt {
            context: self.clone(),
            id,
            finished: false,
        })
    }
}

tokio::task_local! {
    static CURRENT_REQUEST: RequestScope;
}

pub(crate) async fn scoped_request<F: Future>(
    context: Option<Context>,
    stage: &str,
    logical_key: Option<&str>,
    provider: &str,
    future: F,
) -> F::Output {
    let Some(context) = context else {
        return future.await;
    };
    CURRENT_REQUEST
        .scope(
            RequestScope {
                context,
                stage: stage.to_string(),
                logical_key: logical_key.map(str::to_string),
                provider: provider.to_string(),
            },
            future,
        )
        .await
}

pub(crate) fn start_current_attempt(body: &Value) -> Option<Attempt> {
    let result = CURRENT_REQUEST
        .try_with(|scope| {
            scope.context.start_attempt(
                &scope.stage,
                body,
                scope.logical_key.as_deref(),
                &scope.provider,
            )
        })
        .ok()?;
    match result {
        Ok(attempt) => Some(attempt),
        Err(error) => {
            warn_accounting(Err(error));
            None
        }
    }
}

impl Attempt {
    pub(crate) fn response(&mut self, response: &Value) {
        let usage = super::gateway::normalized_usage(response);
        let number = |pointer: &str| -> Option<i64> {
            usage
                .as_ref()
                .and_then(|value| value.pointer(pointer))
                .and_then(Value::as_i64)
                .filter(|value| *value >= 0)
        };
        let cost = usage
            .as_ref()
            .and_then(|value| value.get("cost"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0);
        let result = self.context.database.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE generation_requests SET status='response',finished_at=?1,
                 input_tokens=?2,output_tokens=?3,reasoning_tokens=?4,cached_input_tokens=?5,
                 cache_write_tokens=?6,total_tokens=?7,cost=?8,response_id=?9,finish_reason=?10,
                 payload_bytes_json=json_set(payload_bytes_json,'$.response',json(?11))
                 WHERE id=?12 AND status='started'",
                    params![
                        unix_time_millis()?,
                        number("/prompt_tokens"),
                        number("/completion_tokens"),
                        number("/completion_tokens_details/reasoning_tokens"),
                        number("/prompt_tokens_details/cached_tokens"),
                        number("/prompt_tokens_details/cache_write_tokens"),
                        number("/total_tokens"),
                        cost,
                        bounded(response.get("id").and_then(Value::as_str), 256),
                        bounded(
                            response
                                .pointer("/choices/0/finish_reason")
                                .and_then(Value::as_str),
                            80
                        ),
                        payload::response_measure(response).to_string(),
                        self.id
                    ],
                )
                .map_err(CommandError::database)?;
            Ok(())
        });
        self.finished = true;
        warn_accounting(result);
    }

    pub(crate) fn failed(&mut self, code: &str) {
        self.end("failed", Some(code));
    }

    fn end(&mut self, status: &str, code: Option<&str>) {
        if self.finished {
            return;
        }
        self.finished = true;
        warn_accounting(self.context.database.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE generation_requests SET status=?1,error_code=?2,finished_at=?3
                 WHERE id=?4 AND status='started'",
                    params![status, bounded(code, 80), unix_time_millis()?, self.id],
                )
                .map_err(CommandError::database)?;
            Ok(())
        }));
    }
}

impl Drop for Attempt {
    fn drop(&mut self) {
        self.end("interrupted", None);
    }
}

pub(crate) fn reconcile_startup(database: &Database) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let tx = connection.transaction().map_err(CommandError::database)?;
        let now = unix_time_millis()?;
        tx.execute(
            "UPDATE generation_requests SET status='interrupted',finished_at=?1
                    WHERE status='started'",
            [now],
        )
        .map_err(CommandError::database)?;
        tx.execute(
            "UPDATE generation_operations SET state='interrupted',finished_at=?1
                    WHERE state='running'",
            [now],
        )
        .map_err(CommandError::database)?;
        tx.commit().map_err(CommandError::database)
    })
}

// Captured by synchronous #[track_caller] request constructors, before polling.
// Classification is based on a compiled owning module, never on source/user text.
pub(crate) fn stage(file: &str) -> &'static str {
    match file.rsplit(['/', '\\']).next().unwrap_or(file) {
        "extraction.rs" => "knowledge_extraction",
        "document_view.rs" => "document_synthesis",
        "collection_view.rs" => "collection_synthesis",
        "flashcards.rs" => "flashcard_authoring",
        "curation.rs" => "flashcard_curation",
        "artifact_metadata.rs" => "artifact_metadata",
        "study_guide.rs" => "guide_authoring",
        _ => "unclassified",
    }
}

pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn bounded(value: Option<&str>, maximum: usize) -> Option<String> {
    value.map(|value| value.chars().take(maximum).collect())
}

fn warn_accounting(result: Result<(), CommandError>) {
    if let Err(error) = result {
        crate::logging::warn_target(
            "ai.generation",
            "generation_accounting_incomplete",
            "A diagnostic write failed; paid output and committed artifacts were not discarded.",
            json!({"code": error.code}),
        );
    }
}
