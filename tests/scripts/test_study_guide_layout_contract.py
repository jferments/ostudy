"""Structural layout guards for the live Tailwind document/guide presentation."""
import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def recipes(name):
    source = (ROOT / "src/screens/documents/presentation.ts").read_text(encoding="utf-8")
    match = re.search(r"export const " + name + r" = (\{.*?\}) as const;", source, re.S)
    if match is None:
        raise AssertionError(f"Missing live presentation owner: {name}")
    return json.loads(match.group(1))


class StudyGuideLayoutContractTests(unittest.TestCase):
    def test_document_actions_do_not_compete_with_the_title_column(self) -> None:
        style = recipes("documentStyles")
        self.assertIn("tw:flex-col", style["card"].split())
        self.assertIn("tw:min-w-0", style["cardMain"].split())
        self.assertIn("tw:flex-wrap", style["titleRow"].split())
        self.assertIn("tw:[&_h2]:break-words", style["titleRow"].split())
        self.assertIn("tw:border-t", style["actions"].split())
        self.assertIn("tw:border-0", style["actions"].split())
        self.assertNotIn("tw:flex-nowrap", style["titleRow"].split())
        source = (ROOT / "src/screens/DocumentsScreen.tsx").read_text()
        self.assertIn("ResponsiveActionGroup", source)
        self.assertIn("className={styles.actions}", source)

    def test_guide_toolbar_and_contents_use_opaque_semantic_surfaces(self) -> None:
        style = recipes("guideReaderStyles")
        for name in ("header", "contents"):
            tokens = style[name].split()
            self.assertIn("tw:bg-card", tokens)
            self.assertIn("tw:border-border", tokens)
            self.assertNotIn("tw:bg-transparent", tokens)
        self.assertIn("tw:[&_a:hover]:underline", style["contents"].split())
        self.assertIn("tw:[&_a:focus-visible]:outline-2", style["contents"].split())
        self.assertIn("tw:[&_a:focus-visible]:outline-ring", style["contents"].split())
        self.assertIn("tw:grid-cols-1", style["layout"].split())
        self.assertIn("tw:xl:grid-cols-[14rem_minmax(0,1fr)]", style["layout"].split())
        for stale in ("--surface-color", "--surface-muted-color", "--border-color", "--muted-text-color"):
            self.assertNotIn(stale, json.dumps(style))


if __name__ == "__main__":
    unittest.main()
