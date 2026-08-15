from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class AndroidDiagnosticContractsTests(unittest.TestCase):
    def test_smoke_result_summarizer_emits_case_timing_and_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            results = root / "results.tsv"
            results.write_text(
                "case\tstatus\tprerequisite\tduration_ms\tlog\n"
                "launch\tPASS\t\t1200\tlaunch.txt\n"
                "picker\tFAIL\t\t3400\tpicker.txt\n"
                "restart\tSKIP\tlaunch failed\t\t\n",
                encoding="utf-8",
            )
            (root / "launch.txt").write_text("OK\n", encoding="utf-8")
            (root / "picker.txt").write_text("failure\n", encoding="utf-8")
            output = root / "android-smoke-results.json"
            subprocess.run(
                [
                    "node",
                    str(ROOT / "scripts" / "summarize-android-smoke.mjs"),
                    "--results",
                    str(results),
                    "--artifact-dir",
                    str(root),
                    "--output",
                    str(output),
                    "--exit-code",
                    "1",
                    "--serial",
                    "emulator-5554",
                    "--api-level",
                    "36",
                    "--page-size",
                    "16384",
                ],
                cwd=ROOT,
                check=True,
                text=True,
            )
            payload = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(payload["schemaVersion"], 1)
            self.assertEqual(payload["counts"], {"pass": 1, "fail": 1, "skip": 1})
            self.assertEqual(payload["apiLevel"], 36)
            self.assertEqual(payload["pageSize"], 16384)
            self.assertEqual(payload["cases"][0]["durationMs"], 1200)
            self.assertIn("picker.txt", payload["artifacts"])

    def test_android_qualification_is_tauri_first_and_reuses_exact_built_apks(self) -> None:
        android_test = (ROOT / "scripts" / "android-test.sh").read_text(encoding="utf-8")
        x86 = android_test.index("ostudy_android_gate_build_x86_smoke")
        gradle = android_test.index("ostudy_android_gate_gradle_contracts")
        self.assertGreater(gradle, x86)
        self.assertIn("ostudy_android_gate_build_arm64_debug", android_test)
        self.assertIn("ostudy_android_gate_build_cleanliness", android_test)
        self.assertIn("--app-apk", android_test)
        self.assertIn("--test-apk", android_test)

        workflow = (ROOT / ".github" / "workflows" / "android-check.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("actions/download-artifact@", workflow)
        self.assertIn("stage-android-smoke-artifacts.sh", workflow)
        self.assertIn("ostudy-android-smoke-apks-${{ github.sha }}", workflow)
        self.assertIn("ostudy-smoke-app.apk", workflow)
        self.assertIn("ostudy-smoke-test.apk", workflow)
        self.assertIn("sha256sum --check SHA256SUMS", workflow)
        self.assertIn("--app-apk", workflow)
        self.assertIn("--test-apk", workflow)
        self.assertIn("android-min-sdk-smoke", workflow)

    def test_android_workflows_route_only_to_the_dedicated_host_runner(self) -> None:
        check_workflow = (ROOT / ".github" / "workflows" / "android-check.yml").read_text(
            encoding="utf-8"
        )
        package_workflow = (ROOT / ".github" / "workflows" / "android-package.yml").read_text(
            encoding="utf-8"
        )
        compile_selector = "runs-on: [self-hosted, linux, x64, ostudy-android]"
        emulator_selector = "runs-on: [self-hosted, linux, x64, ostudy-android, kvm]"
        old_selector_fragment = "runs-on: [self-hosted, linux, x64, ostudy, android"

        self.assertIn(compile_selector, check_workflow)
        self.assertGreaterEqual(check_workflow.count(emulator_selector), 3)
        self.assertIn(compile_selector, package_workflow)
        self.assertNotIn(old_selector_fragment, check_workflow)
        self.assertNotIn(old_selector_fragment, package_workflow)

    def test_diagnostic_android_dag_is_tauri_first_and_collects_postbuild_failures_independently(self) -> None:
        diagnostic = (ROOT / "scripts" / "check-diagnostic.sh").read_text(encoding="utf-8")
        postbuild = (ROOT / "scripts" / "android-gradle-postbuild.sh").read_text(encoding="utf-8")

        self.assertIn("android-x86-smoke-build", diagnostic)
        self.assertIn("android-gradle-contracts", diagnostic)
        self.assertIn("android-jvm-tests", diagnostic)
        self.assertIn("android-lint", diagnostic)
        self.assertIn("android-instrumentation-package", diagnostic)
        self.assertNotIn("android-gradle-lint-tests", diagnostic)
        self.assertNotIn(":tauri-plugin-ostudy-platform:compileDebugKotlin", diagnostic)
        self.assertNotIn("./gradlew", diagnostic)

        for mode in ("jvm-tests", "lint", "instrumentation-package", "test-packages"):
            self.assertIn(mode, postbuild)
        self.assertIn(":tauri-plugin-ostudy-platform:testDebugUnitTest", postbuild)
        self.assertIn(":tauri-plugin-ostudy-platform:lintDebug", postbuild)
        self.assertIn(":app:testUniversalDebugUnitTest", postbuild)
        self.assertIn(":app:lintUniversalDebug", postbuild)
        self.assertIn(":app:assembleUniversalDebugAndroidTest", postbuild)

    def test_system_picker_success_contract_is_instrumentation_only(self) -> None:
        test_manifest = (
            ROOT / "src-tauri" / "gen" / "android" / "app" / "src" / "androidTest" / "AndroidManifest.xml"
        ).read_text(encoding="utf-8")
        provider = (
            ROOT
            / "src-tauri"
            / "gen"
            / "android"
            / "app"
            / "src"
            / "androidTest"
            / "java"
            / "io"
            / "github"
            / "jferments"
            / "ostudy"
            / "OstudyTestDocumentsProvider.kt"
        ).read_text(encoding="utf-8")
        smoke = (
            ROOT
            / "src-tauri"
            / "gen"
            / "android"
            / "app"
            / "src"
            / "androidTest"
            / "java"
            / "io"
            / "github"
            / "jferments"
            / "ostudy"
            / "AndroidDocumentsProviderSmokeTest.kt"
        ).read_text(encoding="utf-8")
        self.assertIn("android.content.action.DOCUMENTS_PROVIDER", test_manifest)
        self.assertIn("class OstudyTestDocumentsProvider : DocumentsProvider()", provider)
        self.assertIn("android-fixture.ostudy.json", provider)
        self.assertIn("android-fixture.png", provider)
        self.assertIn("android-fixture.pdf", provider)
        self.assertIn("startDeckExportPicker", smoke)
        self.assertIn('purpose = "study_content"', smoke)
        self.assertIn('purpose = "image"', smoke)
        self.assertIn('purpose = "pdf_document"', smoke)


if __name__ == "__main__":
    unittest.main()
