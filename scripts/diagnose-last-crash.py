#!/usr/bin/env python3
"""Summarize the latest local oStudy crash without requiring path or operation placeholders."""

from __future__ import annotations

import argparse
from datetime import datetime
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
from typing import Any

APP_IDENTIFIER = "io.github.jferments.ostudy"
MAX_RECENT_EVENTS = 80
MAX_COMMAND_CHARS = 20_000
KERNEL_HINTS = (
    "out of memory", "oom", "killed process", "segfault", "segmentation",
    "general protection", "trap", "sigsegv", "sigabrt", "ostudy",
    "webkit", "pdfium", "tesseract",
)


def paths() -> dict[str, Path]:
    explicit_app = os.environ.get("OSTUDY_APP_DATA_DIR", "").strip()
    if explicit_app:
        app_data = Path(explicit_app).expanduser()
    else:
        data_home = os.environ.get("XDG_DATA_HOME", "").strip()
        data_root = Path(data_home).expanduser() if data_home else Path.home() / ".local" / "share"
        app_data = data_root / APP_IDENTIFIER
    diagnostic_override = os.environ.get("OSTUDY_DIAGNOSTIC_LOG_PATH", "").strip()
    application_log = (
        Path(diagnostic_override).expanduser()
        if diagnostic_override
        else app_data / "diagnostics" / "ostudy.jsonl"
    )
    state_home = os.environ.get("XDG_STATE_HOME", "").strip()
    state_root = Path(state_home).expanduser() if state_home else Path.home() / ".local" / "state"
    launcher_log = state_root / "ostudy" / "ostudy.jsonl"
    return {
        "appData": app_data,
        "database": app_data / "library.sqlite3",
        "applicationLog": application_log,
        "launcherLog": launcher_log,
    }


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for candidate in (path.with_suffix(path.suffix + ".1"), path):
        if not candidate.is_file():
            continue
        for line in candidate.read_text(encoding="utf-8", errors="replace").splitlines():
            try:
                value = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(value, dict):
                records.append(value)
    return records


def latest_process(records: list[dict[str, Any]]) -> dict[str, Any] | None:
    current: dict[str, Any] | None = None
    latest: dict[str, Any] | None = None
    for record in records:
        if record.get("stream") is not None:
            continue
        if record.get("event") == "process_start":
            current = {"start": record, "exit": None}
            latest = current
        elif record.get("event") == "process_exit":
            if current is None:
                current = {"start": None, "exit": record}
                latest = current
            else:
                current["exit"] = record
    return latest


def signal_details(exit_record: dict[str, Any] | None) -> dict[str, Any]:
    if not exit_record:
        return {"exitCode": None, "signalNumber": None, "signalName": None}
    code = exit_record.get("exit_code")
    if not isinstance(code, int):
        return {"exitCode": code, "signalNumber": None, "signalName": None}
    number = exit_record.get("signal_number")
    name = exit_record.get("signal_name")
    if code < 0 and not isinstance(number, int):
        number = -code
    if isinstance(number, int) and not isinstance(name, str):
        try:
            name = signal.Signals(number).name
        except ValueError:
            name = f"SIG{number}"
    return {"exitCode": code, "signalNumber": number, "signalName": name}


