#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


def replace_once(text: str, old: str, new: str, path: str) -> str:
    if new in text and old not in text:
        return text
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one occurrence, found {count}: {old[:160]!r}")
    return text.replace(old, new, 1)


path = "src/AppShell.module.css"
text = read(path)
text = text.replace(
    '''  @media (min-width: 761px) and (max-width: 1020px) {
    .shell { grid-template-columns: var(--ostudy-navigation-medium-width) minmax(0, 1fr); }
  }''',
    '''  @media (min-width: 761px) and (max-width: 1180px) {
    .shell { grid-template-columns: clamp(168px, 20vw, 220px) minmax(0, 1fr); }
  }''',
)
write(path, text)

path = "src/components/NavigationSidebar.module.css"
text = read(path)
text = re.sub(
    r'''  @media \(min-width: 761px\) and \(max-width: 1020px\) \{.*?\n  \}\n''',
    '''  @media (min-width: 761px) and (max-width: 1180px) {
    .sidebar { align-items: stretch; gap: var(--ostudy-space-5); padding-inline: 12px; }
    .navButton { justify-content: flex-start; padding-inline: 10px; }
    .navLabel,
    .footer span:last-child { position: static; width: auto; height: auto; overflow: hidden; clip-path: none; }
    .active::before { inset-inline-start: -6px; }
  }

  @media (min-width: 761px) and (max-width: 860px) {
    .sidebar [data-artwork-slot="brand.logo.sidebar"] > span:last-child { display: none; }
    .navLabel { font-size: .76rem; }
    .navButton { gap: var(--ostudy-space-2); }
  }
''',
    text,
    count=1,
    flags=re.S,
)
write(path, text)

path = "src/screens/flashcards/DeckLibraryWorkspace.module.css"
text = read(path)
text = text.replace(
    ".root { display: grid; min-width: 0; gap: var(--ostudy-space-5); }",
    ".root { container-type: inline-size; container-name: deck-library; display: grid; min-width: 0; gap: var(--ostudy-space-5); }",
)
if "@container deck-library" not in text:
    text = text.replace(
        "  @media (max-width: 900px) {",
        '''  @container deck-library (min-width: 620px) and (max-width: 980px) {
    .folderHeader { grid-template-columns: auto minmax(0, 1fr); }
    .folderActions { grid-column: 2; justify-content: flex-start; }
    .deckItem { grid-template-columns: var(--touch-target-min) minmax(0, 1fr); }
    .deckActions { grid-column: 1 / -1; border-block-start: 1px solid var(--ostudy-color-border-subtle); border-inline-start: 0; justify-content: flex-end; }
    .practiceButton { min-width: 130px; }
  }

  :root[data-material="simple"] .deckItem,
  :root[data-material="simple"] .folder { box-shadow: none; backdrop-filter: none; }
  :root[data-material="simple"] .deckItem:hover { transform: none; }

  @media (max-width: 900px) {''',
        1,
    )
write(path, text)

path = "src/screens/flashcards/FlashcardsWorkspace.module.css"
text = read(path)
text = text.replace("  .panel { min-width: 0; }", "  .panel { container-type: inline-size; container-name: flashcards-panel; min-width: 0; }")
if "@container flashcards-panel" not in text:
    text = text.replace(
        "  @media (max-width: 1240px) {",
        '''  @container flashcards-panel (min-width: 620px) and (max-width: 980px) {
    .taskCard { min-height: 210px; }
    .taskActions { align-items: stretch; }
    .taskActions > * { flex: 1 1 180px; }
  }

  @media (max-width: 1240px) {''',
        1,
    )
write(path, text)

# Re-selecting Practice is a root-reset request and therefore safely terminates
# an active session through the same guard as switching to another primary tab.
path = "src/controllers/useAppController.ts"
text = read(path)
text = text.replace(
    '''    if (section !== "practice" || next === "practice" || !hasActivePracticeSession()) {
      return true;
    }''',
    '''    if (section !== "practice" || !hasActivePracticeSession()) {
      return true;
    }''',
)
write(path, text)

