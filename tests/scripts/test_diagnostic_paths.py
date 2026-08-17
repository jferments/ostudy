from __future__ import annotations

import os
from pathlib import Path
import shlex
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
DIAGNOSTICS_LIBRARY = ROOT / "scripts" / "lib" / "diagnostics.bash"


class DiagnosticPathResolutionTests(unittest.TestCase):
    def resolve(self, overrides: dict[str, str | None]) -> str:
        environment = os.environ.copy()
        for key in ("OSTUDY_DIAGNOSTIC_LOG_PATH", "OSTUDY_APP_DATA_DIR", "XDG_DATA_HOME"):
            environment.pop(key, None)
        for key, value in overrides.items():
            if value is None:
                environment.pop(key, None)
            else:
                environment[key] = value
        command = (
            f"source {shlex.quote(str(DIAGNOSTICS_LIBRARY))}; "
            "ostudy_resolve_application_diagnostic_log"
        )
        completed = subprocess.run(
            ["bash", "-c", command],
            check=True,
            capture_output=True,
            text=True,
            env=environment,
        )
        return completed.stdout.strip()

    def test_explicit_log_path_has_highest_precedence(self) -> None:
        resolved = self.resolve(
            {
                "OSTUDY_DIAGNOSTIC_LOG_PATH": "/tmp/explicit/ostudy.jsonl",
                "OSTUDY_APP_DATA_DIR": "/tmp/app-data",
                "XDG_DATA_HOME": "/tmp/xdg-data",
            }
        )
        self.assertEqual(resolved, "/tmp/explicit/ostudy.jsonl")

    def test_explicit_app_data_directory_uses_application_diagnostics_layout(self) -> None:
        resolved = self.resolve(
            {
                "OSTUDY_APP_DATA_DIR": "/tmp/ostudy-app-data",
                "XDG_DATA_HOME": "/tmp/xdg-data",
            }
        )
        self.assertEqual(
            resolved,
            "/tmp/ostudy-app-data/diagnostics/ostudy.jsonl",
        )

    def test_linux_default_uses_tauri_bundle_identifier_under_xdg_data(self) -> None:
        resolved = self.resolve({"XDG_DATA_HOME": "/tmp/xdg-data"})
        self.assertEqual(
            resolved,
            "/tmp/xdg-data/io.github.jferments.ostudy/diagnostics/ostudy.jsonl",
        )


if __name__ == "__main__":
    unittest.main()
