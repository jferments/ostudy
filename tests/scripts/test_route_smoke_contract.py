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

    def write_contract(self, root: Path, contract: dict) -> None:
        (root / "scripts/route-smoke-contract.json").write_text(
            json.dumps(contract, indent=2) + "\n",
            encoding="utf-8",
        )

    def test_repository_contract_passes(self) -> None:
        completed = self.run_verifier(ROOT)

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("7 application sections, 1 application workflow", completed.stdout)
        self.assertIn("7 route-entry; 2 presence-only", completed.stdout)
        self.assertIn("1 protected workflow", completed.stdout)
        self.assertIn("3 compact primary / 2 More destinations", completed.stdout)

    def test_internal_practice_retains_protected_smoke(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)

            completed = self.run_verifier(fixture)

            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertIn("5 primary destinations", completed.stdout)
            self.assertIn("1 protected workflow", completed.stdout)

    def test_selected_setup_and_direct_row_practice_are_in_the_smoke_inventory(self) -> None:
        contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
        practice = next(
            workflow
            for workflow in contract["protectedWorkflows"]
            if workflow["id"] == "active-practice"
        )
        native = next(
            evidence
            for evidence in practice["smoke"]["evidence"]
            if evidence["path"] == "tests/webdriver/smoke.py"
        )

        self.assertIn(
            "Opening and dismissing selected-deck Practice setup",
            native["entryFragments"],
        )
        self.assertIn("click(session_id, row_practice_button)", native["entryFragments"])
        self.assertIn("Practice selected decks", native["assertionFragments"])

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

    def test_rejects_missing_study_guides_destination(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            contract["applicationSections"] = [
                entry
                for entry in contract["applicationSections"]
                if entry["id"] != "study-guides"
            ]
            contract["primaryDestinations"] = [
                entry
                for entry in contract["primaryDestinations"]
                if entry["section"] != "study-guides"
            ]
            contract["compactNavigation"]["primary"].remove("study-guides")
            self.write_contract(fixture, contract)

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("study-guides", completed.stderr)
            self.assertIn("drifted", completed.stderr)

    def test_rejects_checkpoint_destination_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            contract["checkpointDestinations"].remove("study-guides")
            self.write_contract(fixture, contract)

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("checkpoint destination taxonomy drifted", completed.stderr)
            self.assertIn("study-guides", completed.stderr)

    def test_rejects_compact_destination_omission(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            contract["compactNavigation"]["more"].remove("settings")
            self.write_contract(fixture, contract)

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("compact navigation coverage is missing: settings", completed.stderr)

    def test_rejects_compact_destination_duplication(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            contract["compactNavigation"]["more"].insert(0, "analytics")
            self.write_contract(fixture, contract)

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("Duplicate compact destination: analytics", completed.stderr)

    def test_rejects_compact_group_order_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            contract["compactNavigation"]["more"].reverse()
            self.write_contract(fixture, contract)

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("compact More destination order drifted", completed.stderr)

    def test_rejects_smoke_evidence_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            self.copy_fixture(fixture)
            smoke = fixture / "tests/webdriver/ui_platform_smoke.py"
            source = smoke.read_text(encoding="utf-8")
            self.assertIn("webdriver.click(session_id, search)", source)
            smoke.write_text(
                source.replace(
                    "webdriver.click(session_id, search)",
                    "webdriver.click(session_id, changed_search)",
                    1,
                ),
                encoding="utf-8",
            )

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("Flashcards workspace search", completed.stderr)
            self.assertIn("missing required entry fragment", completed.stderr)

    def test_rejects_medium_navigation_catalog_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            self.copy_fixture(fixture)
            smoke = fixture / "tests/webdriver/card_listing_followup_smoke.py"
            source = smoke.read_text(encoding="utf-8")
            expected = (
                'EXPECTED_NAVIGATION = ["Flashcards", "Study Guides", "Documents", '
                '"Analytics", "Settings"]'
            )
            self.assertIn(expected, source)
            smoke.write_text(
                source.replace(expected, expected.replace("Study Guides", "Practice"), 1),
                encoding="utf-8",
            )

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("primary destination decks", completed.stderr)
            self.assertIn("missing required entry fragment", completed.stderr)

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
            smoke.write_text(source.replace(invocation, "session_id = session_id", 1), encoding="utf-8")

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("primary destination study-guides", completed.stderr)
            self.assertIn("missing required entry fragment", completed.stderr)

    def test_rejects_native_smoke_entrypoint_runner_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            self.copy_fixture(fixture)
            runner = fixture / "scripts/smoke-native.sh"
            source = runner.read_text(encoding="utf-8")
            invocation = (
                'run_smoke "Native durable-library WebDriver smoke" \\\n'
                '  "$ROOT_DIR/tests/webdriver/durable_library_smoke.py"\n'
            )
            self.assertIn(invocation, source)
            runner.write_text(source.replace(invocation, "", 1), encoding="utf-8")

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn(
                "native smoke entrypoint tests/webdriver/durable_library_smoke.py "
                "is not enumerated by scripts/smoke-native.sh",
                completed.stderr,
            )

    def test_rejects_native_smoke_delegation_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            self.copy_fixture(fixture)
            entrypoint = fixture / "tests/webdriver/durable_library_smoke.py"
            source = entrypoint.read_text(encoding="utf-8")
            self.assertIn("raise SystemExit(webdriver.main())", source)
            entrypoint.write_text(
                source.replace(
                    "raise SystemExit(webdriver.main())",
                    "raise SystemExit(webdriver.main_changed())",
                    1,
                ),
                encoding="utf-8",
            )

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("protected workflow active-practice execution chain", completed.stderr)
            self.assertIn("missing required delegation fragment", completed.stderr)

    def test_rejects_android_instrumentation_runner_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            self.copy_fixture(fixture)
            runner = fixture / "scripts/android-emulator-smoke.sh"
            source = runner.read_text(encoding="utf-8")
            invocation = "  record_case c_documentsStubAndAndroidBackReturnToFlashcardsDecks || true\n"
            self.assertIn(invocation, source)
            runner.write_text(source.replace(invocation, "", 1), encoding="utf-8")

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn(
                "instrumentation case "
                "AndroidSmokeTest#c_documentsStubAndAndroidBackReturnToFlashcardsDecks "
                "is not scheduled by scripts/android-emulator-smoke.sh",
                completed.stderr,
            )

    def test_rejects_rendered_app_destination_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            self.copy_fixture(fixture)
            app = fixture / "src/App.tsx"
            source = app.read_text(encoding="utf-8")
            rendered_branch = '{controller.activeWorkflow === "practice" ? ('
            self.assertIn(rendered_branch, source)
            app.write_text(
                source.replace(rendered_branch, "{false ? (", 1),
                encoding="utf-8",
            )

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("rendered App shell workflows drifted", completed.stderr)
            self.assertIn("practice", completed.stderr)

    def test_presence_evidence_cannot_be_reclassified_as_route_entry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            contract = self.copy_fixture(fixture)
            templates = next(
                entry
                for entry in contract["flashcardsWorkspaces"]
                if entry["id"] == "templates"
            )
            templates["smoke"]["coverage"] = "route-entry"
            self.write_contract(fixture, contract)

            completed = self.run_verifier(fixture)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("entryFragments", completed.stderr)
            self.assertIn("must not use presenceFragments", completed.stderr)

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
