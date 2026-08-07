from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("ostudy_log_run", ROOT / "scripts" / "log-run.py")
assert SPEC is not None and SPEC.loader is not None
LOG_RUN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LOG_RUN)


class BoundedJsonlWriterTests(unittest.TestCase):
    def test_rotates_while_a_process_is_running_and_keeps_both_files_bounded(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ostudy.jsonl"
            with LOG_RUN.BoundedJsonlWriter(path, max_bytes=900, max_record_bytes=300) as writer:
                for index in range(40):
                    writer.write({"event": "line", "index": index, "message": "x" * 120})

            self.assertTrue(path.exists())
            self.assertTrue(path.with_suffix(".jsonl.1").exists())
            self.assertLessEqual(path.stat().st_size, 900)
            self.assertLessEqual(path.with_suffix(".jsonl.1").stat().st_size, 900)
            for candidate in (path, path.with_suffix(".jsonl.1")):
                for line in candidate.read_text(encoding="utf-8").splitlines():
                    json.loads(line)

    def test_replaces_an_oversized_record_with_bounded_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ostudy.jsonl"
            with LOG_RUN.BoundedJsonlWriter(path, max_bytes=1_024, max_record_bytes=256) as writer:
                writer.write(
                    {
                        "timestamp": "2026-08-06T00:00:00Z",
                        "version": "test",
                        "event": "webview_error",
                        "message": "sensitive-value" * 5_000,
                    }
                )

            raw = path.read_bytes()
            self.assertLessEqual(len(raw), 256)
            record = json.loads(raw)
            self.assertTrue(record["truncated"])
            self.assertEqual(record["event"], "webview_error")
            self.assertNotIn("sensitive-value", raw.decode("utf-8"))


if __name__ == "__main__":
    unittest.main()
