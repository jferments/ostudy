#!/usr/bin/env python3
"""Run ostudy while preserving bounded, structured terminal diagnostics."""

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import IO, Any

MAX_LOG_BYTES = 2 * 1024 * 1024
MAX_RECORD_BYTES = 32 * 1024
MAX_STRING_CHARS = 8_000
MAX_COLLECTION_ITEMS = 64
MAX_DEPTH = 6


def timestamp() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def bounded_value(value: Any, depth: int = 0) -> Any:
    """Bound untrusted diagnostic values without inspecting application data."""
    if depth >= MAX_DEPTH:
        return "[maximum diagnostic depth reached]"
    if isinstance(value, str):
        if len(value) <= MAX_STRING_CHARS:
            return value
        omitted = len(value) - MAX_STRING_CHARS
        return f"{value[:MAX_STRING_CHARS]}… [truncated {omitted} characters]"
    if value is None or isinstance(value, (bool, int, float)):
        return value
    if isinstance(value, dict):
        items = list(value.items())
        bounded = {
            str(key)[:256]: bounded_value(item, depth + 1)
            for key, item in items[:MAX_COLLECTION_ITEMS]
        }
        if len(items) > MAX_COLLECTION_ITEMS:
            bounded["diagnosticItemsOmitted"] = len(items) - MAX_COLLECTION_ITEMS
        return bounded
    if isinstance(value, (list, tuple)):
        bounded = [bounded_value(item, depth + 1) for item in value[:MAX_COLLECTION_ITEMS]]
        if len(value) > MAX_COLLECTION_ITEMS:
            bounded.append({"diagnosticItemsOmitted": len(value) - MAX_COLLECTION_ITEMS})
        return bounded
    return bounded_value(str(value), depth + 1)


