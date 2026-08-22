from __future__ import annotations

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class AndroidJavaScriptToolchainContractTests(unittest.TestCase):
    def test_android_environment_activates_pinned_node_after_path_assembly(self) -> None:
        source = (ROOT / "scripts" / "android-env.sh").read_text(encoding="utf-8")
        self.assertIn('source "$SCRIPT_DIR/lib/node-toolchain.bash"', source)
        path_index = source.index('export PATH=')
        activation_index = source.index("ostudy_activate_node_toolchain", path_index)
        self.assertGreater(activation_index, path_index)

    def test_android_doctor_requires_exact_javascript_tool_versions(self) -> None:
        source = (ROOT / "scripts" / "android-doctor.sh").read_text(encoding="utf-8")
        self.assertIn("java javac node npm pnpm rustup", source)
        self.assertIn('require_exact_version node "$NODE_VERSION"', source)
        self.assertIn('require_exact_version npm "$NPM_VERSION"', source)
        self.assertIn('require_exact_version pnpm "$PNPM_VERSION"', source)

    def test_android_qualification_entrypoints_share_android_environment(self) -> None:
        for relative in ("scripts/android-test.sh", "scripts/android-emulator-smoke.sh"):
            source = (ROOT / relative).read_text(encoding="utf-8")
            self.assertIn('source "$SCRIPT_DIR/android-env.sh"', source, relative)

    def test_vitest_workers_disable_node_process_web_storage(self) -> None:
        config = (ROOT / "vitest.config.mjs").read_text(encoding="utf-8")
        setup = (ROOT / "src" / "test" / "setup.ts").read_text(encoding="utf-8")
        self.assertIn('execArgv: ["--no-experimental-webstorage"]', config)
        self.assertIn('assertBrowserStorage(window.localStorage, "localStorage")', setup)
        self.assertIn('assertBrowserStorage(window.sessionStorage, "sessionStorage")', setup)


if __name__ == "__main__":
    unittest.main()
