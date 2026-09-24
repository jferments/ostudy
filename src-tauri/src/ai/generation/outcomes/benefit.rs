//! Same-strategy recovery is justified by observed native results, not a fresh
//! child key. Missing observations never become zero gain. This conservative
//! brake does not grade factual quality or change the hard recovery allowance.
use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

use super::{current, same, write, Context, RecoveryReason};
use crate::error::CommandError;

const REQUIRED_NO_GAIN_ROOTS: usize = 2;
const MAX_OBSERVATIONS: usize = 256;

pub(crate) fn stop(
    context: &Context,
    stage: &str,
    key: &str,
    reason: RecoveryReason,
    code: &str,
) -> bool {
    if reason != RecoveryReason::ContractRepair {
        return false;
    }
    let request = current().and_then(|current| {
        let state = current.lock().ok()?;
        (same(&state, context, stage, key) && !state.linkage_incomplete)
            .then(|| state.last_request.clone())
            .flatten()
    });
    let Some(request) = request else {
        return false;
    };
    let evidence = context
        .database
        .with_connection(|connection| observations(connection, context, &request, stage, code))
        .ok()
        .flatten();
    let Some((roots, progress)) = evidence else {
        return false;
    };
    let stop = !progress && roots >= REQUIRED_NO_GAIN_ROOTS;
    write(
        context,
        &request,
        "$.nativeRecoveryPolicy",
        json!({
            "schemaVersion": 1,
            "decision": if stop { "stop_same_strategy" } else { "budget_policy" },
            "reason": "observed_native_progress",
            "noGainSiblingRoots": roots,
            "recentProgressObserved": progress,
            "requiredNoGainRoots": REQUIRED_NO_GAIN_ROOTS,
            "inspectionLimit": MAX_OBSERVATIONS,
            "quality": "not_measured",
        }),
    );
    stop
}

fn decode(text: &str) -> Option<Value> {
    if text.len() > 128_000 {
        return None;
    }
    serde_json::from_str(text).ok()
}