class BoundedJsonlWriter:
    """JSONL writer that enforces limits while the process is still running."""

    def __init__(
        self,
        path: Path,
        *,
        max_bytes: int = MAX_LOG_BYTES,
        max_record_bytes: int = MAX_RECORD_BYTES,
    ) -> None:
        if max_bytes < 256 or max_record_bytes < 128:
            raise ValueError("diagnostic limits are too small")
        self.path = path
        self.backup = path.with_suffix(path.suffix + ".1")
        self.max_bytes = max_bytes
        self.max_record_bytes = min(max_record_bytes, max_bytes)
        self.lock = threading.Lock()
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self._file: IO[str] | None = None
        self._size = self.path.stat().st_size if self.path.exists() else 0
        if self._size >= self.max_bytes:
            self._rotate()
        else:
            self._open()

    def _open(self) -> None:
        self._file = self.path.open("a", encoding="utf-8", buffering=1)
        self._size = self.path.stat().st_size if self.path.exists() else 0

    def _rotate(self) -> None:
        if self._file is not None:
            self._file.close()
            self._file = None
        self.backup.unlink(missing_ok=True)
        if self.path.exists():
            self.path.replace(self.backup)
        self._open()

    def _serialize(self, record: dict[str, Any]) -> bytes:
        bounded = bounded_value(record)
        payload = json.dumps(bounded, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(payload) + 1 <= self.max_record_bytes:
            return payload + b"\n"
        fallback = {
            "timestamp": record.get("timestamp", timestamp()),
            "application": "ostudy",
            "version": record.get("version", "unknown"),
            "level": record.get("level", "error"),
            "event": record.get("event", "diagnostic_record_truncated"),
            "message": "Diagnostic record exceeded the per-record limit.",
            "truncated": True,
            "originalBytes": len(payload),
        }
        encoded = json.dumps(fallback, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        return encoded[: self.max_record_bytes - 1] + b"\n"

    def write(self, record: dict[str, Any]) -> None:
        payload = self._serialize(record)
        with self.lock:
            if self._size > 0 and self._size + len(payload) > self.max_bytes:
                self._rotate()
            assert self._file is not None
            self._file.write(payload.decode("utf-8"))
            self._file.flush()
            self._size += len(payload)

    def close(self) -> None:
        with self.lock:
            if self._file is not None:
                self._file.close()
                self._file = None

    def __enter__(self) -> "BoundedJsonlWriter":
        return self

    def __exit__(self, _type: object, _value: object, _traceback: object) -> None:
        self.close()



def termination_details(return_code: int) -> dict[str, Any]:
    if return_code >= 0:
        return {
            "signal_number": None,
            "signal_name": None,
            "core_dump_candidate": False,
        }
    signal_number = -return_code
    try:
        signal_name = signal.Signals(signal_number).name
    except ValueError:
        signal_name = f"SIG{signal_number}"
    core_signals = {
        getattr(signal, name)
        for name in ("SIGABRT", "SIGBUS", "SIGFPE", "SIGILL", "SIGSEGV", "SIGTRAP")
        if hasattr(signal, name)
    }
    return {
        "signal_number": signal_number,
        "signal_name": signal_name,
        "core_dump_candidate": signal_number in core_signals,
    }


def _read_status_value(path: Path, key: str) -> int | None:
    try:
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if not line.startswith(key + ":"):
                continue
            value = line.split(":", 1)[1].strip().split()[0]
            return int(value)
    except (OSError, ValueError, IndexError):
        return None
    return None


def _process_group_sample(process_group: int) -> tuple[int, int, int]:
    proc = Path("/proc")
    if not proc.is_dir():
        return (0, 0, 0)
    rss_kib = 0
    process_count = 0
    thread_count = 0
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            stat = (entry / "stat").read_text(encoding="utf-8", errors="replace")
            end = stat.rfind(")")
            if end < 0:
                continue
            fields = stat[end + 2 :].split()
            if len(fields) < 3 or int(fields[2]) != process_group:
                continue
        except (OSError, ValueError):
            continue
        process_count += 1
        rss_kib += _read_status_value(entry / "status", "VmRSS") or 0
        thread_count += _read_status_value(entry / "status", "Threads") or 0
    return (rss_kib * 1024, process_count, thread_count)


def _available_memory_bytes() -> int | None:
    value = _read_status_value(Path("/proc/meminfo"), "MemAvailable")
    return None if value is None else value * 1024


class ProcessResourceMonitor:
    def __init__(self, process_group: int) -> None:
        self.process_group = process_group
        self.stop_event = threading.Event()
        self.thread = threading.Thread(target=self._run, name="ostudy-resource-monitor", daemon=True)
        self.peak_group_rss_bytes = 0
        self.peak_process_count = 0
        self.peak_thread_count = 0
        self.min_available_memory_bytes: int | None = None
        self.samples = 0

    def start(self) -> None:
        self.thread.start()

    def stop(self) -> dict[str, Any]:
        self.stop_event.set()
        self.thread.join(timeout=2)
        return {
            "resource_samples": self.samples,
            "peak_group_rss_bytes": self.peak_group_rss_bytes,
            "peak_process_count": self.peak_process_count,
            "peak_thread_count": self.peak_thread_count,
            "min_available_memory_bytes": self.min_available_memory_bytes,
        }

    def _run(self) -> None:
        while not self.stop_event.is_set():
            rss, processes, threads = _process_group_sample(self.process_group)
            available = _available_memory_bytes()
            self.samples += 1
            self.peak_group_rss_bytes = max(self.peak_group_rss_bytes, rss)
            self.peak_process_count = max(self.peak_process_count, processes)
            self.peak_thread_count = max(self.peak_thread_count, threads)
            if available is not None:
                self.min_available_memory_bytes = (
                    available
                    if self.min_available_memory_bytes is None
                    else min(self.min_available_memory_bytes, available)
                )
            self.stop_event.wait(1.0)

def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--log", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required after --")

    with BoundedJsonlWriter(args.log) as writer:
        def write_record(record: dict[str, Any]) -> None:
            base: dict[str, Any] = {
                "timestamp": timestamp(),
                "application": "ostudy",
                "version": args.version,
            }
            base.update(record)
            writer.write(base)

        process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            start_new_session=True,
        )
        started_monotonic = time.monotonic()
        monitor = ProcessResourceMonitor(process.pid)
        monitor.start()
        write_record({
            "level": "info",
            "event": "process_start",
            "command": command[0],
            "pid": process.pid,
        })

        def forward(signum: int, _frame: object) -> None:
            if process.poll() is None:
                os.killpg(process.pid, signum)

        for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(signum, forward)

        def consume(stream: IO[str], target: IO[str], stream_name: str) -> None:
            for raw_line in stream:
                target.write(raw_line)
                target.flush()
                line = raw_line.rstrip("\n")
                try:
                    payload = json.loads(line)
                except json.JSONDecodeError:
                    payload = None
                if isinstance(payload, dict):
                    write_record({"stream": stream_name, **payload})
                else:
                    write_record(
                        {
                            "level": "error" if stream_name == "stderr" else "info",
                            "event": "process_output",
                            "stream": stream_name,
                            "message": line,
                        }
                    )

        assert process.stdout is not None
        assert process.stderr is not None
        threads = [
            threading.Thread(target=consume, args=(process.stdout, sys.stdout, "stdout")),
            threading.Thread(target=consume, args=(process.stderr, sys.stderr, "stderr")),
        ]
        for thread in threads:
            thread.start()

        return_code = process.wait()
        resources = monitor.stop()
        for thread in threads:
            thread.join()
        write_record(
            {
                "level": "info" if return_code == 0 else "error",
                "event": "process_exit",
                "exit_code": return_code,
                "pid": process.pid,
                "runtime_ms": int((time.monotonic() - started_monotonic) * 1000),
                **termination_details(return_code),
                **resources,
            }
        )
        return return_code


if __name__ == "__main__":
    raise SystemExit(main())