write(
    "src/screens/settings/LayoutDiagnosticsCard.tsx",
    r'''import { useMemo } from "react";

import type { ShowToast } from "../../components/Toast";
import styles from "./Settings.module.css";

export function LayoutDiagnosticsCard({ showToast }: { showToast: ShowToast }) {
  const diagnostics = useMemo(buildLayoutDiagnostics, []);

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(diagnostics);
      showToast("success", "Layout diagnostics copied.");
    } catch {
      showToast("error", "Layout diagnostics could not be copied on this platform.");
    }
  }

  return (
    <section className={styles.settingsCard}>
      <div className={styles.sectionHeading}>
        <div>
          <h2>Layout diagnostics</h2>
          <p>Safe viewport and interaction facts for phone, tablet, desktop, and e-ink testing.</p>
        </div>
        <button type="button" className="secondary-button" onClick={() => void copy()}>
          Copy layout diagnostics
        </button>
      </div>
      <pre>{diagnostics}</pre>
    </section>
  );
}

function buildLayoutDiagnostics(): string {
  const visual = window.visualViewport;
  const media = (query: string) => window.matchMedia(query).matches;
  const shellMode = window.innerWidth <= 760 ? "phone" : window.innerWidth <= 1180 ? "tablet" : "wide";
  return [
    "ostudy layout diagnostics",
    `captured_at: ${new Date().toISOString()}`,
    `css_viewport: ${window.innerWidth}x${window.innerHeight}`,
    `visual_viewport: ${visual ? `${Math.round(visual.width)}x${Math.round(visual.height)}` : "unavailable"}`,
    `device_pixel_ratio: ${window.devicePixelRatio}`,
    `orientation: ${screen.orientation?.type ?? "unavailable"}`,
    `shell_mode: ${shellMode}`,
    `pointer_coarse: ${media("(pointer: coarse)")}`,
    `hover_available: ${media("(hover: hover)")}`,
    `reduced_motion: ${media("(prefers-reduced-motion: reduce)")}`,
    `reduced_transparency: ${media("(prefers-reduced-transparency: reduce)")}`,
  ].join("\n");
}
''',
)

path = "src/screens/settings/DiagnosticsSettings.tsx"
text = read(path)
if "LayoutDiagnosticsCard" not in text:
    first_import_end = text.find("\n", text.find("import "))
    insertion = 'import { LayoutDiagnosticsCard } from "./LayoutDiagnosticsCard";\n'
    text = text[: first_import_end + 1] + insertion + text[first_import_end + 1 :]
    closing_candidates = [text.rfind("</div>\n  );"), text.rfind("</>\n  );")]
    closing = max(closing_candidates)
    if closing < 0:
        raise RuntimeError(f"{path}: component return boundary not found")
    text = text[:closing] + "  <LayoutDiagnosticsCard showToast={showToast} />\n      " + text[closing:]
write(path, text)

# Extend the permanent source policy to protect the intermediate tablet mode.
path = "scripts/agent-pr55-test-refresh.py"
text = read(path)
text = text.replace(
    'requireText(!tours.includes("autoStartTimer"), "Tutorials must remain explicitly initiated");',
    'requireText(!tours.includes("autoStartTimer"), "Tutorials must remain explicitly initiated");\nconst shell = read("src/AppShell.module.css");\nrequireText(shell.includes("max-width: 1180px"), "Tablet shell mode is missing");\nconst navigation = read("src/components/NavigationSidebar.module.css");\nrequireText(navigation.includes("max-width: 1180px"), "Tablet navigation rail is missing");',
)
write(path, text)

path = "docs/architecture/ui.md"
text = read(path)
addition = '''

## Phone, tablet, wide, and e-ink behavior

The shell alone selects the primary navigation form: phone uses the app bar and bottom navigation, tablet uses a compact labeled rail, and wide layouts use the full sidebar. Workspaces use container queries for their own cards, toolbars, and lists so an intermediate device does not combine a tablet shell with desktop-only content assumptions.

Simple-material and reduced/no-motion preferences are the e-ink-compatible path: opaque surfaces, no required blur, stable geometry, no hover-only actions, and no information conveyed only through animation. Diagnostics may report viewport dimensions, DPR, orientation, pointer/hover capabilities, and the selected shell mode, but never learner content.
'''
if "## Phone, tablet, wide, and e-ink behavior" not in text:
    text += addition
write(path, text)
