#!/usr/bin/env python3
"""Read-only, content-free generation usage report (Python standard library).

Percentages use provider-reported input/output totals independently. Missing
usage is never imputed as zero. Reasoning is a subset of output, and cache reads
are a subset of input; neither is added to total tokens a second time.
"""
from __future__ import annotations

import argparse
import importlib.util
from collections import defaultdict
import json
import os
from pathlib import Path
import sqlite3
import sys
from typing import Any

FIELDS = ("inputTokens", "outputTokens", "reasoningTokens", "cachedInputTokens",
          "cacheWriteTokens", "totalTokens", "cost")
APP_IDENTIFIER = "io.github.jferments.ostudy"

# Resolve the maintained sibling independently of the caller's current directory
# and of importlib-based diagnostic/test consumers, without changing sys.path.
_OUTCOME_SPEC = importlib.util.spec_from_file_location(
    "ostudy_generation_outcomes", Path(__file__).with_name("generation_outcomes.py"))
_OUTCOMES = importlib.util.module_from_spec(_OUTCOME_SPEC)
_OUTCOME_SPEC.loader.exec_module(_OUTCOMES)


def default_database_path() -> Path:
    explicit = os.environ.get("OSTUDY_APP_DATA_DIR", "").strip()
    if explicit:
        app_data = Path(explicit).expanduser()
    else:
        data_home = os.environ.get("XDG_DATA_HOME", "").strip()
        root = Path(data_home).expanduser() if data_home else Path.home() / ".local" / "share"
        app_data = root / APP_IDENTIFIER
    return app_data / "library.sqlite3"


INPUT_PAYLOAD_FIELDS = {
    "systemInstructions": ("systemInstructionBytes",),
    "embeddedSchema": ("embeddedSchemaBytes",),
    "wireSchema": ("wireSchemaBytes",),
    "knowledgeUnits": ("userParts", "knowledgeUnits"),
    "groundingEvidence": ("userParts", "groundingEvidence"),
    "structuralEvidence": ("userParts", "structuralEvidence"),
    "sourceEvidence": ("userParts", "sourceEvidence"),
    "sourceWindow": ("userParts", "sourceWindow"),
    "collectionDigest": ("userParts", "collectionDigest"),
    "learningPlan": ("userParts", "learningPlan"),
    "outline": ("userParts", "outline"),
    "repairDraft": ("userParts", "repairDraft"),
    "repairTargets": ("userParts", "repairTargets"),
    "embeddingInput": ("userParts", "embeddingInput"),
    "otherUserText": ("userParts", "otherUserText"),
    "imageUrls": ("imageUrlBytes",),
}


def percentage(numerator: int | float, denominator: int | float) -> float | None:
    return 100.0 * numerator / denominator if denominator else None


def valid_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and value >= 0


def interval_union_ms(rows: list[dict[str, Any]]) -> int:
    intervals = sorted(
        (int(row["startedAt"]), int(row["finishedAt"]))
        for row in rows
        if valid_number(row.get("startedAt"))
        and valid_number(row.get("finishedAt"))
        and row["finishedAt"] >= row["startedAt"]
    )
    total = 0
    current_start: int | None = None
    current_end: int | None = None
    for start, end in intervals:
        if current_start is None:
            current_start, current_end = start, end
        elif start <= current_end:
            current_end = max(current_end, end)
        else:
            total += current_end - current_start
            current_start, current_end = start, end
    if current_start is not None and current_end is not None:
        total += current_end - current_start
    return total


