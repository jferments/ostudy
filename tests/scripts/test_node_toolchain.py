import os
import platform
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "scripts" / "lib" / "node-toolchain.bash"


class NodeToolchainTests(unittest.TestCase):
    def write_version_command(self, path: Path, output: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"#!/usr/bin/env bash\nprintf '%s\\n' '{output}'\n", encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def managed_arch(self) -> str:
        machine = platform.machine()
        if machine == "x86_64":
            return "x64"
        if machine in {"aarch64", "arm64"}:
            return "arm64"
        self.skipTest(f"unsupported test architecture: {machine}")
        raise AssertionError("unreachable")

    def test_setup_managed_node_wins_over_drifted_local_bin(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "home"
            local_bin = home / ".local" / "bin"
            managed_bin = (
                home
                / ".local"
                / "opt"
                / f"node-v24.18.0-linux-{self.managed_arch()}"
                / "bin"
            )

            self.write_version_command(local_bin / "node", "v26.7.0")
            self.write_version_command(local_bin / "npm", "11.19.0")
            self.write_version_command(local_bin / "pnpm", "11.14.0")
            self.write_version_command(managed_bin / "node", "v24.18.0")
            self.write_version_command(managed_bin / "npm", "11.16.0")

            env = os.environ.copy()
            env.update(
                {
                    "HOME": str(home),
                    "NODE_VERSION": "24.18.0",
                    "NPM_VERSION": "11.16.0",
                    "PATH": f"{local_bin}:/usr/bin:/bin",
                }
            )
            command = f"""
source {HELPER!s}
ostudy_activate_node_toolchain
printf '%s\n' "$(command -v node)"
printf '%s\n' "$(node --version)"
printf '%s\n' "$(npm --version)"
printf '%s\n' "$OSTUDY_NODE_TOOLCHAIN_SOURCE"
"""
            completed = subprocess.run(
                ["bash", "-c", command],
                check=False,
                capture_output=True,
                text=True,
                env=env,
            )

            self.assertEqual(completed.returncode, 0, completed.stderr)
            lines = completed.stdout.splitlines()
            self.assertEqual(lines[0], str(managed_bin / "node"))
            self.assertEqual(lines[1], "v24.18.0")
            self.assertEqual(lines[2], "11.16.0")
            self.assertEqual(lines[3], "setup-managed")

    def test_exact_path_node_is_preserved_without_managed_install(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "home"
            exact_bin = Path(tmp) / "exact" / "bin"
            self.write_version_command(exact_bin / "node", "v24.18.0")
            self.write_version_command(exact_bin / "npm", "11.16.0")

            env = os.environ.copy()
            env.update(
                {
                    "HOME": str(home),
                    "NODE_VERSION": "24.18.0",
                    "NPM_VERSION": "11.16.0",
                    "PATH": f"{exact_bin}:/usr/bin:/bin",
                }
            )
            command = f"""
source {HELPER!s}
ostudy_activate_node_toolchain
printf '%s\n' "$(command -v node)"
printf '%s\n' "$OSTUDY_NODE_TOOLCHAIN_SOURCE"
"""
            completed = subprocess.run(
                ["bash", "-c", command],
                check=False,
                capture_output=True,
                text=True,
                env=env,
            )

            self.assertEqual(completed.returncode, 0, completed.stderr)
            lines = completed.stdout.splitlines()
            self.assertEqual(lines[0], str(exact_bin / "node"))
            self.assertEqual(lines[1], "PATH")


if __name__ == "__main__":
    unittest.main()
