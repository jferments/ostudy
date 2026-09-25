//! Native recovery families keep request splitting from multiplying retries.
//! Request identity and usage stay per child; only the spending allowance is
//! shared. Registration never grants a retry or marks partial output complete.
use serde_json::{json, Value};

use super::{Context, RecoveryLedger, RecoveryReason, MAX_RECOVERIES_PER_LOGICAL_TASK};
use crate::error::CommandError;

impl RecoveryLedger {
    fn root<'a>(&'a self, key: &'a str) -> &'a str {
        self.parents.get(key).map_or(key, String::as_str)
    }

    fn bind(&mut self, root: String, children: &[String]) -> Result<(), CommandError> {
        // Flat native-owned families only. A root cannot later become a child,
        // and an already-used child cannot be redirected to an unspent budget.
        if self.parents.contains_key(&root) {
            return Err(binding_error());
        }
        for child in children.iter().filter(|child| **child != root) {
            match self.parents.get(child) {
                Some(existing) if existing == &root => {}
                Some(_) => return Err(binding_error()),
                None if self.started.contains(child) || self.family_roots.contains(child) => {
                    return Err(binding_error());
                }
                None => {}
            }
        }
        // Validate the whole batch before mutation: no half-bound family.
        self.family_roots.insert(root.clone());
        for child in children.iter().filter(|child| **child != root) {
            self.parents.insert(child.clone(), root.clone());
        }
        Ok(())
    }

    pub(super) fn reserve(&mut self, key: String, reason: RecoveryReason) -> bool {
        self.started.insert(key.clone());
        let root = self.root(&key).to_string();
        let state = self.tasks.entry(root).or_default();
        if state.reasons.contains(&reason) || state.reasons.len() >= MAX_RECOVERIES_PER_LOGICAL_TASK
        {
            return false;
        }
        state.reasons.insert(reason);
        true
    }
}

fn binding_error() -> CommandError {
    CommandError::new(
        "generation_recovery_scope_invalid",
        "Recovery task ownership changed; an existing allowance was not reset.",
    )
}

fn scoped(stage: &str, key: &str) -> Result<String, CommandError> {
    if stage.is_empty()
        || stage.len() > 80
        || !stage
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
        || key.len() != 64
        || !key.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(binding_error());
    }
    Ok(format!("{stage}:{}", key.to_ascii_lowercase()))
}

impl Context {
    /// Bind planned request keys before dispatch. Existing parent reservations
    /// remain spent. Repeating the same binding is idempotent, not a new budget.
    pub(crate) fn bind_recovery_family(
        &self,
        stage: &str,
        root_key: &str,
        children: &[String],
    ) -> Result<(), CommandError> {
        if children.is_empty() || children.len() > 16_384 {
            return Err(binding_error());
        }
        let root = scoped(stage, root_key)?;
        let children = children
            .iter()
            .map(|key| scoped(stage, key))
            .collect::<Result<Vec<_>, _>>()?;
        self.recovery
            .lock()
            .map_err(|_| binding_error())?
            .bind(root, &children)
    }

