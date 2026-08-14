from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "summarize-diagnostics.mjs"


class DiagnosticSummaryTests(unittest.TestCase):
    def test_writes_machine_readable_counts_and_failure_highlights(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            logs = output / "logs"
            logs.mkdir()
            (logs / "typescript.log").write_text(
                "src/example.ts(1,1): error TS2322: Type 'number' is not assignable to type 'string'.\n",
                encoding="utf-8",
            )
            (logs / "vitest.log").write_text("all tests passed\n", encoding="utf-8")
            (logs / "cargo-check.log").write_text(
                "BLOCKED cargo-check (dependencies: cargo-metadata)\n",
                encoding="utf-8",
            )
            results = output / "results.tsv"
            results.write_text(
                "name\tprofile\tstatus\texit_code\tdependencies\tlog\n"
                "typescript\tcore\tFAIL\t2\tpnpm-install\tlogs/typescript.log\n"
                "vitest\tcore\tPASS\t0\tpnpm-install\tlogs/vitest.log\n"
                "cargo-check\tcore\tBLOCKED\t\tcargo-metadata\tlogs/cargo-check.log\n",
                encoding="utf-8",
            )

            subprocess.run(
                [
                    "node",
                    str(SCRIPT),
                    "--results",
                    str(results),
                    "--output-dir",
                    str(output),
                    "--profile",
                    "core",
                ],
                cwd=ROOT,
                check=True,
                text=True,
            )

            payload = json.loads((output / "diagnostics.json").read_text(encoding="utf-8"))
            self.assertEqual(payload["schemaVersion"], 1)
            self.assertEqual(payload["profile"], "core")
            self.assertEqual(payload["counts"], {"pass": 1, "fail": 1, "blocked": 1})
            failure = next(gate for gate in payload["gates"] if gate["name"] == "typescript")
            self.assertEqual(failure["exitCode"], 2)
            self.assertTrue(any("TS2322" in line for line in failure["highlights"]))

            summary = (output / "summary.md").read_text(encoding="utf-8")
            self.assertIn("1 passed", summary)
            self.assertIn("1 failed", summary)
            self.assertIn("1 blocked", summary)
            self.assertIn("typescript", summary)
            self.assertIn("TS2322", summary)


if __name__ == "__main__":
    unittest.main()
