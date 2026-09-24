"""Read-only projections of versioned native outcomes; never infer model quality.

Provider response status, committed extraction progress and returned authoring
items are independent facts. Absence of a native record remains unobserved.
"""
from __future__ import annotations

from collections import Counter, defaultdict
import json
import math
import re
from typing import Any

REASONS = ("contract_repair", "output_compaction", "transient_transport", "provider_compatibility")
STATES = ("running", "continued", "returned", "failed", "interrupted", "linkage_lost")
ITEMS = ("cards", "guide_sections", "curation_selections")
CODE = re.compile(r"[a-z][a-z0-9_]{0,79}\Z")
COUNTS = ("acceptedUnits", "rejectedProposals", "unaccountedSources")


def versioned(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) and type(value.get("schemaVersion")) is int and value["schemaVersion"] == 1 else {}


def code(value: Any) -> str:
    return value if isinstance(value, str) and CODE.fullmatch(value) else "unclassified"


def count(value: Any) -> bool:
    return type(value) is int and 0 <= value <= 2**32 - 1


def payload(row: dict[str, Any]) -> dict[str, Any]:
    value = row.get("payloadBytesJson")
    if isinstance(value, str):
        try:
            value = json.loads(value)
        except (ValueError, RecursionError):
            return {}
    return value if isinstance(value, dict) else {}


