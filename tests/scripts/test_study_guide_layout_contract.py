import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOCUMENTS_CSS = ROOT / "src" / "screens" / "DocumentsScreen.module.css"
GUIDE_CSS = ROOT / "src" / "screens" / "StudyGuideDialog.module.css"


class StudyGuideLayoutContractTests(unittest.TestCase):
    def test_document_actions_have_a_bounded_column_that_cannot_crush_the_title(self) -> None:
        css = DOCUMENTS_CSS.read_text(encoding="utf-8")
        self.assertIn(
            "grid-template-columns: minmax(16rem, 1fr) auto;",
            css,
        )
        self.assertIn(".cardMain {\n  min-width: 0;", css)
        self.assertIn(".actions {\n  align-self: center;", css)
        self.assertIn(
            "@media (max-width: 780px) {\n  .card {\n    grid-template-columns: minmax(0, 1fr);",
            css,
        )
        self.assertNotIn("flex: 0 0 auto;", css)

    def test_guide_toolbar_and_contents_use_opaque_current_semantic_surfaces(self) -> None:
        css = GUIDE_CSS.read_text(encoding="utf-8")
        for stale_token in (
            "--surface-color",
            "--surface-muted-color",
            "--border-color",
            "--muted-text-color",
        ):
            self.assertNotIn(stale_token, css)
        self.assertIn(
            "background: var(--ostudy-color-surface-base);\n  box-shadow:",
            css,
        )
        self.assertIn("background: var(--ostudy-color-surface-soft);", css)
        self.assertIn("font-weight: 650;", css)
        self.assertIn("text-decoration: underline;", css)
        self.assertIn(".contents a:focus-visible", css)


if __name__ == "__main__":
    unittest.main()
