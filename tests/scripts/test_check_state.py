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

    def isolated_environment(self) -> dict[str, str]:
        environment = os.environ.copy()
        # The outer qualification process exports this after reporting its own
        # baseline. Every subprocess-backed unit test must exercise a fresh
        # guard instance rather than inheriting that one-shot reporting state.
        environment.pop("OSTUDY_WORKTREE_BASELINE_REPORTED", None)
        environment.pop("ANDROID_HOME", None)
        return environment

    def run_guard(self, repo: Path, body: str) -> subprocess.CompletedProcess[str]:
        environment = self.isolated_environment()
        environment["OSTUDY_ROOT_DIR"] = str(repo)
        command = f"source {shlex.quote(str(CHECK_STATE))}; {body}"
        return subprocess.run(
            ["bash", "-c", command],
            capture_output=True,
            text=True,
            env=environment,
            check=False,
        )

    def write_command(self, path: Path, output: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            f"#!/usr/bin/env bash\nprintf '%s\\n' {shlex.quote(output)}\n",
            encoding="utf-8",
        )
        path.chmod(0o755)

    def prepare_fake_toolchain(self, fake_bin: Path) -> None:
        self.write_command(fake_bin / "node", "v24.18.0")
        self.write_command(fake_bin / "npm", "11.16.0")
        self.write_command(fake_bin / "pnpm", "11.14.0")
        self.write_command(fake_bin / "rustc", "rustc 1.97.1 fake")
        self.write_command(fake_bin / "cargo", "cargo 1.97.1 fake")
        self.write_command(fake_bin / "java", "openjdk fake")
        self.write_command(fake_bin / "javac", "javac fake")

    def fingerprint(self, repo: Path, fake_bin: Path, tier: str = "exact") -> str:
        environment = self.isolated_environment()
        environment["OSTUDY_ROOT_DIR"] = str(repo)
        environment["PATH"] = f"{fake_bin}:/usr/bin:/bin"
        completed = subprocess.run(
            [
                "bash",
                "-c",
                f"source {shlex.quote(str(CHECK_STATE))}; ostudy_check_fingerprint {shlex.quote(tier)}",
            ],
            capture_output=True,
            text=True,
            env=environment,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return completed.stdout.strip()

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

    def test_preexisting_untracked_file_is_not_blindly_attributed_to_gate(self) -> None:
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
            self.assertIn("pre-existing Git-visible worktree state", completed.stderr)
            self.assertIn("?? preexisting.txt", completed.stderr)

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

    def test_fingerprint_changes_when_npm_version_changes(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            repo = Path(raw_directory) / "repo"
            fake_bin = Path(raw_directory) / "bin"
            repo.mkdir()
            self.initialize_repo(repo)
            self.prepare_fake_toolchain(fake_bin)

            before = self.fingerprint(repo, fake_bin)
            self.write_command(fake_bin / "npm", "11.19.0")
            after = self.fingerprint(repo, fake_bin)

            self.assertNotEqual(before, after)

    def test_product_fingerprint_ignores_only_maintained_markdown_docs(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            repo = Path(raw_directory) / "repo"
            fake_bin = Path(raw_directory) / "bin"
            repo.mkdir()
            self.initialize_repo(repo)
            self.prepare_fake_toolchain(fake_bin)
            (repo / "docs").mkdir()
            (repo / "docs" / "guide.md").write_text("before\n", encoding="utf-8")
            (repo / "README.md").write_text("before\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(repo), "add", "docs/guide.md", "README.md"], check=True)

            static_before = self.fingerprint(repo, fake_bin, "static")
            full_before = self.fingerprint(repo, fake_bin, "full")
            android_before = self.fingerprint(repo, fake_bin, "android-compile")

            (repo / "docs" / "guide.md").write_text("after\n", encoding="utf-8")
            (repo / "README.md").write_text("after\n", encoding="utf-8")

            self.assertNotEqual(static_before, self.fingerprint(repo, fake_bin, "static"))
            self.assertEqual(full_before, self.fingerprint(repo, fake_bin, "full"))
            self.assertEqual(android_before, self.fingerprint(repo, fake_bin, "android-compile"))

    def test_product_fingerprint_changes_for_source_and_other_repository_files(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            repo = Path(raw_directory) / "repo"
            fake_bin = Path(raw_directory) / "bin"
            repo.mkdir()
            self.initialize_repo(repo)
            self.prepare_fake_toolchain(fake_bin)
            (repo / "src").mkdir()
            (repo / "src" / "source.ts").write_text("before\n", encoding="utf-8")
            (repo / ".github").mkdir()
            (repo / ".github" / "policy.md").write_text("before\n", encoding="utf-8")
            subprocess.run(
                ["git", "-C", str(repo), "add", "src/source.ts", ".github/policy.md"],
                check=True,
            )

            before = self.fingerprint(repo, fake_bin, "full")
            (repo / "src" / "source.ts").write_text("after\n", encoding="utf-8")
            after_source = self.fingerprint(repo, fake_bin, "full")
            self.assertNotEqual(before, after_source)

            (repo / "src" / "source.ts").write_text("before\n", encoding="utf-8")
            (repo / ".github" / "policy.md").write_text("after\n", encoding="utf-8")
            after_policy = self.fingerprint(repo, fake_bin, "full")
            self.assertNotEqual(before, after_policy)

    def test_product_fingerprint_changes_for_committed_file_mode(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            repo = Path(raw_directory) / "repo"
            fake_bin = Path(raw_directory) / "bin"
            repo.mkdir()
            self.initialize_repo(repo)
            self.prepare_fake_toolchain(fake_bin)
            (repo / "scripts").mkdir()
            tool = repo / "scripts" / "tool.sh"
            tool.write_text("#!/usr/bin/env bash\necho ok\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(repo), "add", "scripts/tool.sh"], check=True)

            before = self.fingerprint(repo, fake_bin, "full")
            subprocess.run(
                ["git", "-C", str(repo), "update-index", "--chmod=+x", "scripts/tool.sh"],
                check=True,
            )
            after = self.fingerprint(repo, fake_bin, "full")

            self.assertNotEqual(before, after)

    def test_full_stamp_survives_docs_only_edit_but_static_stamp_does_not(self) -> None:
        with tempfile.TemporaryDirectory() as raw_directory:
            repo = Path(raw_directory) / "repo"
            state = Path(raw_directory) / "state"
            repo.mkdir()
            self.initialize_repo(repo)
            (repo / "docs").mkdir()
            guide = repo / "docs" / "guide.md"
            guide.write_text("before\n", encoding="utf-8")
            subprocess.run(["git", "-C", str(repo), "add", "docs/guide.md"], check=True)

            prefix = f"export OSTUDY_CHECK_STATE_DIR={shlex.quote(str(state))}; "
            written = self.run_guard(
                repo,
                prefix + "ostudy_write_check_stamp static; ostudy_write_check_stamp full",
            )
            self.assertEqual(written.returncode, 0, written.stderr)

            guide.write_text("after\n", encoding="utf-8")
            checked = self.run_guard(
                repo,
                prefix
                + "if ostudy_check_stamp_is_current full; then echo full-current; else echo full-stale; fi; "
                + "if ostudy_check_stamp_is_current static; then echo static-current; else echo static-stale; fi",
            )
            self.assertEqual(checked.returncode, 0, checked.stderr)
            self.assertIn("full-current", checked.stdout)
            self.assertIn("static-stale", checked.stdout)


if __name__ == "__main__":
    unittest.main()