def recent_application_events(records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    selected: list[dict[str, Any]] = []
    for record in records:
        level = str(record.get("level", "")).lower()
        target = str(record.get("target", ""))
        event = str(record.get("event", ""))
        if (
            level in {"warn", "warning", "error"}
            or target.startswith(("ostudy.documents", "ostudy.ai", "ostudy.frontend", "ostudy.app"))
            or any(term in event for term in ("image", "document", "generation", "panic", "crash"))
        ):
            selected.append(record)
    return selected[-MAX_RECENT_EVENTS:]


def bounded_command(args: list[str], timeout: float = 4.0) -> str | None:
    try:
        result = subprocess.run(
            args,
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    output = (result.stdout + "\n" + result.stderr).strip()
    return output[-MAX_COMMAND_CHARS:] if output else None


def kernel_evidence(start_timestamp: Any) -> list[str]:
    if not isinstance(start_timestamp, str):
        return []
    output = bounded_command(
        ["journalctl", "--no-pager", "-k", "--since", start_timestamp],
        timeout=5.0,
    )
    if not output:
        return []
    return [
        line for line in output.splitlines()
        if any(term in line.lower() for term in KERNEL_HINTS)
    ][-80:]


def coredump_evidence(pid: Any) -> str | None:
    if not isinstance(pid, int) or pid <= 0:
        return None
    return bounded_command(["coredumpctl", "--no-pager", "info", str(pid)], timeout=5.0)


def generation_usage(root: Path) -> dict[str, Any] | None:
    script = root / "scripts" / "generation-usage.py"
    if not script.is_file():
        return None
    try:
        result = subprocess.run(
            [sys.executable, str(script), "--json"],
            check=False,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if result.returncode != 0:
        return {"error": result.stderr.strip()[-2000:]}
    try:
        report = json.loads(result.stdout)
    except json.JSONDecodeError:
        return {"error": "generation-usage.py did not return JSON"}
    totals = report.get("totals", {})
    return {
        "operationId": report.get("operationId"),
        "operation": report.get("operation"),
        "wallClockMs": report.get("wallClockMs"),
        "usageComplete": report.get("usageComplete"),
        "reportedInputTokens": totals.get("inputTokens"),
        "reportedOutputTokens": totals.get("outputTokens"),
        "failedOrInterruptedRequests": totals.get("failedOrInterruptedRequests"),
        "repeatedLogicalRequests": totals.get("repeatedLogicalRequests"),
        "cacheReadPercentOfCoveredInput": totals.get("cacheReadPercentOfCoveredInput"),
        "localReuse": report.get("localReuse"),
    }


def source_sha(root: Path) -> str | None:
    value = bounded_command(["git", "-C", str(root), "rev-parse", "HEAD"], timeout=2.0)
    return value.splitlines()[-1] if value else None


def build_report(root: Path) -> dict[str, Any]:
    resolved = paths()
    launcher_records = read_jsonl(resolved["launcherLog"])
    process = latest_process(launcher_records)
    start = process.get("start") if process else None
    exit_record = process.get("exit") if process else None
    pid = (
        exit_record.get("pid")
        if isinstance(exit_record, dict) and isinstance(exit_record.get("pid"), int)
        else start.get("pid") if isinstance(start, dict) else None
    )
    process_details = signal_details(exit_record if isinstance(exit_record, dict) else None)
    if isinstance(exit_record, dict):
        for source, target in (
            ("runtime_ms", "runtimeMs"),
            ("resource_samples", "resourceSamples"),
            ("peak_group_rss_bytes", "peakGroupRssBytes"),
            ("peak_process_count", "peakProcessCount"),
            ("peak_thread_count", "peakThreadCount"),
            ("min_available_memory_bytes", "minAvailableMemoryBytes"),
            ("core_dump_candidate", "coreDumpCandidate"),
        ):
            process_details[target] = exit_record.get(source)
    process_details["pid"] = pid
    process_details["startedAt"] = start.get("timestamp") if isinstance(start, dict) else None
    process_details["finishedAt"] = (
        exit_record.get("timestamp") if isinstance(exit_record, dict) else None
    )

    app_records = read_jsonl(resolved["applicationLog"])
    return {
        "schemaVersion": 1,
        "generatedAt": datetime.now().astimezone().isoformat(timespec="seconds"),
        "sourceSha": source_sha(root),
        "paths": {key: str(value) for key, value in resolved.items()},
        "process": process_details,
        "recentApplicationEvents": recent_application_events(app_records),
        "latestGenerationUsage": generation_usage(root),
        "coredump": coredump_evidence(pid),
        "kernelEvidence": kernel_evidence(process_details.get("startedAt")),
    }


def render(report: dict[str, Any]) -> str:
    process = report["process"]
    lines = [
        "oStudy last-crash diagnostic",
        f"Source SHA: {report.get('sourceSha') or 'unknown'}",
        f"PID: {process.get('pid') or 'unknown'}",
        f"Start: {process.get('startedAt') or 'unknown'}",
        f"Exit: {process.get('finishedAt') or 'unknown'}",
        f"Exit code: {process.get('exitCode')}",
        f"Signal: {process.get('signalName') or 'none'} ({process.get('signalNumber')})",
        f"Runtime: {process.get('runtimeMs')} ms",
        f"Peak process-group RSS: {process.get('peakGroupRssBytes')} bytes",
        f"Minimum system MemAvailable: {process.get('minAvailableMemoryBytes')} bytes",
        "",
        "Latest content-free generation usage:",
        json.dumps(report.get("latestGenerationUsage"), indent=2, ensure_ascii=False),
        "",
        "Recent application events:",
    ]
    for event in report.get("recentApplicationEvents", []):
        lines.append(json.dumps(event, ensure_ascii=False, separators=(",", ":")))
    lines.extend(["", "Kernel evidence:"])
    lines.extend(report.get("kernelEvidence") or ["none found"])
    lines.extend(["", "Coredump evidence:", report.get("coredump") or "none found"])
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    report = build_report(root)
    print(json.dumps(report, indent=2, ensure_ascii=False) if args.json else render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