    /// Captured before starting the physical request. Stored separately from
    /// provider usage and native outcome lineage; historical rows stay unknown.
    pub(super) fn request_recovery_scope(&self, stage: &str, key: &str) -> Option<Value> {
        let normalized = self.logical_request_key(stage, Some(key));
        let scoped_key = format!("{stage}:{normalized}");
        let mut ledger = self.recovery.lock().ok()?;
        ledger.started.insert(scoped_key.clone());
        let root = ledger.root(&scoped_key);
        Some(json!({
            "version": 1,
            "rootTaskKey": root,
            "shared": root != scoped_key,
            "maximumRecoveries": MAX_RECOVERIES_PER_LOGICAL_TASK,
            "maximumPerReason": 1,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ai::generation::Operation, database::Database};
    use RecoveryReason::{
        ContractRepair, OutputCompaction, ProviderCompatibility, TransientTransport,
    };

    const GUIDE: &str = "guide_authoring";

    fn key(index: usize) -> String {
        format!("{index:064x}")
    }

    fn bind(context: &Context, root: usize, children: &[usize]) -> Result<(), CommandError> {
        let keys = children.iter().copied().map(key).collect::<Vec<_>>();
        context.bind_recovery_family(GUIDE, &key(root), &keys)
    }

    fn reserve(context: &Context, index: usize, reason: RecoveryReason) -> bool {
        context.reserve_recovery(GUIDE, Some(&key(index)), reason, "fixture_contract")
    }

    fn context() -> (Database, Operation, Context) {
        let database = Database::open_in_memory().unwrap();
        let operation = Operation::begin(&database, None, None, None, "recovery_test").unwrap();
        let context = operation.context();
        (database, operation, context)
    }

    #[test]
    fn document_recovery_children_share_parent_reasons_and_total_allowance() {
        let (_, _operation, context) = context();
        assert!(reserve(&context, 1, ContractRepair));
        bind(&context, 1, &[2, 3, 4]).unwrap();
        assert!(!reserve(&context, 2, ContractRepair));
        assert!(reserve(&context, 3, TransientTransport));
        assert!(!reserve(&context, 4, ProviderCompatibility));
        assert!(!reserve(&context, 1, OutputCompaction));
    }

    #[test]
    fn document_recovery_registration_is_atomic_and_reparenting_cannot_reset_budget() {
        let (_, _operation, context) = context();
        bind(&context, 1, &[2]).unwrap();
        assert!(reserve(&context, 2, ContractRepair));
        assert!(bind(&context, 3, &[4, 2]).is_err());
        let ledger = context.recovery.lock().unwrap();
        assert!(!ledger.parents.contains_key(&format!("{GUIDE}:{}", key(4))));
        drop(ledger);
        bind(&context, 1, &[2]).unwrap();
        assert!(!reserve(&context, 2, ContractRepair));
    }

    #[test]
    fn document_recovery_started_children_and_nested_roots_cannot_change_owner() {
        let (_, _operation, context) = context();
        context.request_recovery_scope(GUIDE, &key(2)).unwrap();
        assert!(bind(&context, 1, &[2]).is_err());
        bind(&context, 3, &[4]).unwrap();
        assert!(bind(&context, 1, &[3]).is_err());
        assert!(bind(&context, 4, &[3]).is_err());
    }

    #[test]
    fn document_recovery_unrelated_roots_stages_and_operations_remain_independent() {
        let (_, _operation, context) = context();
        for root in [1, 3] {
            bind(&context, root, &[root + 1]).unwrap();
            assert!(reserve(&context, root + 1, ContractRepair));
        }
        context
            .bind_recovery_family("flashcard_authoring", &key(1), &[key(2)])
            .unwrap();
        assert!(context.reserve_recovery(
            "flashcard_authoring",
            Some(&key(2)),
            ContractRepair,
            "fixture_contract",
        ));
        let (_, _other_operation, other) = self::context();
        bind(&other, 1, &[2]).unwrap();
        assert!(reserve(&other, 2, ContractRepair));
    }

    #[test]
    fn document_recovery_concurrent_siblings_cannot_reserve_the_same_reason_twice() {
        let (_, _operation, context) = context();
        let children = (2..10).collect::<Vec<_>>();
        bind(&context, 1, &children).unwrap();
        let count = std::thread::scope(|scope| {
            let handles = children
                .iter()
                .map(|child| {
                    let context = &context;
                    scope.spawn(move || reserve(context, *child, ContractRepair))
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(|allowed| *allowed)
                .count()
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn document_recovery_scope_is_durable_without_collapsing_request_identity_or_usage() {
        let (database, _operation, context) = context();
        bind(&context, 1, &[2, 3]).unwrap();
        let body = json!({"model":"fixture","max_tokens":3200,"messages":[]});
        for child in [key(2), key(3)] {
            let mut attempt = context
                .start_attempt(GUIDE, &body, Some(&child), "fixture")
                .unwrap();
            attempt.response(&json!({
                "usage":{"prompt_tokens":10,"completion_tokens":3,"total_tokens":13}
            }));
        }
        database
            .with_connection(|connection| {
                let mut query = connection
                    .prepare(
                        "SELECT request_key,attempt,input_tokens,output_tokens,payload_bytes_json
                         FROM generation_requests ORDER BY request_key",
                    )
                    .map_err(CommandError::database)?;
                let rows = query
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, String>(4)?,
                        ))
                    })
                    .map_err(CommandError::database)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CommandError::database)?;
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].0, key(2));
                assert_eq!(rows[1].0, key(3));
                for (_, ordinal, input, output, payload) in rows {
                    assert_eq!((ordinal, input, output), (1, 10, 3));
                    let value: Value = serde_json::from_str(&payload).unwrap();
                    assert_eq!(
                        value["recoveryScope"]["rootTaskKey"],
                        format!("{GUIDE}:{}", key(1))
                    );
                    assert_eq!(value["recoveryScope"]["shared"], true);
                    assert_eq!(value["recoveryScope"]["maximumRecoveries"], 2);
                }
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn document_recovery_invalid_binding_does_not_create_or_clear_state() {
        let (_, _operation, context) = context();
        for (stage, root, children) in [
            (GUIDE, "private source text".into(), vec![key(2)]),
            (GUIDE, key(1), vec!["invalid".into()]),
            ("invalid:stage", key(1), vec![key(2)]),
            (GUIDE, key(1), vec![]),
        ] {
            let error = context
                .bind_recovery_family(stage, &root, &children)
                .unwrap_err();
            assert_eq!(error.code, "generation_recovery_scope_invalid");
            assert!(!error.message.contains("private source text"));
        }
        assert!(context.recovery.lock().unwrap().parents.is_empty());
    }
}
