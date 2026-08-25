import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERIFIER = ROOT / "scripts" / "verify-style-ownership.mjs"
FINAL_LAYER_ORDER = "@layer reset, tokens, theme, base, components, patterns, features, utilities;\n"
FOUNDATION_IMPORT = {
    "path": "src/main.tsx",
    "specifier": "./ui/foundations/index.css",
}


class StyleOwnershipVerifierTests(unittest.TestCase):
    def run_verifier(self, root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["node", str(VERIFIER), "--root", str(root)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def write_fixture(self, root: Path, baseline: dict) -> None:
        (root / "scripts").mkdir(parents=True)
        (root / "src/ui/foundations").mkdir(parents=True)
        (root / "src").mkdir(exist_ok=True)
        (root / "scripts/style-ownership-baseline.json").write_text(
            json.dumps(baseline), encoding="utf-8"
        )
        (root / "src/ui/foundations/index.css").write_text(
            FINAL_LAYER_ORDER, encoding="utf-8"
        )
        (root / "src/main.tsx").write_text(
            'import "./ui/foundations/index.css";\n', encoding="utf-8"
        )

    def zero_baseline(self) -> dict:
        return {
            "schemaVersion": 1,
            "mode": "zero",
            "legacyFiles": {},
            "allowedGlobalCssImports": [FOUNDATION_IMPORT],
            "policyAllowances": {},
        }

    def test_ratchet_rejects_new_legacy_selector(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(
                root,
                {
                    "schemaVersion": 1,
                    "mode": "ratchet",
                    "legacyFiles": {
                        "src/styles/features/cards.css": {"selectors": ["known"]}
                    },
                    "allowedGlobalCssImports": [FOUNDATION_IMPORT],
                    "policyAllowances": {},
                },
            )
            legacy = root / "src/styles/features/cards.css"
            legacy.parent.mkdir(parents=True)
            legacy.write_text(".known {}\n.new-debt {}\n", encoding="utf-8")

            completed = self.run_verifier(root)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("introduced legacy selector .new-debt", completed.stderr)

    def test_ratchet_allows_legacy_deletion_but_not_new_global_import(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(
                root,
                {
                    "schemaVersion": 1,
                    "mode": "ratchet",
                    "legacyFiles": {
                        "src/styles/features/cards.css": {"selectors": ["known"]}
                    },
                    "allowedGlobalCssImports": [FOUNDATION_IMPORT],
                    "policyAllowances": {},
                },
            )
            (root / "src/feature.tsx").write_text(
                'import "./feature.css";\n', encoding="utf-8"
            )
            (root / "src/feature.css").write_text(".feature {}\n", encoding="utf-8")

            completed = self.run_verifier(root)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("introduced global CSS import", completed.stderr)

    def test_zero_mode_accepts_only_final_foundation_and_module_styles(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root, self.zero_baseline())
            module = root / "src/feature.module.css"
            module.write_text(
                ".feature { color: var(--ostudy-color-text-primary); }\n",
                encoding="utf-8",
            )

            completed = self.run_verifier(root)

            self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_zero_mode_rejects_stale_baseline_allowances_and_global_css(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            baseline = self.zero_baseline()
            baseline["policyAllowances"] = {
                "src/feature.module.css": {
                    "rawColors": 1,
                    "numericZIndexes": 0,
                    "arbitraryDurations": 0,
                }
            }
            self.write_fixture(root, baseline)
            (root / "src/feature.module.css").write_text(
                ".feature { color: var(--ostudy-color-text-primary); }\n",
                encoding="utf-8",
            )
            (root / "src/unowned.css").write_text(".unowned {}\n", encoding="utf-8")

            completed = self.run_verifier(root)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("policyAllowances to be empty", completed.stderr)
            self.assertIn("authored global CSS outside the foundation", completed.stderr)

    def test_zero_mode_rejects_wrong_layer_order_or_missing_foundation_import(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            baseline = self.zero_baseline()
            self.write_fixture(root, baseline)
            (root / "src/ui/foundations/index.css").write_text(
                "@layer reset, tokens, components, utilities;\n",
                encoding="utf-8",
            )
            (root / "src/main.tsx").write_text("export {};\n", encoding="utf-8")

            completed = self.run_verifier(root)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("requires exactly one global TypeScript CSS import", completed.stderr)
            self.assertIn("final cascade exactly", completed.stderr)

    def test_topology_verifier_does_not_text_match_owned_semantic_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(
                root,
                {
                    "schemaVersion": 1,
                    "mode": "ratchet",
                    "legacyFiles": {},
                    "allowedGlobalCssImports": [FOUNDATION_IMPORT],
                    "policyAllowances": {},
                },
            )
            (root / "src/feature.tsx").write_text(
                'const key = "primary-button";\n', encoding="utf-8"
            )

            completed = self.run_verifier(root)

            self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_dynamic_css_policy_rejects_new_raw_color(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root, self.zero_baseline())
            (root / "src/feature.module.css").write_text(
                ".feature { color: #fff; }\n", encoding="utf-8"
            )

            completed = self.run_verifier(root)

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("rawColors", completed.stderr)


if __name__ == "__main__":
    unittest.main()