def summarize(data: dict[str, Any]) -> dict[str, Any]:
    requests = data.get("requests", [])
    if not isinstance(requests, list):
        raise ValueError("requests must be an array")
    stages: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in requests:
        if not isinstance(row, dict) or not isinstance(row.get("stage"), str):
            raise ValueError("each request requires a stage")
        stages[row["stage"]].append(row)

    def totals(rows: list[dict[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {"requests": len(rows)}
        for field in FIELDS:
            known = [row[field] for row in rows if valid_number(row.get(field))]
            result[field] = sum(known) if known else (0 if not rows else None)
            result[field + "MissingRequests"] = len(rows) - len(known)
        result["failedOrInterruptedRequests"] = sum(
            row.get("status") in ("failed", "interrupted", "started") for row in rows)
        repeated = [row for row in rows if row.get("attempt", 1) > 1]
        result["repeatedLogicalRequests"] = len(repeated)
        result["repeatReportedInputTokens"] = sum(
            row["inputTokens"] for row in repeated if valid_number(row.get("inputTokens")))
        result["repeatReportedOutputTokens"] = sum(
            row["outputTokens"] for row in repeated if valid_number(row.get("outputTokens")))
        completed_durations = [
            row["finishedAt"] - row["startedAt"] for row in rows
            if valid_number(row.get("startedAt"))
            and valid_number(row.get("finishedAt"))
            and row["finishedAt"] >= row["startedAt"]
        ]
        repeat_durations = [
            row["finishedAt"] - row["startedAt"] for row in repeated
            if valid_number(row.get("startedAt"))
            and valid_number(row.get("finishedAt"))
            and row["finishedAt"] >= row["startedAt"]
        ]
        result["cumulativeRequestMs"] = sum(completed_durations)
        result["requestDurationCoverageRequests"] = len(completed_durations)
        result["repeatCumulativeRequestMs"] = sum(repeat_durations)
        result["repeatDurationCoverageRequests"] = len(repeat_durations)
        result["requestActiveWallClockMs"] = interval_union_ms(rows)
        result["repeatActiveWallClockMs"] = interval_union_ms(repeated)
        cache_pairs = [row for row in rows if valid_number(row.get("inputTokens"))
                       and valid_number(row.get("cachedInputTokens"))]
        result["cacheReadPercentOfCoveredInput"] = percentage(
            sum(row["cachedInputTokens"] for row in cache_pairs),
            sum(row["inputTokens"] for row in cache_pairs))
        result["cacheReadCoverageRequests"] = len(cache_pairs)
        reasoning_pairs = [row for row in rows if valid_number(row.get("outputTokens"))
                           and valid_number(row.get("reasoningTokens"))]
        result["reasoningPercentOfCoveredOutput"] = percentage(
            sum(row["reasoningTokens"] for row in reasoning_pairs),
            sum(row["outputTokens"] for row in reasoning_pairs))
        result["reasoningCoverageRequests"] = len(reasoning_pairs)
        result["inconsistentUsageRequests"] = sum(
            (valid_number(row.get("cachedInputTokens"))
             and valid_number(row.get("inputTokens"))
             and row["cachedInputTokens"] > row["inputTokens"])
            or (valid_number(row.get("reasoningTokens"))
                and valid_number(row.get("outputTokens"))
                and row["reasoningTokens"] > row["outputTokens"])
            for row in rows)
        result["repeatInputPercentOfReportedInput"] = percentage(
            result["repeatReportedInputTokens"],
            result["inputTokens"] or 0)
        result["repeatOutputPercentOfReportedOutput"] = percentage(
            result["repeatReportedOutputTokens"],
            result["outputTokens"] or 0)
        result["repeatDurationPercentOfCumulativeRequestMs"] = percentage(
            result["repeatCumulativeRequestMs"],
            result["cumulativeRequestMs"])
        return result

    overall = totals(requests)
    results = []
    for stage, rows in sorted(stages.items()):
        values = totals(rows)
        values["stage"] = stage
        values["percentOfReportedInput"] = (
            None if values["inputTokens"] is None else
            percentage(values["inputTokens"], overall["inputTokens"] or 0))
        values["percentOfReportedOutput"] = (
            None if values["outputTokens"] is None else
            percentage(values["outputTokens"], overall["outputTokens"] or 0))
        results.append(values)

    def payload_summary(rows: list[dict[str, Any]]) -> dict[str, Any]:
        totals_by_part = {name: 0 for name in INPUT_PAYLOAD_FIELDS}
        serialized_request_bytes = 0
        visible_response_bytes = 0
        coverage = 0
        comparable_prefix_requests = 0
        common_prefix_bytes = 0
        bounded_prefix_requests = 0
        for row in rows:
            payload = row.get("payloadBytesJson")
            if isinstance(payload, str):
                try:
                    payload = json.loads(payload)
                except json.JSONDecodeError:
                    payload = None
            if not isinstance(payload, dict):
                continue
            coverage += 1
            serialized = payload.get("serializedRequestBytes")
            if valid_number(serialized):
                serialized_request_bytes += serialized
            visible = payload.get("response", {}).get("visibleContentBytes")
            if valid_number(visible):
                visible_response_bytes += visible
            cache_prefix = payload.get("cachePrefix")
            if isinstance(cache_prefix, dict) and cache_prefix.get("comparable") is True:
                common = cache_prefix.get("commonPrefixBytes")
                if valid_number(common):
                    comparable_prefix_requests += 1
                    common_prefix_bytes += int(common)
                    if cache_prefix.get("measurementBounded") is True:
                        bounded_prefix_requests += 1
            for name, path in INPUT_PAYLOAD_FIELDS.items():
                value: Any = payload
                for part in path:
                    value = value.get(part) if isinstance(value, dict) else None
                if valid_number(value):
                    totals_by_part[name] += value
        return {
            "coverageRequests": coverage,
            "serializedRequestBytes": serialized_request_bytes,
            "inputParts": {
                name: {
                    "bytes": value,
                    "percentOfSerializedRequestBytes": percentage(
                        value, serialized_request_bytes),
                }
                for name, value in totals_by_part.items()
            },
            "visibleResponseBytes": visible_response_bytes,
            "cachePrefixDiagnostics": {
                "comparableRequests": comparable_prefix_requests,
                "commonPrefixBytes": common_prefix_bytes,
                "averageCommonPrefixBytes": (
                    common_prefix_bytes / comparable_prefix_requests
                    if comparable_prefix_requests else None
                ),
                "boundedMeasurements": bounded_prefix_requests,
            },
        }

    payload_diagnostics = payload_summary(requests)
    for values in results:
        values["payloadByteDiagnostics"] = payload_summary(stages[values["stage"]])

    def request_summary(row: dict[str, Any]) -> dict[str, Any]:
        payload = row.get("payloadBytesJson")
        if isinstance(payload, str):
            try:
                payload = json.loads(payload)
            except json.JSONDecodeError:
                payload = None
        cache_prefix = payload.get("cachePrefix") if isinstance(payload, dict) else None
        return {
            "requestId": row.get("requestId"),
            "operationId": row.get("operationId"),
            "stage": row.get("stage"),
            "attempt": row.get("attempt"),
            "requestKey": row.get("requestKey"),
            "status": row.get("status"),
            "inputTokens": row.get("inputTokens"),
            "outputTokens": row.get("outputTokens"),
            "cachedInputTokens": row.get("cachedInputTokens"),
            "reasoningTokens": row.get("reasoningTokens"),
            "cost": row.get("cost"),
            "serializedRequestBytes": (
                payload.get("serializedRequestBytes")
                if isinstance(payload, dict) else None),
            "commonPrefixBytes": (
                cache_prefix.get("commonPrefixBytes")
                if isinstance(cache_prefix, dict) and cache_prefix.get("comparable") is True
                else None),
            "commonPrefixMeasurementBounded": (
                cache_prefix.get("measurementBounded")
                if isinstance(cache_prefix, dict) and cache_prefix.get("comparable") is True
                else None),
        }

    largest_input = [
        request_summary(row)
        for row in sorted(
            (row for row in requests if valid_number(row.get("inputTokens"))),
            key=lambda row: row["inputTokens"],
            reverse=True,
        )[:20]
    ]
    largest_output = [
        request_summary(row)
        for row in sorted(
            (row for row in requests if valid_number(row.get("outputTokens"))),
            key=lambda row: row["outputTokens"],
            reverse=True,
        )[:20]
    ]

    operations: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in requests:
        operation_id = row.get("operationId")
        if isinstance(operation_id, str):
            operations[operation_id].append(row)
    operation_results = []
    for operation_id, rows in sorted(operations.items()):
        values = totals(rows)
        values["operationId"] = operation_id
        values["percentOfReportedInput"] = (
            None if values["inputTokens"] is None else
            percentage(values["inputTokens"], overall["inputTokens"] or 0))
        values["percentOfReportedOutput"] = (
            None if values["outputTokens"] is None else
            percentage(values["outputTokens"], overall["outputTokens"] or 0))
        values["payloadByteDiagnostics"] = payload_summary(rows)
        operation_results.append(values)

    operation = data.get("operation")
    wall_clock_ms = None
    if isinstance(operation, dict):
        started = operation.get("startedAt")
        finished = operation.get("finishedAt")
        if valid_number(started) and valid_number(finished) and finished >= started:
            wall_clock_ms = finished - started

    return {
        "schemaVersion": 1,
        "operationId": data.get("operationId"),
        "operation": operation,
        "wallClockMs": wall_clock_ms,
        "repeatActivePercentOfOperationWallClock": percentage(
            overall["repeatActiveWallClockMs"], wall_clock_ms or 0),
        "totals": overall,
        "stages": results,
        "operations": operation_results,
        "localReuse": data.get("localReuse", []),
        "nativeOutcomes": _OUTCOMES.summarize(requests),
        "payloadByteDiagnostics": payload_diagnostics,
        "largestReportedInputRequests": largest_input,
        "largestReportedOutputRequests": largest_output,
        "usageComplete": not overall["inputTokensMissingRequests"]
                         and not overall["outputTokensMissingRequests"],
        "notes": [
            "Stage percentages are shares of REPORTED input and output, separately.",
            "Missing/failed/interrupted usage is unknown, not zero. This is not a complete bill when usage is missing.",
            "Reasoning is contained in output; cached input is contained in input. Cache and reasoning rates use only requests reporting both fields.",
            "Local reuse means no provider request. Provider prompt caching still incurs a request.",
            "Common-prefix bytes are content-free local measurements between comparable serialized requests; bounded measurements reached the in-memory 2 MiB sample cap. Provider cached-token counts remain authoritative for actual cache use.",
            "Payload byte categories are NON-BILLING diagnostics. Input-part percentages use serialized request bytes; visible response bytes are reported separately and are never substituted for provider-reported token shares.",
            "Attempt ordinals group the same logical section, or identical wire requests when no section identity is available.",
            "Cumulative request duration sums provider-request intervals and therefore double-counts concurrent requests. Active wall-clock duration is the union of those intervals. Time occupied by repeated attempts is observable overhead, not a claim that eliminating them would save the same wall-clock time because sibling work may overlap.",
        ],
        "requests": requests,
    }


def read_database(path: Path, operation: str | None, all_operations: bool) -> dict[str, Any]:
    path = path.expanduser().resolve(strict=True)
    uri = path.as_uri() + "?mode=ro"
    with sqlite3.connect(uri, uri=True, timeout=5) as connection:
        connection.execute("PRAGMA query_only=ON")
        connection.execute("BEGIN")
        connection.row_factory = sqlite3.Row
        if not all_operations and operation is None:
            row = connection.execute(
                "SELECT id FROM generation_operations ORDER BY started_at DESC,id DESC LIMIT 1").fetchone()
            operation = row[0] if row else None
        if not all_operations and operation is None:
            return {"operationId": None, "operation": None, "requests": [], "localReuse": []}
        operation_row = None
        if not all_operations:
            operation_row = connection.execute("""
                SELECT id,purpose,state,started_at AS startedAt,finished_at AS finishedAt
                FROM generation_operations WHERE id=?
            """, (operation,)).fetchone()
        where = "" if all_operations else "WHERE operation_id=?"
        args = () if all_operations else (operation,)
        rows = connection.execute(f"""
            SELECT id AS requestId,operation_id AS operationId,stage,attempt,provider,model,status,
                   input_tokens AS inputTokens,output_tokens AS outputTokens,
                   reasoning_tokens AS reasoningTokens,cached_input_tokens AS cachedInputTokens,
                   cache_write_tokens AS cacheWriteTokens,total_tokens AS totalTokens,cost,
                   response_id AS responseId,finish_reason AS finishReason,error_code AS errorCode,
                   request_key AS requestKey,
                   payload_bytes_json AS payloadBytesJson,max_output_tokens AS maxOutputTokens,
                   started_at AS startedAt,finished_at AS finishedAt
            FROM generation_requests {where} ORDER BY started_at,id
        """, args).fetchall()
        reuse = connection.execute(f"""
            SELECT operation_id AS operationId,stage,COUNT(*) AS count
            FROM generation_reuse_events {where}
            GROUP BY operation_id,stage ORDER BY operation_id,stage
        """, args).fetchall()
        return {"operationId": "all" if all_operations else operation,
                "operation": dict(operation_row) if operation_row is not None else None,
                "requests": [dict(row) for row in rows],
                "localReuse": [dict(row) for row in reuse]}


def list_operations(path: Path) -> list[dict[str, Any]]:
    uri = path.expanduser().resolve(strict=True).as_uri() + "?mode=ro"
    with sqlite3.connect(uri, uri=True, timeout=5) as connection:
        connection.execute("PRAGMA query_only=ON")
        connection.row_factory = sqlite3.Row
        return [dict(row) for row in connection.execute("""
            SELECT id,purpose,state,started_at AS startedAt,finished_at AS finishedAt
            FROM generation_operations ORDER BY started_at DESC,id DESC LIMIT 50
        """)]


def render(report: dict[str, Any]) -> str:
    def number(value: Any) -> str:
        return "unknown" if value is None else f"{value:,.0f}"

    def percent(value: Any) -> str:
        return "n/a" if value is None else f"{value:.2f}%"

    wall_clock = report.get("wallClockMs")
    wall_clock_label = "unknown" if wall_clock is None else f"{wall_clock / 1000:.2f}s"
    lines = [f"Generation operation: {report['operationId']}",
             f"Operation wall clock: {wall_clock_label}",
             "Stage                         Requests       Input    Input %      Output   Output %"]
    for row in report["stages"]:
        lines.append(f"{row['stage']:<29} {row['requests']:>8} "
                     f"{number(row['inputTokens']):>11} {percent(row['percentOfReportedInput']):>10} "
                     f"{number(row['outputTokens']):>11} {percent(row['percentOfReportedOutput']):>10}")
    total = report["totals"]
    lines.extend(["", f"Reported input: {number(total['inputTokens'])}; output: {number(total['outputTokens'])}",
                  f"Requests missing input/output usage: {total['inputTokensMissingRequests']}/{total['outputTokensMissingRequests']}",
                  f"Failed, interrupted or unfinished requests: {total['failedOrInterruptedRequests']}",
                  f"Repeated logical requests: {total['repeatedLogicalRequests']} "
                  f"(input share {percent(total['repeatInputPercentOfReportedInput'])}; "
                  f"output share {percent(total['repeatOutputPercentOfReportedOutput'])})",
                  f"Repeated-attempt provider time: cumulative {total['repeatCumulativeRequestMs'] / 1000:.2f}s "
                  f"({percent(total['repeatDurationPercentOfCumulativeRequestMs'])} of covered request time); "
                  f"active wall-clock {total['repeatActiveWallClockMs'] / 1000:.2f}s "
                  f"({percent(report['repeatActivePercentOfOperationWallClock'])} of operation wall clock)",
                  f"Cache reads: {number(total['cachedInputTokens'])} tokens; covered-input rate: {percent(total['cacheReadPercentOfCoveredInput'])} ({total['cacheReadCoverageRequests']} requests)",
                  f"Comparable request prefixes: {report['payloadByteDiagnostics']['cachePrefixDiagnostics']['comparableRequests']}; "
                  f"average common prefix: {number(report['payloadByteDiagnostics']['cachePrefixDiagnostics']['averageCommonPrefixBytes'])} bytes; "
                  f"bounded measurements: {report['payloadByteDiagnostics']['cachePrefixDiagnostics']['boundedMeasurements']}",
                  f"Reasoning: {number(total['reasoningTokens'])} tokens; covered-output share: {percent(total['reasoningPercentOfCoveredOutput'])} ({total['reasoningCoverageRequests']} requests)",
                  f"Local reuse: {json.dumps(report['localReuse'], ensure_ascii=False)}",
                  f"Payload byte diagnostics: "
                  f"{json.dumps(report['payloadByteDiagnostics'], ensure_ascii=False)}", "",
                  "Usage complete: " + ("yes" if report["usageComplete"] else "NO; percentages cover reported usage only"),
                  *report["notes"]])
    lines.extend(_OUTCOMES.render(report["nativeOutcomes"]))
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group()
    source.add_argument(
        "--database",
        type=Path,
        help="Existing library.sqlite3; defaults to the active oStudy library",
    )
    source.add_argument("--input-json", type=Path, help="Content-free native diagnostic report")
    parser.add_argument("--operation", help="Operation ID; defaults to latest")
    parser.add_argument("--all", action="store_true", help="Aggregate all retained operations")
    parser.add_argument("--list", action="store_true", help="List the 50 latest operations")
    parser.add_argument("--json", action="store_true", help="Machine-readable report including request records")
    args = parser.parse_args()
    try:
        if args.operation and args.all:
            parser.error("--operation and --all cannot be combined")
        database = args.database or default_database_path()
        if args.list:
            if args.input_json is not None:
                parser.error("--list cannot be combined with --input-json")
            print(json.dumps(list_operations(database), indent=2))
            return 0
        if args.input_json:
            data = json.loads(args.input_json.read_text(encoding="utf-8"))
        else:
            data = read_database(database, args.operation, args.all)
        report = summarize(data)
        print(json.dumps(report, indent=2, allow_nan=False) if args.json else render(report))
        return 0
    except (OSError, sqlite3.Error, ValueError, KeyError, TypeError) as error:
        print(f"Could not read generation diagnostics: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
