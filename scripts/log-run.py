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

        write_record({"level": "info", "event": "process_start", "command": command[0]})
        process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            start_new_session=True,
        )

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
        for thread in threads:
            thread.join()
        write_record(
            {
                "level": "info" if return_code == 0 else "error",
                "event": "process_exit",
                "exit_code": return_code,
            }
        )
        return return_code


if __name__ == "__main__":
    raise SystemExit(main())
