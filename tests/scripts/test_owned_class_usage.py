import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERIFIER = ROOT / "scripts" / "verify-owned-class-usage.mjs"


class OwnedClassUsageVerifierTests(unittest.TestCase):
    def run_verifier(self, root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["node", str(VERIFIER), "--root", str(root)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def write_fixture(self, root: Path) -> None:
        owners = {
            "src/ui/patterns/ScreenPatterns.module.css": ".screen {}\n",
            "src/ui/primitives/ControlPatterns.module.css": ".primary-button {}\n",
            "src/ui/overlays/OverlayPatterns.module.css": ".modal-actions {}\n",
            "src/ui/foundations/utilities.css": ".sr-only {}\n",
        }
        for relative_path, content in owners.items():
            path = root / relative_path
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")

    def test_accepts_explicit_module_owner_and_global_utility(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root)
            (root / "src/Feature.module.css").write_text(
                ".feature-card {}\n.confidence-score-0 {}\n",
                encoding="utf-8",
            )
            (root / "src/Feature.tsx").write_text(
                '''import styles from "./Feature.module.css";\n'''
                '''const ownedStyleMaps = [...sharedClassMaps, styles] as const;\n'''
                '''export function Feature() {\n'''
                '''  return <>\n'''
                '''    <div className={ownedClassNames(ownedStyleMaps, `screen feature-card confidence-score-${score}`)} />\n'''
                '''    <span className="sr-only">Label</span>\n'''
                '''    <div className={styles.featureCard} />\n'''
                '''  </>;\n'''
                '''}\n''',
                encoding="utf-8",
            )

            completed = self.run_verifier(root)

            self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_rejects_unowned_semantic_class(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root)
            (root / "src/Feature.module.css").write_text(".feature-card {}\n", encoding="utf-8")
            (root / "src/Feature.tsx").write_text(
                '''import styles from "./Feature.module.css";\n'''
                '''const ownedStyleMaps = [...sharedClassMaps, styles] as const;\n'''
                '''export function Feature() {\n'''
                '''  return <div className={ownedClassNames(ownedStyleMaps, "missing-class")} />;\n'''
                '''}\n''',
                encoding="utf-8",
            )

            completed = self.run_verifier(root)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn('unowned semantic class "missing-class"', completed.stderr)

    def test_rejects_raw_production_class_literal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root)
            (root / "src/Feature.tsx").write_text(
                '''export function Feature() {\n'''
                '''  return <div className="legacy-feature-class" />;\n'''
                '''}\n''',
                encoding="utf-8",
            )

            completed = self.run_verifier(root)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn('emits raw production class "legacy-feature-class"', completed.stderr)


if __name__ == "__main__":
    unittest.main()
