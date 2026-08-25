from __future__ import annotations

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
BUILD_STATE = ROOT / "scripts" / "build-state.sh"
BUILD = ROOT / "scripts" / "build.sh"
CHECK = ROOT / "scripts" / "check.sh"
RUN_APP = ROOT / "run-app.sh"
SMOKE_NATIVE = ROOT / "scripts" / "smoke-native.sh"


class StandaloneBuildArtifactTests(unittest.TestCase):
    def test_launcher_uses_a_staged_artifact_not_mutable_cargo_debug_output(self) -> None:
        source = BUILD_STATE.read_text(encoding="utf-8")
        self.assertIn(
            'OSTUDY_CARGO_BINARY="$OSTUDY_BUILD_TARGET_DIR/debug/ostudy"',
            source,
        )
        self.assertIn(
            'OSTUDY_STANDALONE_DIR="$OSTUDY_BUILD_TARGET_DIR/standalone"',
            source,
        )
        self.assertIn('OSTUDY_BINARY="$OSTUDY_STANDALONE_DIR/ostudy"', source)
        self.assertIn("ostudy_stage_standalone_build()", source)
        self.assertIn("artifactKind=standalone", source)
        self.assertIn("binarySha256=", source)
        self.assertIn("The standalone build artifact changed after it was staged", source)

    def test_build_and_full_check_stage_before_writing_the_build_stamp(self) -> None:
        for path in (BUILD, CHECK):
            source = path.read_text(encoding="utf-8")
            tauri_build = source.index("pnpm tauri build --debug --no-bundle --ci")
            stage = source.index("ostudy_stage_standalone_build", tauri_build)
            stamp = source.index("ostudy_write_build_stamp", stage)
            self.assertLess(tauri_build, stage, path.name)
            self.assertLess(stage, stamp, path.name)

    def test_manual_launcher_and_native_smoke_share_the_same_staged_binary(self) -> None:
        run_app = RUN_APP.read_text(encoding="utf-8")
        smoke = SMOKE_NATIVE.read_text(encoding="utf-8")
        self.assertIn('-- "$OSTUDY_BINARY"', run_app)
        self.assertIn('--application "$OSTUDY_BINARY"', smoke)


if __name__ == "__main__":
    unittest.main()
