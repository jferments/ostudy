from __future__ import annotations

import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
DIAGNOSTICS_LIBRARY = ROOT / "scripts" / "lib" / "diagnostics.bash"


class DiagnosticPathResolutionTests(unittest.TestCase):
    def run_library(
        self,
        shell: str,
        overrides: dict[str, str | None],
    ) -> str:
        environment = os.environ.copy()
        for key in (
            "CI",
            "OSTUDY_APP_DATA_DIR",
            "OSTUDY_CI_LOG_FILTER",
            "OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT",
            "OSTUDY_DIAGNOSTIC_CAPTURE_TOKEN",
            "OSTUDY_DIAGNOSTIC_LOG_PATH",
            "OSTUDY_DIAGNOSTICS_ACTIVE",
            "OSTUDY_DIAGNOSTICS_AUTOMATIC",
            "OSTUDY_KEEP_FAILURE_ARTIFACTS",
            "OSTUDY_LOG",
            "XDG_DATA_HOME",
        ):
            environment.pop(key, None)
        for key, value in overrides.items():
            if value is None:
                environment.pop(key, None)
            else:
                environment[key] = value
        command = f"source {shlex.quote(str(DIAGNOSTICS_LIBRARY))}; {shell}"
        completed = subprocess.run(
            ["bash", "-c", command],
            check=True,
            capture_output=True,
            text=True,
            env=environment,
        )
        return completed.stdout.strip()

    def resolve(self, overrides: dict[str, str | None]) -> str:
        return self.run_library(
            "ostudy_resolve_application_diagnostic_log",
            overrides,
        )

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

    def test_explicit_artifact_root_wins_over_repository_default(self) -> None:
        resolved = self.run_library(
            "ostudy_diagnostic_artifact_root",
            {"OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT": "/tmp/runner/diagnostics"},
        )
        self.assertEqual(resolved, "/tmp/runner/diagnostics")

    def test_ci_activates_low_noise_timeline_and_bounded_artifacts(self) -> None:
        resolved = self.run_library(
            "ostudy_parse_diagnostic_args >/dev/null; "
            "printf '%s|%s|%s' \"$OSTUDY_LOG\" \"$OSTUDY_DIAGNOSTICS_AUTOMATIC\" \"$OSTUDY_KEEP_FAILURE_ARTIFACTS\"",
            {"CI": "true"},
        )
        self.assertEqual(resolved, "warn,ostudy=info|1|1")

    def test_step_specific_replay_prefers_database_diagnostics(self) -> None:
        resolved = self.run_library(
            "ostudy_recommended_filter_for_step 'Latest-schema migration contract'",
            {},
        )
        self.assertEqual(resolved, "warn,ostudy.database=debug")

    def test_nested_failure_traps_keep_only_the_precise_capture(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            artifact_root = Path(raw_directory) / "artifacts"
            output = self.run_library(
                "ostudy_parse_diagnostic_args >/dev/null; "
                "ostudy_capture_failure_artifacts 'React tests' 1; "
                "ostudy_capture_failure_artifacts initialization 1; "
                f"capture=$(find {shlex.quote(str(artifact_root))} -mindepth 1 -maxdepth 1 -type d ! -name '.*' | head -n 1); "
                f"count=$(find {shlex.quote(str(artifact_root))} -mindepth 1 -maxdepth 1 -type d ! -name '.*' | wc -l); "
                "printf '%s|%s' \"$count\" \"$(grep -F 'step=React tests' \"$capture/context.txt\" >/dev/null && printf precise)\"",
                {
                    "CI": "true",
                    "OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT": str(artifact_root),
                },
            )
            self.assertEqual(output, "1|precise")

    def test_failure_capture_preserves_isolated_log_and_focused_replay(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            directory = Path(raw_directory)
            app_log = directory / "isolated" / "diagnostics" / "ostudy.jsonl"
            command_log = directory / "smoke.log"
            artifact_root = directory / "artifacts"
            app_log.parent.mkdir(parents=True)
            app_log.write_text('{"application":"ostudy","level":"error"}\n', encoding="utf-8")
            command_log.write_text("driver failed\n", encoding="utf-8")
            output = self.run_library(
                "ostudy_capture_failure_artifacts "
                "'Native Tauri/WebDriver smoke test' 7 "
                f"{shlex.quote(str(command_log))} {shlex.quote(str(app_log))}; "
                f"capture=$(find {shlex.quote(str(artifact_root))} -mindepth 1 -maxdepth 1 -type d | head -n 1); "
                "printf '%s|%s|%s' "
                '"$(test -f "$capture/application.jsonl" && printf yes)" '
                '"$(grep -F "ostudy.frontend=debug" "$capture/reproduce.sh" >/dev/null && printf yes)" '
                '"$(grep -F "kind=failure" "$capture/context.txt" >/dev/null && printf yes)"',
                {
                    "OSTUDY_DIAGNOSTIC_ARTIFACT_ROOT": str(artifact_root),
                    "OSTUDY_KEEP_FAILURE_ARTIFACTS": "1",
                },
            )
            self.assertEqual(output, "yes|yes|yes")


if __name__ == "__main__":
    unittest.main()
