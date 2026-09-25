"""Read-only operation outcomes, independent of physical request observations."""
from __future__ import annotations

from collections import Counter
import json
import re
import sqlite3
from typing import Any

_CODE = re.compile(r"[a-z][a-z0-9_]{0,79}\Z")
_STATES = ("running", "complete", "failed", "interrupted")


def read_records(connection: sqlite3.Connection, operation: str | None,
                 all_operations: bool) -> list[dict[str, Any]]:
    """Use the caller's existing read transaction, including legacy schema 27-29."""
    if not all_operations and operation is None:
        return []
    columns = {row[1] for row in connection.execute("PRAGMA table_info(generation_operations)")}
    outcome = "outcome_json" if "outcome_json" in columns else "NULL"
    where = "" if all_operations else "WHERE id=?"
    cursor = connection.execute(f"""
        SELECT id,purpose,state,started_at AS startedAt,finished_at AS finishedAt,
               {outcome} AS outcomeJson
        FROM generation_operations {where} ORDER BY started_at,id
    """, () if all_operations else (operation,))
    names = [column[0] for column in cursor.description]
    return [dict(zip(names, row)) for row in cursor.fetchall()]


def _code(value: Any) -> str | None:
    return value if isinstance(value, str) and _CODE.fullmatch(value) else None


def _outcome(value: Any, state: Any) -> dict[str, Any] | None:
    if isinstance(value, str):
        if len(value) > 1024:
            return None
        try:
            value = json.loads(value)
        except (ValueError, RecursionError):
            return None
    if (not isinstance(value, dict) or type(value.get("schemaVersion")) is not int
            or value["schemaVersion"] != 1 or "errorCode" not in value
            or value.get("state") != state
            or value.get("artifactPublication") != "not_observed"):
        return None
    expected_return = state in ("complete", "failed")
    if state not in ("complete", "failed", "interrupted") or value.get("resultObserved") is not expected_return:
        return None
    error = value.get("errorCode")
    if (state == "complete" and error is not None) or (state != "complete" and _code(error) is None):
        return None
    return value


def summarize(records: Any, requests: list[dict[str, Any]]) -> dict[str, Any]:
    if records is None:
        return {"schemaVersion": 1, "recordsAvailable": False, "observedOperations": 0,
                "unobservedOperations": None, "recordIssues": 0, "operations": []}
    if not isinstance(records, list):
        raise ValueError("operationRecords must be an array or null")
    ids = Counter(row.get("id") for row in records
                  if isinstance(row, dict) and isinstance(row.get("id"), str))
    request_counts = Counter(row.get("operationId") for row in requests
                             if isinstance(row.get("operationId"), str))
    issues = 0
    observed = 0
    rows = []
    for record in records:
        if not isinstance(record, dict) or not isinstance(record.get("id"), str):
            issues += 1
            continue
        identity = record["id"]
        state = record.get("state")
        if state not in _STATES:
            state = "unobserved"
        value = _outcome(record.get("outcomeJson"), state)
        if ids[identity] != 1:
            value = None
            issues += 1
        elif record.get("outcomeJson") is not None and value is None:
            issues += 1
        if value is not None:
            observed += 1
        rows.append({
            "operationId": identity,
            "purpose": _code(record.get("purpose")) or "unclassified",
            "state": state,
            "outcomeObserved": value is not None,
            "resultObserved": value["resultObserved"] if value is not None else None,
            "errorCode": value["errorCode"] if value is not None else None,
            "recordedRequests": request_counts[identity],
            "artifactPublication": "not_observed",
        })
    return {"schemaVersion": 1, "recordsAvailable": True,
            "observedOperations": observed, "unobservedOperations": len(records) - observed,
            "recordIssues": issues, "operations": rows}


def render(report: dict[str, Any]) -> list[str]:
    lines = ["", "Operation results (independent of provider requests):"]
    if not report["recordsAvailable"]:
        lines.append("Operation records were not included; native outcomes are unobserved.")
        return lines
    for row in report["operations"]:
        cause = row["errorCode"] or ("none" if row["outcomeObserved"] else "unobserved")
        lines.append(f"{row['operationId']} {row['purpose']}: {row['state']}; "
                     f"cause={cause}; recorded requests={row['recordedRequests']}")
    if report["recordIssues"]:
        lines.append(f"Malformed, unsupported or ambiguous operation records: {report['recordIssues']}")
    lines.append("Operation results do not certify artifact publication or model quality. "
                 "Zero recorded requests is not proof of zero provider activity when accounting is incomplete.")
    return lines
