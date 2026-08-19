#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


path = "src/controllers/useAppController.ts"
text = read(path)
helper_start = text.find("\nfunction normalizeDestination")
search_end = helper_start if helper_start >= 0 else len(text)
return_start = text.rfind("  return {", 0, search_end)
if return_start < 0:
    raise RuntimeError(f"{path}: useAppController return object not found")
return_end = text.find("\n  };", return_start, search_end)
if return_end < 0:
    raise RuntimeError(f"{path}: useAppController return boundary not found")
return_block = text[return_start:return_end]
if "sectionRootRevision," not in return_block:
    anchor = "    flashcardsWorkspace,\n"
    relative = return_block.find(anchor)
    if relative < 0:
        raise RuntimeError(f"{path}: flashcardsWorkspace return field not found")
    absolute = return_start + relative
    text = text[:absolute] + text[absolute:].replace(
        anchor,
        anchor + "    sectionRootRevision,\n",
        1,
    )
write(path, text)

path = "src-tauri/src/ai/document_provider.rs"
text = read(path)
pattern = r"#\[derive\(([^\)]*)\)\]\n((?:pub(?:\(crate\))?\s+)?)struct DocumentProviderCapabilities"
match = re.search(pattern, text)
if not match:
    raise RuntimeError(f"{path}: DocumentProviderCapabilities derive not found")
derives = [value.strip() for value in match.group(1).split(",")]
if "Clone" not in derives:
    derives.insert(0, "Clone")
    replacement = f"#[derive({', '.join(derives)})]\n{match.group(2)}struct DocumentProviderCapabilities"
    text = text[: match.start()] + replacement + text[match.end() :]
write(path, text)

path = "src-tauri/src/ai/document_intelligence.rs"
text = read(path)
text = re.sub(
    r"providers::openrouter_provider_preferences\(\s*true,\s*profile\.require_zero_data_retention,?\s*\)",
    "providers::openrouter_provider_preferences_with_routing(\n            true,\n            profile.require_zero_data_retention,\n            routing_mode,\n            provider_slugs,\n        )",
    text,
)
text = re.sub(
    r"openrouter_document_body\(\s*&task\.profile,\s*(&?request)\s*\)",
    r"openrouter_document_body_with_routing(\n                &task.openrouter_routing_mode,\n                &task.openrouter_provider_slugs,\n                &task.profile,\n                \1,\n            )",
    text,
)
write(path, text)

path = "src/styles/base/application.css"
text = read(path)
text = text.replace(
    ".card-row-preview > span { display: flex; min-width: 0; align-items: baseline; gap: .45rem; }",
    ".card-row-preview > span { display: flex; width: 100%; max-width: 100%; min-width: 0; align-items: baseline; gap: .45rem; }",
)
write(path, text)

path = "scripts/agent-pr55-test-refresh.py"
text = read(path)
anchor = 'for (const contract of ["DocumentLearningTarget", "document_generation_target_gap_pass_started", "strip_internal_generation_metadata", "prompt_policy_hash"]) {'
if anchor in text:
    text = text.replace(
        anchor,
        'for (const contract of ["DocumentLearningTarget", "document_generation_target_gap_pass_started", "strip_internal_generation_metadata", "prompt_policy_hash", "openrouter_provider_preferences_with_routing"]) {',
        1,
    )
write(path, text)

