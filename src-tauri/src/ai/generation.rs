//! Durable, content-free accounting for each physical generation request.
//! Provider usage is authoritative. Payload sizes are separate diagnostics.
use std::{
    collections::{HashMap, HashSet},
    fmt,
    future::Future,
    sync::{Arc, Mutex},
};

use rusqlite::{params, TransactionBehavior};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::{
    database::Database,
    error::CommandError,
    util::{generate_id, unix_time_millis, validate_entity_id},
};

pub(crate) mod outcomes;
mod payload;
mod recovery;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod live_metrics_tests {
    use super::*;

    #[test]
    fn live_request_metrics_are_content_free_and_keep_evidence_separate() {
        let sizes = json!({
            "serializedRequestBytes": 12_000,
            "systemInstructionBytes": 1000,
            "embeddedSchemaBytes": 500,
            "wireSchemaBytes": 200,
            "userTextBytes": 4000,
            "imageCount": 3,
            "userParts": {
                "knowledgeUnits": 700,
                "groundingEvidence": 800,
                "sourceEvidence": 900,
                "otherUserText": 1600
            }
        });
        assert_eq!(live_request_metrics(&sizes), (12_000, 5_700, 3, 2_400));
    }
}

#[derive(Clone)]
pub(crate) struct Context {
    database: Database,
    pub(crate) operation_id: String,
    recovery: Arc<Mutex<RecoveryLedger>>,
    prefix_history: Arc<Mutex<HashMap<String, PrefixSample>>>,
}

#[derive(Clone)]
struct RequestScope {
    context: Context,
    stage: String,
    logical_key: String,
    provider: String,
}

const MAX_RECOVERIES_PER_LOGICAL_TASK: usize = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RecoveryReason {
    TransientTransport,
    ProviderCompatibility,
    ContractRepair,
    OutputCompaction,
}

impl RecoveryReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::TransientTransport => "transient_transport",
            Self::ProviderCompatibility => "provider_compatibility",
            Self::ContractRepair => "contract_repair",
            Self::OutputCompaction => "output_compaction",
        }
    }
}

#[derive(Debug, Default)]
struct RecoveryState {
    reasons: HashSet<RecoveryReason>,
}

#[derive(Debug, Default)]
struct RecoveryLedger {
    tasks: HashMap<String, RecoveryState>,
    parents: HashMap<String, String>,
    family_roots: HashSet<String>,
    started: HashSet<String>,
}

const CACHE_PREFIX_SAMPLE_MAX_BYTES: usize = 2 * 1024 * 1024;

fn live_request_metrics(sizes: &Value) -> (usize, usize, usize, usize) {
    let number = |pointer: &str| -> usize {
        sizes
            .pointer(pointer)
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0)
    };
    let text_bytes = number("/systemInstructionBytes")
        .saturating_add(number("/embeddedSchemaBytes"))
        .saturating_add(number("/wireSchemaBytes"))
        .saturating_add(number("/userTextBytes"));
    let evidence_bytes = [
        "/userParts/knowledgeUnits",
        "/userParts/groundingEvidence",
        "/userParts/structuralEvidence",
        "/userParts/sourceEvidence",
        "/userParts/sourceWindow",
        "/userParts/collectionDigest",
        "/userParts/learningPlan",
        "/userParts/outline",
        "/userParts/repairDraft",
        "/userParts/repairTargets",
    ]
    .into_iter()
    .map(number)
    .fold(0_usize, usize::saturating_add);
    (
        number("/serializedRequestBytes"),
        text_bytes,
        number("/imageCount"),
        evidence_bytes,
    )
}

#[derive(Debug)]
struct PrefixSample {
    bytes: Vec<u8>,
    request_hash: String,
    truncated: bool,
}

impl Context {
    pub(crate) fn provider_session_id(&self) -> String {
        format!("ostudy:{}", self.operation_id)
    }

    fn logical_request_key(&self, stage: &str, logical_key: Option<&str>) -> String {
        if let Some(key) = logical_key
            .filter(|key| key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return key.to_ascii_lowercase();
        }
        hash(
            format!(
                "generation-logical-task-v1|{}|{stage}|{}",
                self.operation_id,
                logical_key.unwrap_or("stage-default")
            )
            .as_bytes(),
        )
    }

