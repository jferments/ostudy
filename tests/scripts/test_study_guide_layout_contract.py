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


def class_lists(path):
    """The source text and every quoted Tailwind class list in one file."""
    source = (ROOT / path).read_text(encoding="utf-8")
    return source, [value.split() for value in re.findall(r'"((?:tw:[^"\s]+\s*)+)"', source)]


class StudyGuideLayoutContractTests(unittest.TestCase):
    def test_document_actions_do_not_compete_with_the_title_column(self) -> None:
        source, classes = class_lists("src/screens/DocumentsScreen.tsx")
        # The title column takes the free width and wraps long names; everything
        # beside it keeps its own size, and the rest of the actions wait in one menu.
        title_column = next(tokens for tokens in classes if "tw:flex-1" in tokens and "tw:grid" in tokens)
        self.assertIn("tw:min-w-0", title_column)
        title = next(tokens for tokens in classes if "tw:wrap-anywhere" in tokens)
        self.assertIn("tw:m-0", title)
        row = next(tokens for tokens in classes if "tw:items-center" in tokens and "tw:cursor-grab" in tokens)
        self.assertIn("tw:min-w-0", row)
        self.assertNotIn("tw:flex-nowrap", row)
        self.assertIn("OverflowMenu", source)
        self.assertNotIn("ResponsiveActionGroup", source)

    def test_structured_guide_blocks_stay_bounded_and_use_theme_semantics(self) -> None:
        source, classes = class_lists("src/screens/study-guides/StudyGuideContent.tsx")
        # Long words, formulas and wide tables stay inside the reading column.
        root = next(tokens for tokens in classes if "tw:font-(family-name:--card-font-family)" in tokens)
        for token in ("tw:min-w-0", "tw:break-words", "tw:leading-(--ostudy-font-line-height-reading)"):
            self.assertIn(token, root)
        table_wrap = next(tokens for tokens in classes if "tw:overflow-x-auto" in tokens)
        self.assertIn("tw:max-w-full", table_wrap)
        comparisons = next(tokens for tokens in classes if "tw:bg-card" in tokens and "tw:px-4" in tokens and "tw:py-3" in tokens)
        self.assertNotIn("tw:md:grid-cols-2", comparisons)
        # Blocks draw on semantic surfaces, never literal colors or retired variables.
        used = {token for tokens in classes for token in tokens}
        for token in ("tw:bg-card", "tw:bg-accent", "tw:bg-success-soft", "tw:bg-warning-soft", "tw:border-border"):
            self.assertIn(token, used)
        self.assertNotIn("#", source)
        for stale in ("--surface-color", "--surface-muted-color", "--border-color", "--muted-text-color"):
            self.assertNotIn(stale, source)

    def test_document_reader_bars_and_outline_use_opaque_semantic_surfaces(self) -> None:
        dialog, dialog_classes = class_lists("src/ui/overlays/Dialog.tsx")
        self.assertIn('data-presentation="page"', dialog)
        top_bar = next(tokens for tokens in dialog_classes if "tw:sticky" in tokens and "tw:min-h-14" in tokens)
        for token in ("tw:bg-background", "tw:border-border", "tw:border-b", "tw:border-0", "tw:z-(--ostudy-layer-sticky)"):
            self.assertIn(token, top_bar)
        _, outline_classes = class_lists("src/screens/study-guides/GuideOutline.tsx")
        link = next(tokens for tokens in outline_classes if "tw:min-h-10" in tokens)
        for token in ("tw:focus-visible:ring-3", "tw:focus-visible:ring-ring/50", "tw:coarse:min-h-touch", "tw:reduce-motion:transition-none"):
            self.assertIn(token, link)
        _, page_classes = class_lists("src/screens/study-guides/GuideReaderPages.tsx")
        bottom_bar = next(tokens for tokens in page_classes if "tw:sticky" in tokens and "tw:bottom-3" in tokens)
        for token in ("tw:bg-card", "tw:border-border", "tw:z-(--ostudy-layer-sticky)"):
            self.assertIn(token, bottom_bar)
        for _, classes in (("dialog", dialog_classes), ("outline", outline_classes), ("pages", page_classes)):
            for tokens in classes:
                self.assertNotIn("tw:bg-transparent", tokens)

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