# Final product-shape normalization. Later responsive/test applicators can
# legitimately touch shared fixtures, so remove compatibility residue only
# after every feature applicator has run.
path = "src/controllers/useAppController.ts"
text = read(path)
text = re.sub(
    r"function normalizeSettings\(settings: AppSettings\): AppSettings \{.*?\n\}\n$",
    '''function normalizeSettings(settings: AppSettings): AppSettings {
  const motionPreference =
    settings.motionPreference ??
    (settings.animationsEnabled === false
      ? "none"
      : settings.reduceMotion
        ? "reduced"
        : "system");
  return {
    theme: settings.theme,
    uiFont: settings.uiFont,
    cardFont: settings.cardFont,
    uiScale: settings.uiScale,
    cardBaseSize: settings.cardBaseSize,
    motionPreference,
    materialPreference: settings.materialPreference ?? "automatic",
    decorativeArtwork: settings.decorativeArtwork ?? true,
    celebrationsEnabled: settings.celebrationsEnabled ?? true,
    tutorialsEnabled: settings.tutorialsEnabled ?? false,
    confirmDeletions: settings.confirmDeletions,
    practiceShuffle: settings.practiceShuffle,
    desiredRetention: settings.desiredRetention,
    dailyNewLimit: settings.dailyNewLimit,
    dailyReviewLimit: settings.dailyReviewLimit,
    aiReasoningEffort: settings.aiReasoningEffort ?? "low",
    aiContextLimitTokens: settings.aiContextLimitTokens ?? null,
    aiImageInputPolicy: settings.aiImageInputPolicy ?? "automatic",
    aiCoverageMode: settings.aiCoverageMode ?? "balanced",
    aiOpenRouterRoutingMode: settings.aiOpenRouterRoutingMode ?? "automatic",
    aiOpenRouterProviderSlugs: settings.aiOpenRouterProviderSlugs ?? [],
    aiPromptPresetId: settings.aiPromptPresetId ?? null,
    aiPromptPresets: settings.aiPromptPresets ?? [],
  };
}
''',
    text,
    flags=re.S,
)
write(path, text)

path = "src/ui/appearance/AppearanceProvider.tsx"
text = read(path).replace("  tutorialsEnabled: true,", "  tutorialsEnabled: false,", 1)
write(path, text)

path = "src/ui/appearance/appearanceTypes.ts"
text = read(path).replace(
    "    tutorialsEnabled: settings.tutorialsEnabled ?? true,",
    "    tutorialsEnabled: settings.tutorialsEnabled ?? false,",
    1,
)
write(path, text)

# Remove the retired deck-density field from all source fixtures and tests,
# including TypeScript `as const` literals and computed compatibility values.
for file in (ROOT / "src").rglob("*"):
    if file.suffix not in {".ts", ".tsx"}:
        continue
    original = file.read_text()
    updated = re.sub(r'^\s*deckViewDensity:.*\n', "", original, flags=re.M)
    updated = re.sub(
        r'^vi\.mock\("\./CreateCardsWorkspace"[^\n]*\);\n',
        "",
        updated,
        flags=re.M,
    )
    if updated != original:
        file.write_text(updated)

path = "src/ui/appearance/settingsEvents.test.ts"
text = read(path)
text = text.replace('makeSettings({ deckViewDensity: "compact" })', 'makeSettings({ uiScale: 105 })')
text = text.replace('makeSettings({ deckViewDensity: "comfortable" })', 'makeSettings({ uiScale: 110 })')
write(path, text)

# The back-navigation test now drives explicit nested-route state instead of
# relying on DOM inspection of a .back-button.
path = "src/screens/flashcards/FlashcardsWorkspace.backNavigation.test.tsx"
text = read(path)
text = text.replace(
    "    DeckLibraryWorkspace: () => {\n"
    '      const [view, setView] = React.useState<"library" | "detail" | "editor">("library");',
    "    DeckLibraryWorkspace: ({ onNestedChange }: { onNestedChange?: (nested: boolean) => void }) => {\n"
    '      const [view, setView] = React.useState<"library" | "detail" | "editor">("library");\n'
    "      React.useEffect(() => {\n"
    '        onNestedChange?.(view !== "library");\n'
    "        return () => onNestedChange?.(false);\n"
    "      }, [onNestedChange, view]);",
    1,
)
write(path, text)

# Remove the obsolete help topic and CSS branches now that compact rows are the
# only deck/card presentation.
path = "src/ui/help/helpRegistry.ts"
text = read(path).replace('  | "flashcards.deckDensity"\n', "", 1)
text = re.sub(
    r'  "flashcards\.deckDensity": \{.*?\n  \},\n(?=  "flashcards\.search\.studyState")',
    "",
    text,
    count=1,
    flags=re.S,
)
write(path, text)

path = "src/styles/base/application.css"
text = read(path)
text = re.sub(r"\n\.card-row-divider \{.*?\n\}\n", "\n", text, count=1, flags=re.S)
text = text.replace(
    '''  .card-row-divider,
  .card-row-side:last-child {
    grid-column: 2;
  }
''',
    "",
    1,
)
write(path, text)

