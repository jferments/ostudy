#!/usr/bin/env python3
"""Read-only AC-06 comparison of hash-bound usage exports and human reviews.

This program never generates material, opens a library, or calls a provider.
It validates submitted evidence, not the truth of a review or a build attestation.
Synthetic evidence cannot qualify live-provider acceptance.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import sys
from typing import Any

KINDS = frozenset({"textbook", "slides", "table", "numeric", "cross_page",
                   "assessment", "image", "collection"})
CRITERIA = ("factualSupport", "preservedQualifications", "usefulCoverage",
            "readability", "answerKeyCorrectness", "artifactPublication")
REGIMES = frozenset({"fresh", "resume"})
RECOVERY_REASONS = frozenset({"transient_transport", "provider_compatibility",
                              "contract_repair", "output_compaction"})
MAX_JSON_BYTES = 32 * 1024 * 1024
MAX_CASES = 128
_CODE = re.compile(r"[a-z][a-z0-9_]{0,63}\Z")


class EvidenceError(ValueError):
    """Messages are static codes; no source text or local paths escape."""


def require(condition: bool, code: str) -> None:
    if not condition:
        raise EvidenceError(code)


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def identity(value: Any) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":"),
                             ensure_ascii=True, allow_nan=False).encode("utf-8"))


def hexadecimal(value: Any, length: int) -> bool:
    return (isinstance(value, str) and len(value) == length
            and all(character in "0123456789abcdef" for character in value))


def natural(value: Any) -> bool:
    return type(value) is int and 0 <= value <= 9_223_372_036_854_775_807


def positive(value: Any) -> bool:
    if type(value) not in (int, float):
        return False
    try:
        return math.isfinite(value) and value > 0
    except OverflowError:
        return False


def code(value: Any) -> bool:
    return isinstance(value, str) and _CODE.fullmatch(value) is not None


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        require(key not in result, "duplicate_json_key")
        result[key] = value
    return result


def parse_json(raw: bytes) -> Any:
    require(len(raw) <= MAX_JSON_BYTES, "json_size_limit")
    try:
        return json.loads(raw, object_pairs_hook=unique_object,
                          parse_constant=lambda _: reject_constant(), parse_float=finite_float)
    except (UnicodeError, json.JSONDecodeError, RecursionError) as error:
        raise EvidenceError("invalid_json") from error


def finite_float(value: str) -> float:
    result = float(value)
    require(math.isfinite(result), "nonfinite_json_number")
    return result


def reject_constant() -> None:
    raise EvidenceError("nonfinite_json_number")


def read_json(path: Path) -> Any:
    with path.open("rb") as stream:
        return parse_json(stream.read(MAX_JSON_BYTES + 1))


def checked_file(root: Path, reference: Any, *, json_file: bool = False) -> Any:
    require(isinstance(reference, dict), "file_reference_invalid")
    name = reference.get("path")
    expected = reference.get("sha256")
    require(isinstance(name, str) and bool(name) and not Path(name).is_absolute(),
            "relative_file_required")
    require(hexadecimal(expected, 64), "file_hash_invalid")
    try:
        path = (root / name).resolve(strict=True)
        require(path.is_relative_to(root) and path.is_file(), "file_outside_evidence_root")
        hasher = hashlib.sha256()
        chunks: list[bytes] = []
        size = 0
        with path.open("rb") as stream:
            while chunk := stream.read(1024 * 1024):
                hasher.update(chunk)
                size += len(chunk)
                if json_file:
                    require(size <= MAX_JSON_BYTES, "json_size_limit")
                    chunks.append(chunk)
        require(size > 0, "evidence_file_empty")
        require(hasher.hexdigest() == expected, "file_hash_mismatch")
        return parse_json(b"".join(chunks)) if json_file else expected
    except (OSError, ValueError) as error:
        if isinstance(error, EvidenceError):
            raise
        raise EvidenceError("evidence_file_unreadable") from error


def issue(issues: list[dict[str, str]], code_value: str, *, rejected: bool = False,
          side: str | None = None) -> None:
    item = {"code": code_value, "state": "rejected" if rejected else "incomplete"}
    if side is not None:
        item["side"] = side
    issues.append(item)


def request_payload(row: dict[str, Any]) -> dict[str, Any] | None:
    payload = row.get("payloadBytesJson")
    if payload is None:
        return None
    if isinstance(payload, str):
        try:
            payload = parse_json(payload.encode("utf-8"))
        except (UnicodeError, EvidenceError) as error:
            raise EvidenceError("recovery_lineage_invalid") from error
    require(isinstance(payload, dict), "recovery_lineage_invalid")
    return payload


def is_recovery_request(row: dict[str, Any]) -> bool:
    attempt = row.get("attempt")
    require(natural(attempt) and attempt >= 1, "request_attempt_invalid")
    repeated = attempt > 1
    payload = request_payload(row)
    if payload is None or "nativeLineage" not in payload:
        return repeated
    lineage = payload["nativeLineage"]
    require(isinstance(lineage, dict) and lineage.get("schemaVersion") == 1,
            "recovery_lineage_invalid")
    recovery = lineage.get("recovery")
    if recovery is None:
        return repeated
    require(isinstance(recovery, dict) and recovery.get("schemaVersion") == 1,
            "recovery_lineage_invalid")
    require(type(recovery.get("allowed")) is bool, "recovery_lineage_invalid")
    require(recovery.get("reason") in RECOVERY_REASONS, "recovery_lineage_invalid")
    return repeated or recovery["allowed"] is True


def usage_metrics(report: Any, run: dict[str, Any], issues: list[dict[str, str]],
                  side: str) -> dict[str, Any]:
    require(isinstance(report, dict) and type(report.get("schemaVersion")) is int
            and report["schemaVersion"] == 1, "usage_version_invalid")
    operation = report.get("operation")
    require(isinstance(operation, dict), "single_operation_required")
    operation_id = operation.get("id")
    require(isinstance(operation_id, str) and bool(operation_id)
            and operation_id == report.get("operationId")
            and operation_id == run.get("operationId"), "operation_identity_mismatch")
    state = operation.get("state")
    if state != "complete":
        issue(issues, "operation_not_complete", rejected=state in ("failed", "interrupted"),
              side=side)
    started, finished = operation.get("startedAt"), operation.get("finishedAt")
    elapsed = finished - started if (natural(started) and natural(finished)
                                    and finished >= started) else None
    if elapsed is None:
        issue(issues, "operation_duration_unknown", side=side)
    rows = report.get("requests")
    require(isinstance(rows, list), "usage_requests_invalid")
    seen: set[str] = set()
    totals = {"inputTokens": 0, "outputTokens": 0}
    missing = {field: 0 for field in totals}
    cache_known = 0
    cached = 0
    cache_input = 0
    recovery_rows: list[dict[str, Any]] = []
    for row in rows:
        require(isinstance(row, dict), "usage_request_invalid")
        request_id = row.get("requestId")
        require(isinstance(request_id, str) and bool(request_id) and request_id not in seen,
                "request_identity_ambiguous")
        seen.add(request_id)
        require(row.get("operationId") == operation_id, "cross_operation_request")
        require(isinstance(row.get("provider"), str) and bool(row["provider"])
                and isinstance(row.get("model"), str) and bool(row["model"]),
                "provider_identity_missing")
        if is_recovery_request(row):
            recovery_rows.append(row)
        for field in totals:
            value = row.get(field)
            if natural(value):
                totals[field] += value
            else:
                missing[field] += 1
        for subset, whole in (("cachedInputTokens", "inputTokens"),
                              ("reasoningTokens", "outputTokens")):
            value = row.get(subset)
            if value is not None:
                require(natural(value), "usage_subset_invalid")
                if natural(row.get(whole)):
                    require(value <= row[whole], "usage_subset_exceeds_total")
        if natural(row.get("cachedInputTokens")) and natural(row.get("inputTokens")):
            cache_known += 1
            cached += row["cachedInputTokens"]
            cache_input += row["inputTokens"]
        if row.get("status") not in ("response", "failed", "interrupted"):
            issue(issues, "request_not_terminal", side=side)
    if any(missing.values()):
        issue(issues, "token_usage_incomplete", side=side)
    accounted = run.get("requestAccountingComplete") is True
    if not accounted:
        issue(issues, "request_accounting_unconfirmed", side=side)
    recovery_accounted = run.get("recoveryAccountingComplete") is True
    if not recovery_accounted:
        issue(issues, "recovery_accounting_unconfirmed", side=side)
    if not rows:
        reuse = report.get("localReuse")
        reuse_recorded = (isinstance(reuse, list) and any(
            isinstance(item, dict) and item.get("operationId") == operation_id
            and natural(item.get("count")) and item["count"] > 0 for item in reuse))
        if run.get("zeroRequestReuseObserved") is not True or not reuse_recorded:
            issue(issues, "zero_request_reuse_unconfirmed", side=side)
            accounted = False
    complete = accounted and not any(missing.values())
    recovery_totals = {"inputTokens": 0, "outputTokens": 0}
    recovery_missing = {field: 0 for field in recovery_totals}
    for row in recovery_rows:
        for field in recovery_totals:
            value = row.get(field)
            if natural(value):
                recovery_totals[field] += value
            else:
                recovery_missing[field] += 1
    if any(recovery_missing.values()):
        issue(issues, "recovery_token_usage_incomplete", side=side)
    recovery_complete = accounted and recovery_accounted and not any(recovery_missing.values())
    recovery_tokens = sum(recovery_totals.values()) if recovery_complete else None
    return {
        "providerRequests": len(rows),
        "reportedInputTokens": (totals["inputTokens"]
                                if not missing["inputTokens"] and (rows or accounted) else None),
        "reportedOutputTokens": (totals["outputTokens"]
                                 if not missing["outputTokens"] and (rows or accounted) else None),
        "inputOutputTokens": sum(totals.values()) if complete else None,
        "missingInputUsageRequests": missing["inputTokens"],
        "missingOutputUsageRequests": missing["outputTokens"],
        "recoveryProviderRequests": len(recovery_rows),
        "recoveryReportedInputTokens": (
            recovery_totals["inputTokens"] if not recovery_missing["inputTokens"]
            and recovery_accounted else None),
        "recoveryReportedOutputTokens": (
            recovery_totals["outputTokens"] if not recovery_missing["outputTokens"]
            and recovery_accounted else None),
        "recoveryInputOutputTokens": recovery_tokens,
        "recoveryTokenSharePercent": (
            100.0 * recovery_tokens / sum(totals.values())
            if recovery_tokens is not None and complete and sum(totals.values()) else
            (0.0 if recovery_tokens == 0 and complete else None)),
        "missingRecoveryInputUsageRequests": recovery_missing["inputTokens"],
        "missingRecoveryOutputUsageRequests": recovery_missing["outputTokens"],
        "operationElapsedMs": elapsed,
        "cacheReadCoverageRequests": cache_known,
        "cacheReadPercentOfCoveredInput": 100.0 * cached / cache_input if cache_input else None,
    }


def availability_metrics(report: dict[str, Any], artifacts: list[dict[str, Any]],
                         issues: list[dict[str, str]], side: str) -> int | None:
    """Use recorded artifact availability, never a provider response timestamp."""
    operation = report["operation"]
    start, end = operation.get("startedAt"), operation.get("finishedAt")
    times = [artifact.get("usableAt") for artifact in artifacts]
    if (not times or not all(natural(value) for value in times)
            or not natural(start) or not natural(end) or end < start):
        issue(issues, "first_usable_artifact_unknown", side=side)
        return None
    require(all(start <= value <= end for value in times), "artifact_availability_outside_run")
    return min(times) - start


def review_evidence(review: Any, artifact_identity: str, objectives: list[str],
                    issues: list[dict[str, str]], side: str) -> None:
    if not isinstance(review, dict):
        issue(issues, "review_missing", side=side)
        return
    require(review.get("artifactSetSha256") == artifact_identity, "review_artifact_mismatch")
    if not isinstance(review.get("reviewer"), str) or not review["reviewer"].strip():
        issue(issues, "reviewer_unrecorded", side=side)
    criteria = review.get("criteria")
    if not isinstance(criteria, dict):
        issue(issues, "quality_review_missing", side=side)
    else:
        for criterion in CRITERIA:
            result = criteria.get(criterion)
            if result != "pass":
                issue(issues, f"quality_{criterion}", rejected=result == "fail", side=side)
    reviewed = review.get("objectives")
    if not isinstance(reviewed, dict):
        issue(issues, "objective_review_missing", side=side)
        return
    require(not (set(reviewed) - set(objectives)), "unexpected_review_objective")
    for objective in objectives:
        result = reviewed.get(objective)
        if result != "covered":
            issue(issues, "objective_not_covered", rejected=result == "not_covered", side=side)


def evaluate_case(root: Path, case: Any, baseline_sha: str, candidate_sha: str,
                  limits: dict[str, Any]) -> dict[str, Any]:
    require(isinstance(case, dict) and code(case.get("id")), "case_identity_invalid")
    require(isinstance(case.get("kind"), str) and case["kind"] in KINDS, "case_kind_invalid")
    require(case.get("regime") in REGIMES, "generation_regime_unrecorded")
    objectives = case.get("objectiveIds")
    require(isinstance(objectives, list) and bool(objectives) and all(map(code, objectives))
            and len(objectives) == len(set(objectives)), "objectives_invalid")
    artifact_kinds = case.get("artifactKinds")
    require(isinstance(artifact_kinds, list) and bool(artifact_kinds)
            and all(map(code, artifact_kinds)) and len(artifact_kinds) == len(set(artifact_kinds)),
            "artifact_kinds_invalid")
    source_files = case.get("sources")
    require(isinstance(source_files, list) and bool(source_files), "sources_missing")
    source_hash = identity([checked_file(root, source) for source in source_files])
    configuration_hash = checked_file(root, case.get("configuration"))
    issues: list[dict[str, str]] = []
    results = {}
    snapshots: set[str] = set()
    for side, head in (("baseline", baseline_sha), ("candidate", candidate_sha)):
        run = case.get(side)
        require(isinstance(run, dict), "run_missing")
        require(run.get("headSha") == head, "run_head_mismatch")
        require(run.get("sourceSetSha256") == source_hash, "run_source_mismatch")
        require(run.get("configurationSha256") == configuration_hash, "run_configuration_mismatch")
        require(run.get("generationRegime") == case["regime"], "generation_regime_mismatch")
        require(run.get("cacheState") in ("cold", "warm"), "cache_state_unrecorded")
        require(run.get("evidenceKind") in ("live_provider", "synthetic"), "evidence_kind_invalid")
        if run["evidenceKind"] != "live_provider":
            issue(issues, "synthetic_not_live_acceptance", side=side)
        usage = checked_file(root, run.get("usage"), json_file=True)
        usage_hash = run["usage"]["sha256"]
        require(usage_hash not in snapshots, "same_usage_snapshot_reused")
        snapshots.add(usage_hash)
        artifacts = run.get("artifacts")
        require(isinstance(artifacts, list), "artifacts_invalid")
        artifact_hashes: dict[str, str] = {}
        for artifact in artifacts:
            require(isinstance(artifact, dict) and artifact.get("kind") in artifact_kinds,
                    "artifact_kind_unexpected")
            kind = artifact["kind"]
            require(kind not in artifact_hashes, "duplicate_artifact_kind")
            artifact_hashes[kind] = checked_file(root, artifact)
        if set(artifact_hashes) != set(artifact_kinds):
            issue(issues, "published_artifact_missing", side=side)
        review_evidence(run.get("review"), identity(artifact_hashes), objectives, issues, side)
        results[side] = usage_metrics(usage, run, issues, side)
        results[side]["firstUsableArtifactMs"] = availability_metrics(usage, artifacts, issues, side)
    require(case["baseline"]["cacheState"] == case["candidate"]["cacheState"],
            "cache_state_mismatch")
    ratios = {}
    for metric, limit_name in (
        ("inputOutputTokens", "maxInputOutputTokenRatio"),
        ("recoveryInputOutputTokens", "maxRecoveryInputOutputTokenRatio"),
        ("operationElapsedMs", "maxElapsedRatio"),
        ("firstUsableArtifactMs", "maxFirstUsableRatio"),
    ):
        before, after = results["baseline"][metric], results["candidate"][metric]
        ratio = None
        if before is not None and after is not None:
            ratio = after / before if before else (1.0 if after == 0 else None)
            if after > limits[limit_name] * before:
                issue(issues, f"{metric}_limit_exceeded", rejected=True)
        else:
            issue(issues, f"{metric}_not_comparable")
        ratios[metric] = ratio
    return {"id": case["id"], "kind": case["kind"], "regime": case["regime"], "issues": issues,
            "baseline": results["baseline"], "candidate": results["candidate"],
            "ratios": ratios}


def evaluate(manifest_path: Path) -> dict[str, Any]:
    """Return comparisons; raise a content-free EvidenceError for invalid evidence."""
    path = manifest_path.expanduser().resolve(strict=True)
    manifest = read_json(path)
    require(isinstance(manifest, dict) and type(manifest.get("schemaVersion")) is int
            and manifest["schemaVersion"] == 1, "manifest_version_invalid")
    baseline, candidate = manifest.get("baselineSha"), manifest.get("candidateSha")
    require(hexadecimal(baseline, 40) and hexadecimal(candidate, 40) and baseline != candidate,
            "distinct_source_heads_required")
    limits = manifest.get("limits")
    require(isinstance(limits, dict) and all(positive(limits.get(name)) for name in
            ("maxInputOutputTokenRatio", "maxRecoveryInputOutputTokenRatio",
             "maxElapsedRatio", "maxFirstUsableRatio")),
            "explicit_comparison_limits_required")
    cases = manifest.get("cases")
    require(isinstance(cases, list) and 0 < len(cases) <= MAX_CASES, "case_count_invalid")
    results = []
    seen_ids: set[str] = set()
    seen_usage: set[str] = set()
    for case in cases:
        result = evaluate_case(path.parent, case, baseline, candidate, limits)
        require(result["id"] not in seen_ids, "duplicate_case_identity")
        seen_ids.add(result["id"])
        for side in ("baseline", "candidate"):
            snapshot = case[side]["usage"]["sha256"]
            require(snapshot not in seen_usage, "usage_snapshot_reused_across_cases")
            seen_usage.add(snapshot)
        results.append(result)
    missing_kinds = sorted(KINDS - {result["kind"] for result in results})
    missing_regimes = sorted(REGIMES - {result["regime"] for result in results})
    all_issues = [item for result in results for item in result["issues"]]
    verdict = ("rejected" if any(item["state"] == "rejected" for item in all_issues)
               else "incomplete" if missing_kinds or missing_regimes or all_issues else "accepted")
    return {"schemaVersion": 1, "verdict": verdict, "manifestIdentitySha256": identity(manifest),
            "baselineSha": baseline,
            "candidateSha": candidate, "limits": limits, "missingCaseKinds": missing_kinds,
            "missingGenerationRegimes": missing_regimes,
            "cases": results,
            "notes": [
                "Verdict validates submitted measurements and review attestations; it does not independently prove build provenance, factual entailment or reviewer accuracy.",
                "Every required case must meet both quality and explicit cost/latency limits; savings on one case cannot hide a regression on another.",
                "Token comparisons sum provider-reported input and output once. Recovery-token cost is calculated separately from repeated attempts and versioned allowed native recovery lineage; reasoning and cached input remain subsets, not additional tokens. Token reduction is not a measured monetary-billing reduction.",
                "Completion latency uses the recorded operation duration; first-usable latency uses hash-bound artifact availability observations, never provider response times. Neither sums overlapping request intervals. Accounting completeness, availability, cache state, fresh/resume regime and head identity are explicit operator attestations.",
                "Synthetic runs cannot qualify live-provider acceptance. No provider, library or artifact was modified by this comparison.",
            ]}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path, help="Hash-bound AC-06 evidence manifest")
    args = parser.parse_args()
    try:
        result = evaluate(args.manifest)
    except (EvidenceError, OSError, ValueError, TypeError, OverflowError, RecursionError) as error:
        result = {"schemaVersion": 1, "verdict": "invalid",
                  "errorCode": str(error) if isinstance(error, EvidenceError) else "evidence_invalid"}
    print(json.dumps(result, indent=2, sort_keys=True, allow_nan=False))
    return {"accepted": 0, "rejected": 1, "incomplete": 2, "invalid": 2}[result["verdict"]]


if __name__ == "__main__":
    sys.exit(main())
