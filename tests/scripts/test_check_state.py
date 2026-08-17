from __future__ import annotations

import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
CHECK_STATE = ROOT / "scripts" / "check-state.sh"


class WorktreeMutationGuardTests(unittest.TestCase):
    def initialize_repo(self, path: Path) -> None:
        subprocess.run(["git", "init", "-q", str(path)], check=True)
        subprocess.run(["git", "-C", str(path), "config", "user.email", "test@example.invalid"], check=True)
        subprocess.run(["git", "-C", str(path), "config", "user.name", "ostudy test"], check=True)
        (path / "tracked.txt").write_text("baseline\n", encoding="utf-8")
        subprocess.run(["git", "-C", str(path), "add", "tracked.txt"], check=True)
        subprocess.run(["git", "-C", str(path), "commit", "-qm", "baseline"], check=True)

    def run_guard(self, repo: Path, body: str) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment["OSTUDY_ROOT_DIR"] = str(repo)
        command = f"source {shlex.quote(str(CHECK_STATE))}; {body}"
        return subprocess.run(
            ["bash", "-c", command],
            capture_output=True,
            text=True,
            env=environment,
            check=False,
        )

    def test_new_untracked_native_file_is_attributed_to_the_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            repo = Path(raw_directory) / "repo"
            repo.mkdir()
            self.initialize_repo(repo)
            completed = self.run_guard(
                repo,
                "baseline=$(mktemp); "
                "ostudy_reset_worktree_baseline \"$baseline\"; "
                "touch \"$OSTUDY_ROOT_DIR/Native\"; "
                "ostudy_assert_worktree_unchanged \"$baseline\" 'Synthetic native gate'",
            )
            self.assertEqual(completed.returncode, 1)
            self.assertIn("Synthetic native gate changed Git-visible repository state", completed.stderr)
            self.assertIn("+?? Native", completed.stderr)

    def test_preexisting_untracked_file_is_not_bl_id_as_gate_output(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            repo = Path(raw_directory) / "repo"
            repo.mkdir()
            self.initialize_repo(repo)
            (repo / "preexisting.txt").write_text("keep\n", encoding="utf-8")
            completed = self.run_guard(
                repo,
                "baseline=$(mktemp); "
                "ostudy_reset_worktree_baseline \"$baseline\"; "
                "ostudy_assert_worktree_unchanged \"$baseline\" 'No-op gate'",
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_tracked_source_mutation_is_reported(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            repo = Path(raw_directory) / "repo"
            repo.mkdir()
            self.initialize_repo(repo)
            completed = self.run_guard(
                repo,
                "baseline=$(mktemp); "
                "ostudy_reset_worktree_baseline \"$baseline\"; "
                "printf changed > \"$OSTUDY_ROOT_DIR/tracked.txt\"; "
                "ostudy_assert_worktree_unchanged \"$baseline\" 'Synthetic tracked gate'",
            )
            self.assertEqual(completed.returncode, 1)
            self.assertIn("+ M tracked.txt", completed.stderr)


if __name__ == "__main__":
    unittest.main()