def snapshot(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or not all(count(value.get(key)) for key in COUNTS):
        return {}
    regions = value.get("unresolvedRegions")
    if (regions is not None and not count(regions)) or type(value.get("protocolComplete")) is not bool:
        return {}
    return value


def checkpoint_progress(checkpoint: dict[str, Any]) -> str:
    before, after = snapshot(checkpoint.get("before")), snapshot(checkpoint.get("after"))
    if not before or not after:
        return "unobserved"
    # Compute signed deltas from the durable snapshots. Do not turn a boolean
    # claim or the provider's finish reason into a measured improvement.
    deltas = [after[key] - before[key] for key in COUNTS]
    improving = deltas[0] > 0 or any(delta < 0 for delta in deltas[1:])
    regressing = deltas[0] < 0 or any(delta > 0 for delta in deltas[1:])
    if before.get("unresolvedRegions") is not None and after.get("unresolvedRegions") is not None:
        change = after["unresolvedRegions"] - before["unresolvedRegions"]
        improving |= change < 0
        regressing |= change > 0
    if improving and regressing:
        return "mixed_progress"
    if regressing:
        return "native_regression"
    return "committed_progress" if improving else "no_measured_progress"


def usage(rows: list[dict[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for field in ("inputTokens", "outputTokens", "totalTokens", "cost"):
        known = [row[field] for row in rows
                 if type(row.get(field)) in (int, float)
                 and math.isfinite(row[field]) and row[field] >= 0]
        result[field] = sum(known) if known else (0 if not rows else None)
        result[field + "MissingRequests"] = len(rows) - len(known)
    return result


def admission_summary(requests: list[dict[str, Any]]) -> dict[str, Any]:
    """Counts concern examined candidates, not unique facts or saved cards."""
    records = []
    invalid = 0
    boundaries: Counter[tuple[str, str]] = Counter()
    rejections: Counter[tuple[str, str, bool]] = Counter()
    flashcard_requests = sum(row.get("stage") == "flashcard_authoring" for row in requests)
    for row in requests:
        data = payload(row)
        if "nativeAdmission" not in data:
            continue
        record = versioned(data["nativeAdmission"])
        fields = ("examinedCandidates", "admittedCandidates", "rejectedCandidates")
        if (row.get("stage") != "flashcard_authoring" or not record
                or record.get("state") not in ("returned", "failed")
                or record.get("persistence") != "not_observed"
                or not all(count(record.get(key)) and record[key] <= 128 for key in fields)):
            invalid += 1
            continue
        examined, admitted, rejected = (record[key] for key in fields)
        declared, selected = record.get("declaredCandidates"), record.get("selectedCandidates")
        entries = record.get("rejections")
        if (not isinstance(entries, list) or len(entries) != rejected
                or examined != admitted + rejected
                or (declared is not None and (not count(declared) or declared < examined))
                or (declared is None and examined != 0)
                or (declared is not None and not count(record.get("unexaminedCandidates")))
                or record.get("unexaminedCandidates") != (None if declared is None else declared - examined)):
            invalid += 1
            continue
        if record["state"] == "returned":
            valid_selection = (declared == examined and count(selected) and selected <= admitted
                               and count(record.get("unselectedAdmittedCandidates"))
                               and record["unselectedAdmittedCandidates"] == admitted - selected
                               and record.get("failureBoundary") is None
                               and record.get("errorCode") is None)
        else:
            valid_selection = (selected is None and record.get("unselectedAdmittedCandidates") is None)
        indices = set()
        valid_entries = True
        for entry in entries:
            if (not isinstance(entry, dict) or not count(entry.get("candidateIndex"))
                    or entry["candidateIndex"] >= examined or entry["candidateIndex"] in indices
                    or type(entry.get("retryable")) is not bool):
                valid_entries = False
                break
            indices.add(entry["candidateIndex"])
        if not valid_selection or not valid_entries:
            invalid += 1
            continue
        records.append(record)
        if record["state"] == "failed":
            boundaries[(code(record.get("failureBoundary")), code(record.get("errorCode")))] += 1
        for entry in entries:
            rejections[(code(entry.get("rule")), code(entry.get("errorCode")), entry["retryable"])] += 1
    selected_records = [record for record in records if record["state"] == "returned"]
    totals = {key: sum(record[key] for record in records) if records else None
              for key in ("examinedCandidates", "admittedCandidates", "rejectedCandidates")}
    totals.update({key: sum(record[key] for record in selected_records) if selected_records else None
                   for key in ("selectedCandidates", "unselectedAdmittedCandidates")})
    return {
        "observedRequests": len(records),
        "unobservedFlashcardRequests": flashcard_requests - len(records),
        "invalidRecords": invalid,
        "selectionObservedRequests": len(selected_records),
        "responseStates": dict(sorted(Counter(record["state"] for record in records).items())),
        **totals,
        "failureBoundaries": [
            {"boundary": boundary, "errorCode": error, "requests": number}
            for (boundary, error), number in sorted(boundaries.items())
        ],
        "rejectionObservations": [
            {"rule": rule, "errorCode": error, "retryable": retryable, "observations": number}
            for (rule, error, retryable), number in sorted(rejections.items())
        ],
        "persistence": "not_observed",
    }


def summarize(requests: list[dict[str, Any]]) -> dict[str, Any]:
    """Partition physical requests once; linked recovery is not attempt > 1.

    A resumed checkpoint can legitimately cause recovery on attempt 1. Retained
    predecessors are checked by identity, never chosen by timestamps or position.
    """
    index: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in requests:
        if isinstance(row.get("requestId"), str):
            index[row["requestId"]].append(row)
    states: Counter[str] = Counter()
    problems: Counter[str] = Counter()
    failures: Counter[tuple[str, str]] = Counter()
    rules: Counter[tuple[str, str]] = Counter()
    groups: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    returned: dict[tuple[str, str], list[int]] = defaultdict(list)
    decisions: Counter[tuple[str, str, bool]] = Counter()
    observed = 0
    unclassified_repeats = 0
    checkpoints = 0
    native_keys = ("nativeTask", "nativeLineage", "nativeCheckpoint", "nativeAuthoring")

    for row in requests:
        stage = code(row.get("stage"))
        data = payload(row)
        task = versioned(data.get("nativeTask"))
        state = task.get("state")
        if not isinstance(state, str) or state not in STATES:
            state = "unobserved"
        states[state] += 1
        if state != "unobserved":
            observed += 1
        if any(key in data and not versioned(data[key]) for key in native_keys):
            problems["unsupported_or_malformed_record"] += 1
        if state in ("failed", "interrupted", "linkage_lost"):
            failures[(stage, code(task.get("errorCode")))] += 1

        checkpoint = versioned(data.get("nativeCheckpoint"))
        after = snapshot(checkpoint.get("after"))
        if after:
            checkpoints += 1
            histogram = after.get("rejectionRules")
            if isinstance(histogram, dict):
                for rule, number in histogram.items():
                    if count(number):
                        rules[(stage, code(rule))] += number
        authoring = versioned(data.get("nativeAuthoring"))
        item_kind = authoring.get("itemKind")
        if (authoring.get("state") == "returned" and isinstance(item_kind, str)
                and item_kind in ITEMS and count(authoring.get("returnedItems"))
                and authoring.get("persistence") == "not_observed"):
            returned[(stage, item_kind)].append(authoring["returnedItems"])

        recorded = data.get("nativeRecoveryDecisions")
        if isinstance(recorded, dict):
            for reason in REASONS:
                for allowed, suffix in ((True, "allowed"), (False, "denied")):
                    decision = versioned(recorded.get(f"{reason}_{suffix}"))
                    if decision.get("reason") == reason and decision.get("allowed") is allowed:
                        decisions[(stage, reason, allowed)] += 1

        lineage = versioned(data.get("nativeLineage"))
        recovery = versioned(lineage.get("recovery"))
        reason = recovery.get("reason")
        valid_recovery = (isinstance(reason, str) and reason in REASONS
                          and recovery.get("allowed") is True)
        issue = None
        if valid_recovery:
            previous = lineage.get("previousRequestId")
            trigger = recovery.get("triggerRequestId")
            if lineage.get("linkageIncomplete") is not False:
                issue = "incomplete_linkage"
            elif previous != trigger or (previous is not None and not isinstance(previous, str)):
                issue = "invalid_predecessor"
            elif isinstance(previous, str):
                matches = index.get(previous, [])
                if len(matches) != 1:
                    issue = "missing_or_duplicate_predecessor"
                elif (previous == row.get("requestId") or any(
                        not isinstance(row.get(key), str) or row[key] != matches[0].get(key)
                        for key in ("operationId", "stage", "requestKey"))):
                    issue = "cross_task_predecessor"
            if issue:
                problems[issue] += 1
            else:
                progress = checkpoint_progress(checkpoint)
                groups[(stage, reason, progress)].append(row)
        if (not valid_recovery or issue) and count(row.get("attempt")) and row["attempt"] > 1:
            unclassified_repeats += 1

    return {
        "schemaVersion": 1,
        "requests": len(requests),
        "observedRequests": observed,
        "unobservedRequests": len(requests) - observed,
        "requestStates": dict(sorted(states.items())),
        "checkpointRequests": checkpoints,
        "candidateAdmission": admission_summary(requests),
        "unclassifiedRepeatedRequests": unclassified_repeats,
        "linkageOrRecordIssues": dict(sorted(problems.items())),
        "recoveryRequests": [
            {"stage": stage, "reason": reason, "checkpointOutcome": progress,
             "requests": len(rows),
             "nativeReturnedRequests": sum(versioned(payload(row).get("nativeTask")).get("state") == "returned" for row in rows),
             "nativeFailedRequests": sum(versioned(payload(row).get("nativeTask")).get("state") == "failed" for row in rows),
             "nativeInterruptedRequests": sum(versioned(payload(row).get("nativeTask")).get("state") in ("interrupted", "linkage_lost") for row in rows),
             **usage(rows)}
            for (stage, reason, progress), rows in sorted(groups.items())
        ],
        "recordedDecisions": [
            {"stage": stage, "reason": reason, "allowed": allowed, "observations": number}
            for (stage, reason, allowed), number in sorted(decisions.items())
        ],
        "nativeFailures": [
            {"stage": stage, "errorCode": error, "requests": number}
            for (stage, error), number in sorted(failures.items())
        ],
        "checkpointRejectionObservations": [
            {"stage": stage, "rule": rule, "observations": number}
            for (stage, rule), number in sorted(rules.items())
        ],
        "authoringReturns": [
            {"stage": stage, "itemKind": item_kind, "returnedRequests": len(counts),
             "emptyReturns": counts.count(0), "returnedItems": sum(counts),
             "persistence": "not_observed"}
            for (stage, item_kind), counts in sorted(returned.items())
        ],
        "notes": [
            "Native outcomes are recorded observations, not inferred from provider status or attempt numbers. Legacy or unsupported records remain unobserved.",
            "Checkpoint progress compares committed native counts; it does not certify factual accuracy, exhaustive coverage or teaching quality. Mixed progress preserves regressions.",
            "Recovery usage counts only each linked follow-up request, not its predecessor. Missing usage stays unknown. These costs are observations, not predicted savings.",
            "Rejection and decision observations are not unique rejected proposals or unique tasks; repeated snapshots can contain the same pending proposal.",
            "Authoring returns (including valid empty curation selections) do not establish saved decks or guides. Artifact publication and pre-request operation failures are not observed here.",
        ],
    }


def render(report: dict[str, Any]) -> list[str]:
    def number(value: Any) -> str:
        return "unknown" if value is None else f"{value:,.0f}"

    lines = ["", "Native generation outcomes",
             f"Observed requests: {report['observedRequests']}; unobserved: {report['unobservedRequests']}",
             "Request states: " + json.dumps(report["requestStates"], sort_keys=True),
             f"Repeated requests without trustworthy recovery linkage: {report['unclassifiedRepeatedRequests']}"]
    for row in report["recoveryRequests"]:
        lines.append(f"  {row['stage']} / {row['reason']} / {row['checkpointOutcome']}: "
                     f"{row['requests']} requests; reported input {number(row['inputTokens'])}, "
                     f"output {number(row['outputTokens'])}; missing input/output "
                     f"{row['inputTokensMissingRequests']}/{row['outputTokensMissingRequests']}; "
                     f"native returned/failed/interrupted {row['nativeReturnedRequests']}/"
                     f"{row['nativeFailedRequests']}/{row['nativeInterruptedRequests']}")
    for row in report["recordedDecisions"]:
        disposition = "allowed" if row["allowed"] else "denied"
        lines.append(f"  Recovery decision: {row['stage']} / {row['reason']} / {disposition}: {row['observations']} observations")
    for row in report["nativeFailures"]:
        lines.append(f"  Native failure: {row['stage']} / {row['errorCode']}: {row['requests']} requests")
    for row in report["authoringReturns"]:
        lines.append(f"  Authoring returned: {row['returnedItems']} {row['itemKind']} across "
                     f"{row['returnedRequests']} requests ({row['emptyReturns']} empty); persistence not observed")
    for row in sorted(report["checkpointRejectionObservations"], key=lambda row: (-row["observations"], row["stage"], row["rule"]))[:20]:
        lines.append(f"  Rejection observation: {row['stage']} / {row['rule']}: {row['observations']}")
    admission = report.get("candidateAdmission", {})
    if admission:
        lines.append(f"Candidate admission: {admission['observedRequests']} observed responses; "
                     f"{admission['unobservedFlashcardRequests']} unobserved; "
                     f"examined {number(admission['examinedCandidates'])}, "
                     f"admitted {number(admission['admittedCandidates'])}, "
                     f"rejected {number(admission['rejectedCandidates'])}")
        lines.append(f"  Selected {number(admission['selectedCandidates'])}; "
                     f"admitted but not selected {number(admission['unselectedAdmittedCandidates'])} "
                     f"across {admission['selectionObservedRequests']} observed selections; persistence not observed")
        for row in admission["failureBoundaries"]:
            lines.append(f"  Admission failure: {row['boundary']} / {row['errorCode']}: {row['requests']} responses")
        for row in sorted(admission["rejectionObservations"], key=lambda item: (-item["observations"], item["rule"]))[:20]:
            lines.append(f"  Candidate rejection: {row['rule']} / {row['errorCode']}: {row['observations']} observations")
        if admission["invalidRecords"]:
            lines.append(f"  Invalid or unsupported admission records: {admission['invalidRecords']}")
        lines.append("Candidate counts are observations, not unique facts or saved cards. Admission is separate from selection and does not certify factual quality. Retryable does not mean another request was sent.")
    if report["linkageOrRecordIssues"]:
        lines.append("Outcome record issues: " + json.dumps(report["linkageOrRecordIssues"], sort_keys=True))
    lines.extend(report["notes"])
    return lines