    fn observe_cache_prefix(&self, stage: &str, provider: &str, bytes: &[u8]) -> Value {
        let sample_len = bytes.len().min(CACHE_PREFIX_SAMPLE_MAX_BYTES);
        let current = &bytes[..sample_len];
        let current_truncated = bytes.len() > sample_len;
        let request_hash = hash(bytes);
        let key = format!("{stage}|{provider}");
        let Ok(mut history) = self.prefix_history.lock() else {
            return json!({
                "comparable": false,
                "commonPrefixBytes": 0,
                "measurementBounded": false,
                "sampleLimitBytes": CACHE_PREFIX_SAMPLE_MAX_BYTES,
            });
        };
        let previous = history.get(&key);
        let common_prefix_bytes = previous
            .map(|previous| {
                previous
                    .bytes
                    .iter()
                    .zip(current)
                    .take_while(|(left, right)| left == right)
                    .count()
            })
            .unwrap_or(0);
        let measurement_bounded = previous.is_some_and(|previous| {
            common_prefix_bytes == CACHE_PREFIX_SAMPLE_MAX_BYTES
                && (previous.truncated || current_truncated)
        });
        let previous_request_hash = previous.map(|previous| previous.request_hash.clone());
        history.insert(
            key,
            PrefixSample {
                bytes: current.to_vec(),
                request_hash,
                truncated: current_truncated,
            },
        );
        json!({
            "comparable": previous_request_hash.is_some(),
            "commonPrefixBytes": common_prefix_bytes,
            "measurementBounded": measurement_bounded,
            "sampleLimitBytes": CACHE_PREFIX_SAMPLE_MAX_BYTES,
            "previousWireRequestHash": previous_request_hash,
        })
    }

    pub(crate) fn reserve_recovery(
        &self,
        stage: &str,
        logical_key: Option<&str>,
        reason: RecoveryReason,
        trigger_code: &str,
    ) -> bool {
        let request_key = self.logical_request_key(stage, logical_key);
        let ledger_key = format!("{stage}:{request_key}");
        let ineffective = outcomes::benefit::stop(self, stage, &request_key, reason, trigger_code);
        let allowed = !ineffective
            && match self.recovery.lock() {
                Ok(mut ledger) => ledger.reserve(ledger_key, reason),
                Err(_) => false,
            };
        let event = if allowed {
            crate::task_cancellation::record_current_provider_retry(reason.as_str());
            "generation_recovery_reserved"
        } else if ineffective {
            "generation_recovery_ineffective"
        } else {
            "generation_recovery_budget_exhausted"
        };
        crate::logging::debug_target(
            "ai.generation.recovery",
            event,
            "Applied the bounded logical-task recovery policy without logging prompt or response content.",
            json!({
                "operationId": self.operation_id,
                "stage": stage,
                "requestKey": request_key,
                "reason": reason.as_str(),
                "triggerCode": trigger_code.chars().take(80).collect::<String>(),
                "allowed": allowed,
                "maximumRecoveries": MAX_RECOVERIES_PER_LOGICAL_TASK,
            }),
        );
        outcomes::recovery(self, stage, &request_key, reason, trigger_code, allowed);
        allowed
    }
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
                recovery: Arc::new(Mutex::new(RecoveryLedger::default())),
                prefix_history: Arc::new(Mutex::new(HashMap::new())),
            },
            finished: false,
        })
    }

    pub(crate) fn context(&self) -> Context {
        self.context.clone()
    }

    // Test-only compatibility for historical ledger regressions. Production
    // owners must use finish_result so a terminal native cause is never reduced
    // to a boolean success/failure bit.
    #[cfg(test)]
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
        let mut sizes = payload::measure(body)?;
        if let Some(scope) = self.request_recovery_scope(stage, &request_key) {
            sizes["recoveryScope"] = scope;
        }
        if let Some(object) = sizes.as_object_mut() {
            object.insert(
                "cachePrefix".to_string(),
                self.observe_cache_prefix(stage, provider, &bytes),
            );
        }
        let (request_bytes, text_bytes, image_count, evidence_bytes) = live_request_metrics(&sizes);
        let model = body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let max_output_tokens = body
            .get("max_tokens")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0);
        crate::task_cancellation::record_current_request_diagnostics(
            crate::task_cancellation::CurrentRequestDiagnostics {
                stage,
                provider,
                model,
                request_bytes,
                text_bytes,
                image_count,
                evidence_bytes,
                max_output_tokens,
            },
        );
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
                    model,
                    unix_time_millis()?,
                    sizes.to_string(),
                    i64::try_from(max_output_tokens).unwrap_or(i64::MAX)
                ],
            )
            .map_err(CommandError::database)?;
            tx.commit().map_err(CommandError::database)?;
            Ok(id)
        })?;
        outcomes::started(self, stage, &request_key, &id);
        Ok(Attempt {
            context: self.clone(),
            id,
            finished: false,
        })
    }

    pub(crate) fn reused(&self, stage: &str, cache_key: &str) {
        warn_accounting(self.database.with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO generation_reuse_events(operation_id,stage,cache_key,created_at)
                     VALUES(?1,?2,?3,?4)",
                    params![self.operation_id, stage, cache_key, unix_time_millis()?],
                )
                .map_err(CommandError::database)?;
            Ok(())
        }));
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
    let logical_key = context.logical_request_key(stage, logical_key);
    CURRENT_REQUEST
        .scope(
            RequestScope {
                context,
                stage: stage.to_string(),
                logical_key,
                provider: provider.to_string(),
            },
            future,
        )
        .await
}

