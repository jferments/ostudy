import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERIFIER = ROOT / "scripts" / "verify-route-smoke-contract.mjs"
CONTRACT = ROOT / "scripts" / "route-smoke-contract.json"


class RouteSmokeContractTests(unittest.TestCase):
    def run_verifier(self, root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["node", str(VERIFIER), "--root", str(root)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def copy_fixture(self, root: Path) -> dict:
        contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
        paths = set(contract["authoritativeSources"].values())

        def add_smoke_paths(smoke: dict) -> None:
            for evidence in smoke.get("evidence", []):
                paths.add(evidence["path"])
                if "entrypoint" in evidence:
                    paths.add(evidence["entrypoint"])
                for edge in evidence.get("executionChain", []):
                    paths.add(edge["from"])
                    paths.add(edge["to"])

        for catalog in ("primaryDestinations", "flashcardsWorkspaces"):
            for entry in contract[catalog]:
                add_smoke_paths(entry["smoke"])
        for workflow in contract["protectedWorkflows"]:
            add_smoke_paths(workflow["smoke"])
        paths.add("scripts/route-smoke-contract.json")
        for relative_path in paths:
            destination = root / relative_path
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / relative_path, destination)
        return contract

    @staticmethod
    def write_contract(root: Path, contract: dict) -> None:
        (root / "scripts/route-smoke-contract.json").write_text(
            json.dumps(contract, indent=2) + "\n",
            encoding="utf-8",
        )

    def test_repository_contract_passes(self) -> None:
        completed = self.run_verifier(ROOT)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("7 application sections, 1 application workflow", completed.stdout)
        self.assertIn("7 route-entry; 0 presence-only", completed.stdout)
        self.assertIn("1 protected workflow", completed.stdout)
        self.assertIn("4 compact primary / 1 More destinations", completed.stdout)

    def test_internal_practice_retains_protected_smoke(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            self.copy_fixture(fixture)
            completed = self.run_verifier(fixture)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn("5 primary destinations", completed.stdout)
            self.assertIn("1 protected workflow", completed.stdout)

    def test_internal_practice_cannot_drop_protected_smoke(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            contract["protectedWorkflows"] = []
            self.write_contract(fixture, contract)
            completed = self.run_verifier(fixture)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn(
                "internal workflow practice is missing protected smoke coverage",
                completed.stderr,
            )

    def test_rejects_missing_compact_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            contract["compactNavigation"]["primary"].remove("documents")
            self.write_contract(fixture, contract)
            completed = self.run_verifier(fixture)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("compact navigation coverage is missing: documents", completed.stderr)

    def test_rejects_compact_order_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            contract["compactNavigation"]["primary"].reverse()
            self.write_contract(fixture, contract)
            completed = self.run_verifier(fixture)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("compact primary destination order drifted", completed.stderr)

    def test_rejects_flashcards_workspace_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            contract["flashcardsWorkspaces"].append(
                {
                    "id": "transfer",
                    "label": "Import/Export",
                    "smoke": contract["flashcardsWorkspaces"][0]["smoke"],
                }
            )
            self.write_contract(fixture, contract)
            completed = self.run_verifier(fixture)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("Flashcards route workspaces drifted", completed.stderr)

    def test_rejects_removed_study_guides_restart_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            self.copy_fixture(fixture)
            smoke = fixture / "tests/webdriver/ui_platform_smoke.py"
            source = smoke.read_text(encoding="utf-8")
            invocation = (
                "session_id = verify_study_guides_checkpoint_after_restart("
                "application, session_id)"
            )
            self.assertIn(invocation, source)
            smoke.write_text(
                source.replace(invocation, "session_id = session_id", 1),
                encoding="utf-8",
            )
            completed = self.run_verifier(fixture)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("primary destination study-guides", completed.stderr)
            self.assertIn("missing required entry fragment", completed.stderr)

    def test_route_entry_cannot_be_reclassified_as_presence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            search = next(
                entry for entry in contract["flashcardsWorkspaces"] if entry["id"] == "search"
            )
            search["smoke"]["coverage"] = "presence"
            self.write_contract(fixture, contract)
            completed = self.run_verifier(fixture)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("presenceFragments", completed.stderr)
            self.assertIn("must not claim route entry/assertion fragments", completed.stderr)

    def test_product_source_cannot_import_expected_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            self.copy_fixture(fixture)
            product_source = fixture / "src/routeDependency.ts"
            product_source.write_text(
                'import contract from "../scripts/route-smoke-contract.json";\n'
                "export default contract;\n",
                encoding="utf-8",
            )
            completed = self.run_verifier(fixture)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("src/routeDependency.ts depends on", completed.stderr)
            self.assertIn("application routing source must remain authoritative", completed.stderr)


if __name__ == "__main__":
    unittest.main()