# Rewrite user documentation around the canonical four workspaces, compact
# presentation, deck-local creation, and explicitly started tutorials.
path = "docs/user-guide/flashcards-workspace.md"
text = read(path)
text = text.replace("Its compact heading is followed immediately by five workspace tabs:", "Its compact heading is followed immediately by four workspace tabs:")
text = text.replace("- **Create Cards**\n", "")
text = text.replace(
    "On compact mobile layouts the tabs form a centered two-row arrangement: three tabs on the first row and two tabs on the second. They do not require horizontal scrolling.",
    "On compact layouts the four tabs remain reachable without a separate card-creation workspace. Opening **Decks** while a deck or editor is open returns to the main deck library.",
)
text = re.sub(r"\n### Comfortable and Compact density\n.*?(?=\n## Templates)", "\n", text, count=1, flags=re.S)
text = re.sub(r"\n## Create Cards\n.*$", "\n", text, count=1, flags=re.S)
text = text.replace(
    "Use **Decks** to create decks, open them, start practice, organize them into folders, archive them, and remove them.",
    "Use **Decks** to create decks, open them, add one or many cards, start practice, organize them into folders, archive them, and remove them. Inside a deck, **Bulk add** and **Add flashcard** sit with the card table so the destination is explicit.",
)
write(path, text.rstrip() + "\n")

path = "docs/user-guide/flashcards-organization.md"
text = read(path)
text = text.replace("contains five independently mounted workspaces", "contains four independently mounted workspaces")
text = text.replace("- **Create Cards** provides single-card and bulk-entry authoring.\n", "")
text = text.replace("Unfiled decks", "Unsorted decks").replace("Unfiled", "Unsorted")
text = re.sub(r"\n## Comfortable and compact views\n.*?(?=\n## Search)", "\n", text, count=1, flags=re.S)
text = text.replace(
    "Dragging is not required. Every deck also exposes a **Move** action and explicit up/down controls. Folders have equivalent up/down controls.",
    "Dragging is not required. Keyboard users can focus a folder or deck handle and use Arrow keys, Home, and End to reorder or move it through the same revision-checked native operation.",
)
write(path, text.rstrip() + "\n")

path = "docs/user-guide/getting-started.md"
text = read(path)
text = text.replace(
    "Short contextual tutorials can introduce an unfamiliar workspace the first time its relevant controls are available. They are dismissible, can be restarted from related help, and can be disabled under **Settings → Appearance**.",
    "Interactive tutorials never open automatically. Enable them under **Settings → Appearance**, then start or restart a tutorial explicitly from related Help.",
)
text = re.sub(r"\nUse \*\*Comfortable\*\* density.*?\n", "\n", text, count=1)
text = text.replace(
    "Open a deck and choose **Add card**, or open **Flashcards → Create Cards**. Select the card type that matches the material:",
    "Open a deck and choose **Add flashcard**, or use **Bulk add** beside the card table. Select the card type that matches the material:",
)
write(path, text.rstrip() + "\n")

# Extend the permanent policy verifier so stale density/tutorial documentation
# cannot silently reappear after this one-shot applicator is removed.
path = "scripts/verify-ui-ai-overhaul.mjs"
text = read(path)
anchor = 'requireText(!decks.includes("card-row-divider"), "Obsolete question/answer arrow divider returned");\n'
if anchor in text and "helpRegistry" not in text:
    text = text.replace(
        anchor,
        anchor
        + 'const helpRegistry = read("src/ui/help/helpRegistry.ts");\n'
        + 'requireText(!helpRegistry.includes("deckDensity"), "Retired deck-density help returned");\n'
        + 'const gettingStarted = read("docs/user-guide/getting-started.md");\n'
        + 'requireText(!gettingStarted.includes("Flashcards → Create Cards"), "Retired Create Cards documentation returned");\n',
        1,
    )
write(path, text)

# Layout diagnostics must remain safe in jsdom, older WebViews, and restricted
# embedded browsers where matchMedia is absent.
path = "src/screens/settings/LayoutDiagnosticsCard.tsx"
text = read(path)
text = text.replace(
    '  const media = (query: string) => window.matchMedia(query).matches;',
    '''  const media = (query: string) =>
    typeof window.matchMedia === "function" ? window.matchMedia(query).matches : false;''',
    1,
)
write(path, text)

