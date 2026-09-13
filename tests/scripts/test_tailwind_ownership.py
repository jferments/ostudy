import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERIFIER = ROOT / "scripts/verify-owned-class-usage.mjs"


class TailwindOwnershipTests(unittest.TestCase):
    def verify(self, jsx: str) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for path in (
                "src/ui/foundations/utilities.css",
            ):
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text("", encoding="utf-8")
            (root / "src/Example.tsx").write_text(jsx, encoding="utf-8")
            return subprocess.run(
                ["node", str(VERIFIER), "--root", str(root)],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )

    def test_prefixed_static_utilities_have_framework_ownership(self) -> None:
        result = self.verify(
            'export const Example = () => <div className="tw:flex tw:gap-2 tw:hover:bg-accent" />;'
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_unowned_old_classes_are_still_rejected(self) -> None:
        result = self.verify(
            'export const Example = () => <div className="tw:flex unowned-old-panel" />;'
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unowned-old-panel", result.stderr)

    def test_interpolated_tailwind_names_are_rejected(self) -> None:
        result = self.verify(
            'export const Example = ({color}) => <div className={`tw:bg-${color}`} />;'
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("tw:bg-", result.stderr)

    def test_cn_does_not_hide_unowned_or_interpolated_names(self) -> None:
        result = self.verify(
            'export const Example = ({color}) => <div className={cn("tw:flex", "unowned-panel", `tw:bg-${color}`)} />;'
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unowned-panel", result.stderr)
        self.assertIn("tw:bg-", result.stderr)


if __name__ == "__main__":
    unittest.main()
