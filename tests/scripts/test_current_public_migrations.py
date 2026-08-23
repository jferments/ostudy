import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "scripts" / "current-public-migrations.mjs"
SMOKE = ROOT / "scripts" / "smoke-startup-compatibility.sh"
MIGRATIONS = ROOT / "src-tauri" / "migrations"


class CurrentPublicMigrationContractTests(unittest.TestCase):
    def run_helper(self, mode: str) -> str:
        completed = subprocess.run(
            ["node", str(HELPER), mode],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        return completed.stdout.strip()

    def test_helper_tracks_latest_registered_migration(self) -> None:
        migration_files = sorted(MIGRATIONS.glob("[0-9][0-9][0-9][0-9]_*.sql"))
        self.assertTrue(migration_files)
        expected_latest = max(int(path.name[:4]) for path in migration_files)
        self.assertEqual(int(self.run_helper("--latest")), expected_latest)

        ledger = self.run_helper("--ledger").splitlines()
        self.assertEqual(len(ledger), len(migration_files))
        self.assertEqual([int(line.split("|", 1)[0]) for line in ledger], list(range(1, expected_latest + 1)))
        for line in ledger:
            version, name, checksum = line.split("|")
            self.assertEqual(len(checksum), 64)
            self.assertRegex(checksum, r"^[0-9a-f]{64}$")
            expected_prefix = f"{int(version):04d}_{name}"
            self.assertTrue(
                any(path.name.startswith(expected_prefix) for path in migration_files),
                f"missing migration file for {line}",
            )

    def test_startup_smoke_consumes_current_contract_instead_of_hard_coding_latest(self) -> None:
        smoke = SMOKE.read_text(encoding="utf-8")
        self.assertIn('current-public-migrations.mjs" --latest', smoke)
        self.assertIn('current-public-migrations.mjs" --ledger', smoke)
        self.assertIn('"$schema_version" == "$expected_schema_version"', smoke)
        self.assertNotRegex(smoke, r'\[\[ "\$schema_version" == "\d+" \]\]')
        self.assertNotIn("expected_migrations=\"$(cat <<'EOF'\n1|public_baseline", smoke)


if __name__ == "__main__":
    unittest.main()