# Practice now starts with all active decks containing cards selected. Update
# the behavior tests to verify that approved default instead of clicking the
# checkbox and accidentally clearing the scope.
path = "src/screens/PracticeScreen.test.tsx"
text = read(path)
text = text.replace(
    '''async function startAndReveal(): Promise<void> {
  fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
  await screen.findByText("1 cards in the scheduled session");''',
    '''async function startAndReveal(): Promise<void> {
  await screen.findByRole("checkbox", { name: /Dog facts/u, checked: true });
  await screen.findByText("1 cards in the scheduled session");''',
    1,
)
text = text.replace(
    '''    renderPractice();
    fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
    expect(await screen.findByText("1 cards in the scheduled session")).toBeTruthy();''',
    '''    renderPractice();
    expect(await screen.findByRole("checkbox", { name: /Dog facts/u, checked: true })).toBeTruthy();
    expect(await screen.findByText("1 cards in the scheduled session")).toBeTruthy();''',
    1,
)
text = text.replace(
    '''    renderPractice({ decks: [deck, secondDeck] });

    fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
    await waitFor(() => expect(practiceIpc.getPracticeWorkload).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("checkbox", { name: /Cat facts/u }));
    await waitFor(() => expect(practiceIpc.getPracticeWorkload).toHaveBeenCalledTimes(2));''',
    '''    renderPractice({ decks: [deck, secondDeck] });

    await waitFor(() => expect(practiceIpc.getPracticeWorkload).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("checkbox", { name: /Dog facts/u })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: /Cat facts/u })).toBeChecked();
    fireEvent.click(screen.getByRole("checkbox", { name: /Cat facts/u }));
    await waitFor(() => expect(practiceIpc.getPracticeWorkload).toHaveBeenCalledTimes(2));''',
    1,
)
text = text.replace(
    '''    renderPractice({ decks: [noDueDeck] });
    fireEvent.click(await screen.findByRole("checkbox", { name: /Reviewed dog facts/u }));
    fireEvent.click(await screen.findByRole("button", { name: "Study this selection anyway" }));''',
    '''    renderPractice({ decks: [noDueDeck] });
    expect(await screen.findByRole("checkbox", { name: /Reviewed dog facts/u, checked: true })).toBeTruthy();
    fireEvent.click(await screen.findByRole("button", { name: "Study this selection anyway" }));''',
    1,
)
text = text.replace(
    '''    renderPractice();
    fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
    await screen.findByText("1 cards in the scheduled session");

    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));''',
    '''    renderPractice();
    await screen.findByRole("checkbox", { name: /Dog facts/u, checked: true });
    await screen.findByText("1 cards in the scheduled session");

    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));''',
    1,
)
write(path, text)

# Tutorials are explicitly initiated. Cover both the no-popup default and the
# manual Start/Android Back lifecycle rather than preserving contextual timers.
path = "src/ui/tutorials/TourProvider.test.tsx"
write(
    path,
    '''import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
} from "../../platform/backNavigation";
import { startTutorial, TourProvider } from "./TourProvider";

function Anchors() {
  return (
    <div>
      <nav data-tour-anchor="app.navigation">Navigation</nav>
      <div data-tour-anchor="flashcards.workspaces">Workspaces</div>
    </div>
  );
}

describe("TourProvider", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
    expect(platformBackHandlerCountForTests()).toBe(0);
  });

  it("does not open an unfinished tutorial merely because its anchor exists", async () => {
    render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );

    await act(async () => {
      vi.advanceTimersByTime(2_000);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("lets Android Back dismiss a manually started tutorial and records the dismissal", async () => {
    const view = render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );

    await act(async () => {
      startTutorial("orientation");
    });
    expect(
      screen.getByRole("dialog", {
        name: "Everything for flashcards lives together",
      }),
    ).toBeTruthy();
    expect(platformBackHandlerCountForTests()).toBe(1);

    await act(async () => {
      expect(dispatchPlatformBack()).toBe(true);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(window.localStorage.getItem("ostudy.tutorials.v1")).toContain("orientation");
    view.unmount();
  });

  it("ignores an explicit tutorial start while tutorials are disabled", async () => {
    render(
      <TourProvider enabled={false}>
        <Anchors />
      </TourProvider>,
    );
    await act(async () => {
      startTutorial("orientation");
      vi.advanceTimersByTime(2_000);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
''',
)
