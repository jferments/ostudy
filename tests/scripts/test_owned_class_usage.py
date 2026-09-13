import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERIFIER = ROOT / "scripts/verify-owned-class-usage.mjs"


class OwnedClassUsageVerifierTests(unittest.TestCase):
    def verify(self, jsx, css=".panel {}"):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            utility = root / "src/ui/foundations/utilities.css"
            utility.parent.mkdir(parents=True)
            utility.write_text(".sr-only {}", encoding="utf-8")
            if css is not None:
                (root / "src/Feature.module.css").write_text(css, encoding="utf-8")
            (root / "src/Feature.tsx").write_text(jsx, encoding="utf-8")
            return subprocess.run(["node", str(VERIFIER), "--root", str(root)],
                                  cwd=ROOT, text=True, capture_output=True, check=False)

    def test_accepts_explicit_module_owner_and_global_utility(self):
        result = self.verify('import styles from "./Feature.module.css"; export const Feature = () => <div className={styles.panel}><span className="sr-only">Label</span></div>;')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_does_not_treat_comparison_values_as_class_names(self):
        result = self.verify('export const Feature = () => <div className={cn("tw:flex", mode === "compact" && "tw:gap-2")} />;')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_missing_module_selector(self):
        result = self.verify('import styles from "./Feature.module.css"; export const Feature = () => <div className={styles.missing} />;')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("no .missing selector", result.stderr)

    def test_rejects_missing_module_file(self):
        result = self.verify('import styles from "./Feature.module.css"; export const Feature = () => <div className={styles.panel} />;', None)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing CSS Module", result.stderr)

    def test_rejects_raw_production_class_literal(self):
        result = self.verify('export const Feature = () => <div className="legacy-panel" />;')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("legacy-panel", result.stderr)

    def test_cannot_restore_retired_class_maps(self):
        result = self.verify('import { ownedClassNames } from "./ui/styles/classOwnership"; export const Feature = () => <div className={ownedClassNames(maps, "panel")} />;')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("retired", result.stderr)

    def test_cn_object_and_concatenation_cannot_hide_dynamic_names(self):
        for expression in ['cn({ "bad-panel": active })', '"tw:bg-" + color']:
            with self.subTest(expression=expression):
                result = self.verify(f'export const Feature = () => <div className={{{expression}}} />;')
                self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