pub(crate) fn reserve_recovery(
    context: Option<&Context>,
    stage: &str,
    logical_key: Option<&str>,
    reason: RecoveryReason,
    trigger_code: &str,
) -> bool {
    match context {
        Some(context) => context.reserve_recovery(stage, logical_key, reason, trigger_code),
        None => {
            crate::task_cancellation::record_current_provider_retry(reason.as_str());
            true
        }
    }
}

pub(crate) fn reserve_current_recovery(reason: RecoveryReason, trigger_code: &str) -> bool {
    CURRENT_REQUEST
        .try_with(|scope| {
            scope.context.reserve_recovery(
                &scope.stage,
                Some(&scope.logical_key),
                reason,
                trigger_code,
            )
        })
        .unwrap_or_else(|_| {
            crate::task_cancellation::record_current_provider_retry(reason.as_str());
            true
        })
}

pub(crate) fn start_current_attempt(body: &Value) -> Option<Attempt> {
    let result = CURRENT_REQUEST
        .try_with(|scope| {
            scope.context.start_attempt(
                &scope.stage,
                body,
                Some(&scope.logical_key),
                &scope.provider,
            )
        })
        .ok()?;
    match result {
        Ok(attempt) => Some(attempt),
        Err(error) => {
            outcomes::accounting_gap();
            warn_accounting(Err(error));
            None
        }
    }
}

impl Attempt {
    pub(crate) fn response(&mut self, response: &Value) {
        self.finish_response(response, false, None);
    }

    /// Embedding APIs do not emit completion/reasoning tokens. Recording a
    /// provider response as zero-output is protocol semantics, not imputation.
    pub(crate) fn non_generative_response(&mut self, response: &Value) {
        self.finish_response(response, true, None);
    }

    /// A failed stream may already have supplied billable token usage. Store its
    /// safe observation and failure together, never a temporary success record.
    pub(crate) fn failed_with_observation(&mut self, code: &str, observation: &Value) {
        self.finish_response(observation, false, Some(code));
    }

    fn finish_response(&mut self, response: &Value, non_generative: bool, failure: Option<&str>) {
        if self.finished {
            return;
        }
        let usage = super::gateway::normalized_usage(response);
        let number = |pointer: &str| -> Option<i64> {
            usage
                .as_ref()
                .and_then(|value| value.pointer(pointer))
                .and_then(Value::as_i64)
                .filter(|value| *value >= 0)
        };
        let output_tokens = number("/completion_tokens").or(non_generative.then_some(0));
        let reasoning_tokens =
            number("/completion_tokens_details/reasoning_tokens").or(non_generative.then_some(0));
        let total_tokens = number("/total_tokens").or_else(|| {
            number("/prompt_tokens")
                .zip(output_tokens)
                .map(|(input, output)| input.saturating_add(output))
        });
        let cost = usage
            .as_ref()
            .and_then(|value| value.get("cost"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0);
        let result = self.context.database.with_connection(|connection| {
            connection
                .execute(
                    "UPDATE generation_requests SET status=?13,error_code=?14,finished_at=?1,
                 input_tokens=?2,output_tokens=?3,reasoning_tokens=?4,cached_input_tokens=?5,
                 cache_write_tokens=?6,total_tokens=?7,cost=?8,response_id=?9,finish_reason=?10,
                 payload_bytes_json=json_set(payload_bytes_json,'$.response',json(?11))
                 WHERE id=?12 AND status='started'",
                    params![
                        unix_time_millis()?,
                        number("/prompt_tokens"),
                        output_tokens,
                        reasoning_tokens,
                        number("/prompt_tokens_details/cached_tokens"),
                        number("/prompt_tokens_details/cache_write_tokens"),
                        total_tokens,
                        cost,
                        bounded(response.get("id").and_then(Value::as_str), 256),
                        bounded(
                            response
                                .pointer("/choices/0/finish_reason")
                                .and_then(Value::as_str),
                            80
                        ),
                        payload::response_measure(response).to_string(),
                        self.id,
                        if failure.is_some() {
                            "failed"
                        } else {
                            "response"
                        },
                        bounded(failure, 80)
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

/// Generation ledgers are local diagnostics. Device-sync packages do not
/// transfer provider usage, request fingerprints, or reuse history.
pub(crate) fn sanitize_for_device_transfer(
    connection: &rusqlite::Connection,
) -> Result<(), CommandError> {
    connection
        .execute("DELETE FROM generation_operations", [])
        .map_err(CommandError::database)?;
    Ok(())
}

pub(crate) fn reconcile_startup(database: &Database) -> Result<(), CommandError> {
    database.with_connection(|connection| {
        let tx = connection.transaction().map_err(CommandError::database)?;
        let now = unix_time_millis()?;
        outcomes::reconcile_in(&tx, now)?;
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
        "lesson.rs" => "lesson_authoring",
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