fn fingerprint(value: &Value) -> Option<&str> {
    if value["schemaVersion"] != 1 {
        return None;
    }
    value["requestContractFingerprint"]
        .as_str()
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn root(value: &Value) -> Option<&str> {
    let scope = &value["recoveryScope"];
    if scope["version"] != 1 {
        return None;
    }
    scope["rootTaskKey"].as_str().filter(|key| {
        let Some((stage, hash)) = key.split_once(':') else {
            return false;
        };
        !stage.is_empty()
            && stage.len() <= 80
            && stage
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            && hash.len() == 64
            && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn number(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .filter(|number| *number <= u64::from(u32::MAX))
}

fn snapshot(value: &Value) -> Option<[u64; 5]> {
    value["protocolComplete"].as_bool()?;
    let numbers = [
        number(&value["acceptedUnits"])?,
        number(&value["rejectedProposals"])?,
        number(&value["unaccountedSources"])?,
        number(&value["unresolvedRegions"])?,
        number(&value["deferredAnnotations"]["count"])?,
    ];
    if numbers[4] > numbers[0]
        || (value["protocolComplete"] == true && numbers[1..].iter().any(|count| *count != 0))
    {
        return None;
    }
    for (rules, count) in [
        (&value["rejectionRules"], numbers[1]),
        (&value["deferredAnnotations"]["rules"], numbers[4]),
    ] {
        let rules = rules.as_object()?;
        let total = rules
            .values()
            .try_fold(0_u64, |sum, value| sum.checked_add(number(value)?))?;
        if total != count {
            return None;
        }
    }
    Some(numbers)
}

fn admission(value: &Value) -> Option<&Value> {
    let record = &value["nativeAdmission"];
    if record["schemaVersion"] != 1
        || record["state"] != "failed"
        || record["failureBoundary"] != "all_candidates_rejected"
        || record["persistence"] != "not_observed"
        || record["admittedCandidates"] != 0
        || record["unexaminedCandidates"] != 0
        || !record["selectedCandidates"].is_null()
    {
        return None;
    }
    let count = number(&record["declaredCandidates"])?;
    if count == 0
        || count > 128
        || number(&record["examinedCandidates"])? != count
        || number(&record["rejectedCandidates"])? != count
        || record["rejections"].as_array()?.len() as u64 != count
    {
        return None;
    }
    Some(record)
}

fn signature(value: &Value) -> Option<Value> {
    let checkpoint = &value["nativeCheckpoint"];
    if checkpoint["schemaVersion"] == 1 {
        let after = &checkpoint["after"];
        let counts = snapshot(after)?;
        return Some(json!({
            "kind": "knowledge",
            "rules": after["rejectionRules"],
            "unaccounted": counts[2] > 0,
            "deferredRules": after["deferredAnnotations"]["rules"],
        }));
    }
    let admission = admission(value)?;
    let mut failures = BTreeSet::new();
    for rejected in admission["rejections"].as_array()? {
        if rejected["retryable"] != true {
            return None;
        }
        failures.insert((rejected["rule"].as_str()?, rejected["errorCode"].as_str()?));
    }
    Some(json!({"kind": "cards", "failures": failures}))
}

/// Some(true) is measured progress, Some(false) is the identical unresolved
/// fingerprint with no measured progress; None is an inconclusive comparison.
fn progress(before: &Value, after: &Value) -> Option<bool> {
    let checkpoint = &after["nativeCheckpoint"];
    if checkpoint["schemaVersion"] == 1 {
        if checkpoint["before"] != before["nativeCheckpoint"]["after"] {
            return None;
        }
        let old = snapshot(&checkpoint["before"])?;
        let new = snapshot(&checkpoint["after"])?;
        if new[0] > old[0]
            || (1..5).any(|index| new[index] < old[index])
            || (checkpoint["before"]["protocolComplete"] == false
                && checkpoint["after"]["protocolComplete"] == true)
        {
            return Some(true);
        }
        return (signature(before)? == signature(after)?).then_some(false);
    }
    if after["nativeAuthoring"]["schemaVersion"] == 1
        && after["nativeAuthoring"]["state"] == "returned"
        && number(&after["nativeAuthoring"]["returnedItems"]).is_some_and(|count| count > 0)
    {
        return Some(true);
    }
    // A failed guide can still contain useful fields; without their measured
    // admission do not infer zero progress. A fully rejected card response has
    // a complete native admission record and can be compared safely.
    (signature(before)? == signature(after)?).then_some(false)
}

fn observations(
    connection: &Connection,
    context: &Context,
    request: &str,
    stage: &str,
    code: &str,
) -> Result<Option<(usize, bool)>, CommandError> {
    let current: Option<(String, String, String)> = connection
        .query_row(
            "SELECT payload_bytes_json,provider,model FROM generation_requests
             WHERE id=?1 AND operation_id=?2 AND stage=?3 AND status='response'",
            params![request, context.operation_id, stage],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((payload, provider, model)) = current else {
        return Ok(None);
    };
    let Some(current) = decode(&payload) else {
        return Ok(None);
    };
    let (Some(contract), Some(current_root), Some(expected)) =
        (fingerprint(&current), root(&current), signature(&current))
    else {
        return Ok(None);
    };
    let mut query = connection
        .prepare(
            "SELECT p.id,p.payload_bytes_json,r.payload_bytes_json
             FROM generation_requests r JOIN generation_requests p
               ON p.id=json_extract(CASE WHEN json_valid(r.payload_bytes_json)
                    THEN r.payload_bytes_json ELSE '{}' END,'$.nativeLineage.previousRequestId')
             WHERE r.operation_id=?1 AND p.operation_id=r.operation_id
               AND r.stage=?2 AND p.stage=r.stage AND p.request_key=r.request_key
               AND r.provider=?3 AND p.provider=r.provider
               AND r.model=?4 AND p.model=r.model
               AND r.status='response' AND p.status='response'
               AND r.finished_at IS NOT NULL AND p.id<>?5
             ORDER BY r.finished_at DESC,r.rowid DESC LIMIT ?6",
        )
        .map_err(CommandError::database)?;
    let rows = query
        .query_map(
            params![
                context.operation_id,
                stage,
                provider,
                model,
                request,
                MAX_OBSERVATIONS
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(CommandError::database)?;
    let mut roots = BTreeSet::<String>::new();
    for row in rows {
        let (previous_id, before, after) = row.map_err(CommandError::database)?;
        let (Some(before), Some(after)) = (decode(&before), decode(&after)) else {
            continue;
        };
        let Some(owner) = root(&before) else {
            continue;
        };
        let lineage = &after["nativeLineage"];
        let task = &after["nativeTask"];
        if owner == current_root
            || roots.contains(owner)
            || root(&after) != Some(owner)
            || fingerprint(&before) != Some(contract)
            || signature(&before).as_ref() != Some(&expected)
            || lineage["schemaVersion"] != 1
            || lineage["linkageIncomplete"] != false
            || lineage["recovery"]["schemaVersion"] != 1
            || lineage["recovery"]["reason"] != "contract_repair"
            || lineage["recovery"]["triggerCode"] != code
            || lineage["recovery"]["allowed"] != true
            || lineage["recovery"]["triggerRequestId"] != previous_id
            || task["schemaVersion"] != 1
            || !matches!(task["state"].as_str(), Some("returned" | "failed"))
        {
            continue;
        }
        match progress(&before, &after) {
            Some(true) => return Ok(Some((roots.len(), true))),
            Some(false) => {
                roots.insert(owner.to_string());
            }
            None => continue,
        }
        if roots.len() >= REQUIRED_NO_GAIN_ROOTS {
            break;
        }
    }
    Ok(Some((roots.len(), false)))
}

#[cfg(test)]
mod tests;
