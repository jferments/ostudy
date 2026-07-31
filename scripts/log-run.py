#!/usr/bin/env python3
"""Run ostudy while preserving terminal output in a small JSON Lines log."""

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


def timestamp() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def rotate_log(path: Path) -> None:
    if path.exists() and path.stat().st_size >= MAX_LOG_BYTES:
        backup = path.with_suffix(path.suffix + ".1")
        backup.unlink(missing_ok=True)
        path.replace(backup)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--log", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required after --")

    args.log.parent.mkdir(parents=True, exist_ok=True)
    rotate_log(args.log)
    write_lock = threading.Lock()

    with args.log.open("a", encoding="utf-8", buffering=1) as log_file:
        def write_record(record: dict[str, Any]) -> None:
            base = {
                "timestamp": timestamp(),
                "application": "ostudy",
                "version": args.version,
            }
            base.update(record)
            with write_lock:
                log_file.write(json.dumps(base, ensure_ascii=False) + "\n")

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
