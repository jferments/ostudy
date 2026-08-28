import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "project-orchestrator-checklist.mjs"
SHA = "a" * 40


class ProjectOrchestratorChecklistTests(unittest.TestCase):
    def project(
        self,
        *,
        body: str,
        commit_message: str,
        expected_sha: str = SHA,
        actual_sha: str = SHA,
    ) -> tuple[subprocess.CompletedProcess[str], dict | None]:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            body_file = root / "body.md"
            message_file = root / "message.txt"
            output_file = root / "result.json"
            body_file.write_text(body, encoding="utf-8")
            message_file.write_text(commit_message, encoding="utf-8")
            completed = subprocess.run(
                [
                    "node",
                    str(SCRIPT),
                    "--body-file",
                    str(body_file),
                    "--commit-message-file",
                    str(message_file),
                    "--expected-head-sha",
                    expected_sha,
                    "--actual-head-sha",
                    actual_sha,
                    "--output",
                    str(output_file),
                ],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            result = (
                json.loads(output_file.read_text(encoding="utf-8"))
                if output_file.is_file()
                else None
            )
            return completed, result

    def test_exact_unique_checkbox_is_checked(self) -> None:
        completed, result = self.project(
            body="- [ ] Finish Stage 2 item\n- [ ] Keep this open\n",
            commit_message=(
                "feat: checkpoint\n\n"
                "Orchestrator-Checklist-Item: Finish Stage 2 item\n"
            ),
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        assert result is not None
        self.assertTrue(result["changed"])
        self.assertIn("- [x] Finish Stage 2 item", result["body"])
        self.assertIn("- [ ] Keep this open", result["body"])

    def test_already_checked_item_is_idempotent(self) -> None:
        completed, result = self.project(
            body="- [x] Finish Stage 2 item\n",
            commit_message="Orchestrator-Checklist-Item: Finish Stage 2 item\n",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        assert result is not None
        self.assertFalse(result["changed"])
        self.assertEqual(result["body"], "- [x] Finish Stage 2 item\n")

    def test_missing_checkbox_fails_closed(self) -> None:
        completed, result = self.project(
            body="- [ ] Different item\n",
            commit_message="Orchestrator-Checklist-Item: Finish Stage 2 item\n",
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIsNone(result)
        self.assertIn("matched 0 PR checkbox lines", completed.stderr)

    def test_duplicate_checkbox_text_fails_closed(self) -> None:
        completed, result = self.project(
            body=(
                "- [ ] Finish Stage 2 item\n"
                "- [ ] Finish Stage 2 item\n"
            ),
            commit_message="Orchestrator-Checklist-Item: Finish Stage 2 item\n",
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIsNone(result)
        self.assertIn("matched 2 PR checkbox lines", completed.stderr)

    def test_head_sha_mismatch_fails_before_projection(self) -> None:
        completed, result = self.project(
            body="- [ ] Finish Stage 2 item\n",
            commit_message="Orchestrator-Checklist-Item: Finish Stage 2 item\n",
            actual_sha="b" * 40,
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIsNone(result)
        self.assertIn("PR head changed", completed.stderr)

    def test_no_trailers_is_read_only_noop(self) -> None:
        completed, result = self.project(
            body="- [ ] Finish Stage 2 item\n",
            commit_message="feat: ordinary commit\n",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        assert result is not None
        self.assertFalse(result["changed"])
        self.assertEqual(result["matchedItems"], [])

    def test_whitespace_is_normalized_but_text_must_still_match(self) -> None:
        completed, result = self.project(
            body="- [ ] Finish   Stage 2   item\n",
            commit_message="Orchestrator-Checklist-Item: Finish Stage 2 item\n",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        assert result is not None
        self.assertTrue(result["changed"])
        self.assertIn("Finish   Stage 2   item", result["body"])


if __name__ == "__main__":
    unittest.main()
