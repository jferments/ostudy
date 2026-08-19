#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text)


def replace_once(text: str, old: str, new: str, path: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one occurrence, found {count}: {old[:120]!r}")
    return text.replace(old, new, 1)


def sub_once(text: str, pattern: str, repl: str, path: str, flags: int = 0) -> str:
    new, count = re.subn(pattern, repl, text, count=1, flags=flags)
    if count != 1:
        raise RuntimeError(f"{path}: expected regex once, found {count}: {pattern[:120]!r}")
    return new


path = "src/navigation/flashcardsRoutes.ts"
text = read(path)
text = replace_once(text, '  "transfer",\n  "create",\n', '  "transfer",\n', path)
write(path, text)

path = "src/screens/flashcards/FlashcardsWorkspace.tsx"
text = read(path)
text = text.replace("  useEffect,\n  useLayoutEffect,\n", "  useEffect,\n")
text = text.replace("  useLayoutEffect,\n", "")
text = text.replace('import { CreateCardsWorkspace } from "./CreateCardsWorkspace";\n', "")
text = replace_once(text, "  showToast: ShowToast;\n}", "  showToast: ShowToast;\n  rootRevision?: number;\n}", path)
text = replace_once(
    text,
    '  { value: "transfer", label: "Import/Export", compactLabel: "Transfer", icon: "import" },\n  { value: "create", label: "Create Cards", compactLabel: "Create", icon: "plus" },\n',
    '  { value: "transfer", label: "Import/Export", compactLabel: "Transfer", icon: "import" },\n',
    path,
)
text = replace_once(
    text,
    "  showToast,\n}: FlashcardsWorkspaceProps) {\n  const [deckNested, setDeckNested] = useState(false);",
    "  showToast,\n  rootRevision = 0,\n}: FlashcardsWorkspaceProps) {\n  const [deckNested, setDeckNested] = useState(false);",
    path,
)
text = sub_once(
    text,
    r'''\n  useLayoutEffect\(\(\) => \{\n    workspaceChangeRef\.current = onWorkspaceChange;\n  \}, \[onWorkspaceChange\]\);\n\n  useLayoutEffect\(\(\) => \{.*?\n  \}, \[workspace\]\);\n''',
    '''\n  useEffect(() => {\n    workspaceChangeRef.current = onWorkspaceChange;\n  }, [onWorkspaceChange]);\n\n  useEffect(() => {\n    if (workspace !== "decks") setDeckNested(false);\n  }, [workspace]);\n''',
    path,
    flags=re.S,
)
text = replace_once(
    text,
    '<RouteTransition routeKey={workspace} relation="sibling" className={styles.panel}>',
    '<RouteTransition routeKey={`${workspace}:${rootRevision}`} relation="sibling" className={styles.panel}>',
    path,
)
text = replace_once(
    text,
    '              onManageTemplates={() => onWorkspaceChange("templates")}\n              onCreateCards={() => onWorkspaceChange("create")}\n              onCardDirtyChange={onCardDirtyChange}',
    '              onManageTemplates={() => onWorkspaceChange("templates")}\n              onNestedChange={setDeckNested}\n              onCardDirtyChange={onCardDirtyChange}',
    path,
)
text = sub_once(
    text,
    r'''\n          \{workspace === "create" \? \(\n            <CreateCardsWorkspace.*?\n          \) : null\}\n''',
    "\n",
    path,
    flags=re.S,
)
write(path, text)

path = "src/controllers/useAppController.ts"
text = read(path)
text = replace_once(
    text,
    '  const [flashcardsWorkspace, setFlashcardsWorkspace] =\n    useState<FlashcardsWorkspaceName>("decks");',
    '  const [flashcardsWorkspace, setFlashcardsWorkspace] =\n    useState<FlashcardsWorkspaceName>("decks");\n  const [sectionRootRevision, setSectionRootRevision] = useState(0);',
    path,
)
text = text.replace("  tutorialsEnabled: true,\n", "  tutorialsEnabled: false,\n")
text = text.replace('  deckViewDensity: "comfortable",\n', "")
text = replace_once(
    text,
    '          const restoredWorkspace =\n            restoredSection === "decks"\n              ? restoredCheckpoint.flashcardsWorkspace ??\n                restored.flashcardsWorkspace ??\n                "decks"\n              : "decks";',
    '          const checkpointWorkspace = restoredCheckpoint.flashcardsWorkspace as\n            | FlashcardsWorkspaceName\n            | "create"\n            | undefined;\n          const restoredWorkspace =\n            restoredSection === "decks"\n              ? checkpointWorkspace === "create"\n                ? "decks"\n                : checkpointWorkspace ?? restored.flashcardsWorkspace ?? "decks"\n              : "decks";',
    path,
)
text = replace_once(
    text,
    '    if (alreadyThere && destination.section !== "practice") return;\n    if (!(await confirmPracticeTermination(destination.section))) return;\n    if (!(await canLeaveCurrentSection())) return;',
    '    if (alreadyThere && destination.section !== "practice") {\n      if (!(await canLeaveCurrentSection())) return;\n      if (destination.section === "decks") {\n        setFlashcardsWorkspace(destination.flashcardsWorkspace ?? "decks");\n        setDeckOpenRequest(null);\n        void refreshDecks().catch((error: unknown) =>\n          showToast("error", getErrorMessage(error)),\n        );\n      }\n      if (destination.section === "analytics") {\n        const requestId = analyticsRequestSequence.current;\n        analyticsRequestSequence.current += 1;\n        setAnalyticsLaunch({ requestId, range: "7d", deckIds: [], tagIds: [] });\n      }\n      setSectionRootRevision((current) => current + 1);\n      return;\n    }\n    if (!(await confirmPracticeTermination(destination.section))) return;\n    if (!(await canLeaveCurrentSection())) return;',
    path,
)
text = replace_once(
    text,
    '    setSection(destination.section);\n  }\n\n  async function navigateFlashcardsWorkspace(',
    '    setSection(destination.section);\n    setSectionRootRevision((current) => current + 1);\n  }\n\n  async function navigateFlashcardsWorkspace(',
    path,
)
text = replace_once(
    text,
    '    if (section === "decks" && next === flashcardsWorkspace) return;\n    if (!(await canLeaveCurrentSection())) return;\n    setSection("decks");\n    setFlashcardsWorkspace(next);\n    if (next === "decks" || next === "search" || next === "create") {',
    '    if (!(await canLeaveCurrentSection())) return;\n    setSection("decks");\n    setFlashcardsWorkspace(next);\n    setDeckOpenRequest(null);\n    setSectionRootRevision((current) => current + 1);\n    if (next === "decks" || next === "search") {',
    path,
)
text = replace_once(text, "    flashcardsWorkspace,\n    decks,", "    flashcardsWorkspace,\n    sectionRootRevision,\n    decks,", path)
write(path, text)

path = "src/App.tsx"
text = read(path)
text = replace_once(text, "                  routeKey={controller.section}", "                  routeKey={`${controller.section}:${controller.sectionRootRevision}`}", path)
text = replace_once(
    text,
    "                          showToast={controller.showToast}\n                        />",
    "                          showToast={controller.showToast}\n                          rootRevision={controller.sectionRootRevision}\n                        />",
    path,
)
text = text.replace(
    "  const tutorialsEnabled = controller.activeSettings.tutorialsEnabled ?? true;",
    "  const tutorialsEnabled = controller.activeSettings.tutorialsEnabled ?? false;",
)
write(path, text)

path = "src/screens/flashcards/DeckLibraryWorkspace.tsx"
text = read(path)
text = text.replace('import { saveSettings, type AppSettings, type DeckViewDensity } from "../../ipc/settings";\n', 'import type { AppSettings } from "../../ipc/settings";\n')
text = text.replace('import { Progress } from "../../ui/primitives/Progress";\n', "")
text = replace_once(
    text,
    "  onManageTemplates: () => void | Promise<void>;\n  onCreateCards: () => void | Promise<void>;\n  onCardDirtyChange: (dirty: boolean) => void;",
    "  onManageTemplates: () => void | Promise<void>;\n  onNestedChange?: (nested: boolean) => void;\n  onCardDirtyChange: (dirty: boolean) => void;",
    path,
)
text = replace_once(text, "  onManageTemplates,\n  onCreateCards,\n  onCardDirtyChange,", "  onManageTemplates,\n  onNestedChange,\n  onCardDirtyChange,", path)
text = sub_once(text, r'''\n  const \[density, setDensity\] = useState<DeckViewDensity>\(\n    settings\.deckViewDensity \?\? "comfortable",\n  \);''', "", path)
text = sub_once(text, r'''\n  useEffect\(\(\) => \{\n    setDensity\(settings\.deckViewDensity \?\? "comfortable"\);\n  \}, \[settings\.deckViewDensity\]\);''', "", path)
text = sub_once(text, r'''\n  async function changeDensity\(next: DeckViewDensity\): Promise<void> \{.*?\n  \}\n''', "\n", path, flags=re.S)
anchor = "  function toggleFolder(folderId: string): void {"
if anchor not in text:
    raise RuntimeError(f"{path}: toggleFolder anchor missing")
text = text.replace(
    anchor,
    '  useEffect(() => {\n    onNestedChange?.(detailRequest !== null);\n    return () => onNestedChange?.(false);\n  }, [detailRequest, onNestedChange]);\n\n' + anchor,
    1,
)
text = sub_once(text, r'''\n          <div className=\{styles\.density\} role="group" aria-label="Deck list density">.*?\n          </div>''', "", path, flags=re.S)
text = text.replace('<div className={styles.library} data-density={density}>', '<div className={styles.library}>')
text = re.sub(r"\n\s+density=\{density\}", "", text)
text = text.replace('                      density="compact"\n', "")
text = text.replace("  density,\n", "")
text = text.replace("  density: DeckViewDensity;\n", "")
text = text.replace("      data-density={density}\n", "")
text = text.replace("  const progressMax = Math.max(1, deck.generatedCardCount);\n  const completed = Math.max(0, deck.generatedCardCount - deck.studyableDueCount);\n", "")
text = sub_once(text, r'''\n        \{density === "comfortable" && !archived \? \(\n          <Progress.*?\n        \) : null\}''', "", path, flags=re.S)
write(path, text)

path = "src/screens/flashcards/DeckLibraryWorkspace.module.css"
text = read(path)
text = re.sub(r'\n  \.density \{[^\n]*\}\n  \.density > button \{[^\n]*\}\n  \.density > button\[aria-pressed="true"\] \{[^\n]*\}\n', "\n", text)
text = text.replace("  .dragHandle { width: 38px; border-inline-end: 1px solid var(--ostudy-color-border-subtle); }", "  .dragHandle { width: 38px; }")
text = text.replace(
    "  .deckMain { display: grid; min-width: 0; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: var(--ostudy-space-3); border: 0; padding: var(--ostudy-space-3); background: transparent; color: var(--ostudy-color-text-primary); text-align: start; }",
    "  .deckMain { display: grid; min-width: 0; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: var(--ostudy-space-3); border: 0; padding: 8px var(--ostudy-space-3); background: transparent; color: var(--ostudy-color-text-primary); text-align: start; }",
)
text = text.replace("  .deckMark { display: grid; width: 36px; height: 36px;", "  .deckMark { display: grid; width: 30px; height: 30px;")
text = text.replace("  .deckCopy span { color: var(--ostudy-color-text-secondary); font-size: .8rem; }", "  .deckCopy span { display: none; color: var(--ostudy-color-text-secondary); font-size: .8rem; }")
text = re.sub(r'\n  \.deckItem\[data-density="compact"\].*?\n  \.deckItem\[data-density="compact"\].*?\n  \.deckItem\[data-density="compact"\].*?\n', "\n", text)
text = text.replace("    .density { flex: 2 1 250px; }\n    .density > button { flex: 1; }\n", "")
write(path, text)

path = "src/screens/DecksScreen.tsx"
text = read(path)
text = text.replace('import { CardEditorScreen } from "./CardEditorScreen";\n', 'import { CardEditorScreen } from "./CardEditorScreen";\nimport { BulkEntryModal } from "./cards/BulkEntryModal";\n')
text = text.replace("  onBulkAdd?: () => void;\n", "")
text = text.replace("  onBulkAdd,\n", "")
text = replace_once(text, '  const [searchDialogOpen, setSearchDialogOpen] = useState(false);', '  const [searchDialogOpen, setSearchDialogOpen] = useState(false);\n  const [bulkEntryOpen, setBulkEntryOpen] = useState(false);', path)
text = sub_once(text, r'''\n              \{onBulkAdd \? \(\n                <button.*?\n              \) : null\}''', "", path, flags=re.S)
text = sub_once(
    text,
    r'''\n              <button\n                type="button"\n                className="primary-button"\n                onClick=\{\(\) => \{\n                  setEditingCard\(null\);\n                  setView\("editor"\);\n                \}\}\n              >\n                Add flashcard\n              </button>''',
    "",
    path,
)
needle = '''              <span className="card-count-summary">\n                {cardTotalCount} {cardTotalCount === 1 ? "Card" : "Cards"}\n                {selectedDeck.studyableDueCount > 0\n                  ? ` · ${selectedDeck.studyableDueCount} ready now`\n                  : ""}\n              </span>'''
replacement = needle + '''\n              <div className="card-browser-create-actions">\n                <button type="button" className="secondary-button" onClick={() => setBulkEntryOpen(true)}>\n                  Bulk add\n                </button>\n                <button\n                  type="button"\n                  className="primary-button"\n                  onClick={() => { setEditingCard(null); setView("editor"); }}\n                >\n                  Add flashcard\n                </button>\n              </div>'''
text = replace_once(text, needle, replacement, path)
text = replace_once(
    text,
    '''                <button\n                  type="button"\n                  className="primary-button"\n                  onClick={() => {\n                    setEditingCard(null);\n                    setView("editor");\n                  }}\n                >\n                  Add the first flashcard\n                </button>''',
    '''                <div className="empty-actions">\n                  <button type="button" className="secondary-button" onClick={() => setBulkEntryOpen(true)}>\n                    Bulk add\n                  </button>\n                  <button\n                    type="button"\n                    className="primary-button"\n                    onClick={() => { setEditingCard(null); setView("editor"); }}\n                  >\n                    Add the first flashcard\n                  </button>\n                </div>''',
    path,
)
text = replace_once(
    text,
    '''                      <span className="card-number">{index + 1}</span>\n                      <span className="card-row-side">\n                        <small>Question</small>\n                        <strong>{summarize(card.questionExcerpt, card.imageCount > 0)}</strong>\n                      </span>\n                      <span className="card-row-divider" aria-hidden="true">\n                        →\n                      </span>\n                      <span className="card-row-side">\n                        <small>Answer</small>\n                        <strong>{summarize(card.answerExcerpt, card.imageCount > 0)}</strong>\n                      </span>''',
    '''                      <span className="sr-only">Card {index + 1}. </span>\n                      <span className="card-row-preview">\n                        <span><small aria-hidden="true">Q:</small><strong>{summarize(card.questionExcerpt, card.imageCount > 0)}</strong></span>\n                        <span><small aria-hidden="true">A:</small><strong>{summarize(card.answerExcerpt, card.imageCount > 0)}</strong></span>\n                      </span>''',
    path,
)
needle = '''      {searchDialogOpen ? (\n        <Modal'''
text = replace_once(
    text,
    needle,
    '''      {bulkEntryOpen && selectedDeck ? (\n        <BulkEntryModal\n          decks={decks}\n          defaultDeckId={selectedDeck.id}\n          destinationMode="fixed"\n          onClose={() => setBulkEntryOpen(false)}\n          onSaved={async () => {\n            setBulkEntryOpen(false);\n            await refreshSelectedDeckAfterMutation(selectedDeck.id);\n          }}\n          showToast={showToast}\n        />\n      ) : null}\n\n''' + needle,
    path,
)
text = replace_once(text, '  return normalized.length > 150 ? `${normalized.slice(0, 147)}…` : normalized;', "  return normalized;", path)
write(path, text)

path = "src/screens/cards/BulkEntryModal.tsx"
text = read(path)
text = replace_once(text, "  defaultDeckId?: string;\n  onClose: () => void;", '  defaultDeckId?: string;\n  destinationMode?: "selectable" | "fixed";\n  onClose: () => void;', path)
text = replace_once(text, "  defaultDeckId,\n  onClose,", '  defaultDeckId,\n  destinationMode = "selectable",\n  onClose,', path)
text = replace_once(
    text,
    '''          <label>\n            <span>Destination deck</span>\n            <select\n              value={deckId}\n              disabled={isSaving}\n              onChange={(event: ChangeEvent<HTMLSelectElement>) => {\n                setDeckId(event.currentTarget.value);\n                setErrorMessage(null);\n                clearAnalysis();\n              }}\n            >\n              {activeDecks.map((deck) => (\n                <option key={deck.id} value={deck.id}>\n                  {deck.name}\n                </option>\n              ))}\n            </select>\n          </label>''',
    '''          {destinationMode === "fixed" ? (\n            <div className="bulk-entry-fixed-destination">\n              <span>Destination deck</span>\n              <strong>{activeDecks.find((deck) => deck.id === deckId)?.name ?? "Current deck"}</strong>\n            </div>\n          ) : (\n            <label>\n              <span>Destination deck</span>\n              <select\n                value={deckId}\n                disabled={isSaving}\n                onChange={(event: ChangeEvent<HTMLSelectElement>) => {\n                  setDeckId(event.currentTarget.value);\n                  setErrorMessage(null);\n                  clearAnalysis();\n                }}\n              >\n                {activeDecks.map((deck) => (\n                  <option key={deck.id} value={deck.id}>{deck.name}</option>\n                ))}\n              </select>\n            </label>\n          )}''',
    path,
)
write(path, text)

path = "src/styles/base/application.css"
text = read(path)
if "container-name: deck-card-browser" not in text:
    text += '''\n\n/* Compact deck-detail card previews respond to the panel width. */\n.card-browser-panel { container-type: inline-size; container-name: deck-card-browser; }\n.card-browser-create-actions { display: flex; align-items: center; justify-content: flex-end; gap: .55rem; margin-inline-start: auto; }\n.card-row-main { grid-template-columns: minmax(0, 1fr); }\n.card-row-preview { display: grid; min-width: 0; gap: .28rem; }\n.card-row-preview > span { display: flex; min-width: 0; align-items: baseline; gap: .45rem; }\n.card-row-preview small { flex: 0 0 auto; color: var(--accent); font-size: .75rem; font-weight: 850; }\n.card-row-preview strong { min-width: 0; overflow: hidden; font-weight: 620; text-overflow: ellipsis; white-space: nowrap; }\n.bulk-entry-fixed-destination { display: grid; gap: .25rem; }\n.bulk-entry-fixed-destination span { color: var(--muted); font-size: .75rem; font-weight: 760; }\n@container deck-card-browser (max-width: 700px) {\n  .card-browser-toolbar { grid-template-columns: auto minmax(0, 1fr); }\n  .card-count-summary { grid-column: 1 / -1; }\n  .card-browser-create-actions { grid-column: 1 / -1; width: 100%; }\n  .card-browser-create-actions > button { flex: 1 1 0; }\n  .card-row { grid-template-columns: minmax(0, 1fr) auto; }\n  .card-row-actions { border-top: 0; border-left: 1px solid var(--border); }\n}\n'''
write(path, text)

path = "src/screens/flashcards/TransferWorkspace.tsx"
text = read(path)
text = text.replace('import { Artwork } from "../../ui/artwork/Artwork";\n', "")
text = sub_once(text, r'''\n      <Surface level=\{1\} material="soft" className=\{styles\.transferHelp\}>.*?\n      </Surface>''', "", path, flags=re.S)
write(path, text)

path = "src/screens/PracticeScreen.tsx"
text = read(path)
text = replace_once(
    text,
    '  const [selectedDeckIds, setSelectedDeckIds] = useState<Set<string>>(new Set());',
    '  const [selectedDeckIds, setSelectedDeckIds] = useState<Set<string>>(\n    () => new Set(activeDecks.filter((deck) => deck.generatedCardCount > 0).map((deck) => deck.id)),\n  );',
    path,
)
text = replace_once(
    text,
    "  const [workload, setWorkload] = useState<PracticeWorkloadOverview | null>(null);\n  const [isLoadingWorkload, setIsLoadingWorkload] = useState(false);",
    '  const [workload, setWorkload] = useState<PracticeWorkloadOverview | null>(null);\n  const [workloadStatus, setWorkloadStatus] = useState<"idle" | "loading" | "loaded" | "error">("idle");\n  const [isLoadingWorkload, setIsLoadingWorkload] = useState(false);',
    path,
)
text = replace_once(text, '      setWorkload(null);\n      setIsLoadingWorkload(false);\n      return;\n    }\n    setIsLoadingWorkload(true);', '      setWorkload(null);\n      setWorkloadStatus("idle");\n      setIsLoadingWorkload(false);\n      return;\n    }\n    setWorkloadStatus("loading");\n    setIsLoadingWorkload(true);', path)
text = replace_once(text, '      setWorkload(loaded);\n      setErrorMessage(null);\n      setIsLoadingWorkload(false);', '      setWorkload(loaded);\n      setWorkloadStatus("loaded");\n      setErrorMessage(null);\n      setIsLoadingWorkload(false);', path)
text = replace_once(text, '      setWorkload(null);\n      setErrorMessage(getErrorMessage(error));\n      setIsLoadingWorkload(false);', '      setWorkload(null);\n      setWorkloadStatus("error");\n      setErrorMessage(getErrorMessage(error));\n      setIsLoadingWorkload(false);', path)
text = replace_once(text, "        isLoadingWorkload={isLoadingWorkload}\n        workload={workload}", "        isLoadingWorkload={isLoadingWorkload}\n        workloadStatus={workloadStatus}\n        workload={workload}", path)
write(path, text)

path = "src/screens/practice/PracticeSetup.tsx"
text = read(path)
text = replace_once(text, "  isLoadingWorkload: boolean;\n  workload: PracticeWorkloadOverview | null;", '  isLoadingWorkload: boolean;\n  workloadStatus: "idle" | "loading" | "loaded" | "error";\n  workload: PracticeWorkloadOverview | null;', path)
text = replace_once(text, "  isLoadingWorkload,\n  workload,", "  isLoadingWorkload,\n  workloadStatus,\n  workload,", path)
text = sub_once(
    text,
    r'''      <section className="practice-workload-panel" aria-label="Practice workload" aria-live="polite">.*?      </section>\n\n      <section className="recent-practice-section"''',
    '''      <section className="practice-workload-panel" aria-label="Practice workload" aria-live="polite">\n        {workloadStatus === "idle" ? (\n          <p className="practice-workload-state">Select at least one deck to calculate a practice workload.</p>\n        ) : workloadStatus === "loading" ? (\n          <p className="practice-workload-state">Calculating the selected workload…</p>\n        ) : workloadStatus === "error" ? (\n          <p className="practice-workload-state">The selected workload could not be calculated.</p>\n        ) : workload ? (\n          <>\n            <div><span>Today</span><strong>{workload.reviewsRemaining}</strong><small>reviews remaining</small></div>\n            <div><span>New</span><strong>{workload.newRemaining}</strong><small>cards remaining</small></div>\n            <div><span>Ready now</span><strong>{workload.studyableDueReviews + workload.studyableDueNew}</strong><small>studyable cards</small></div>\n            <div><span>Priority</span><strong>{workload.needsMorePractice}</strong><small>need more practice</small></div>\n          </>\n        ) : null}\n        {isLoadingWorkload ? <span className="workload-loading">Updating…</span> : null}\n      </section>\n\n      <section className="recent-practice-section"''',
    path,
    flags=re.S,
)
write(path, text)

path = "src/ui/tutorials/TourProvider.tsx"
text = read(path)
text = text.replace("  tutorialModuleOrder,\n", "")
text = re.sub(r"\nconst CONTEXTUAL_AUTO_START_ENABLED =.*?;\n", "\n", text, flags=re.S)
text = text.replace("  const autoStartTimer = useRef<number | null>(null);\n", "")
text = sub_once(text, r'''\n  useEffect\(\(\) => \{\n    if \(!enabled \|\| active \|\| !CONTEXTUAL_AUTO_START_ENABLED\) return;.*?\n  \}, \[active, enabled, start, stored\.completed, stored\.dismissed\]\);\n''', "\n", path, flags=re.S)
write(path, text)

path = "src/screens/settings/AppearanceSettings.tsx"
text = read(path)
text = text.replace("<strong>Contextual tutorials</strong>", "<strong>Interactive tutorials</strong>")
text = text.replace("<small>Offer short, dismissible walkthroughs for unfamiliar workflows.</small>", "<small>Allow tutorials that you explicitly start from Help. They never open automatically.</small>")
text = text.replace("checked={draft.tutorialsEnabled ?? true}", "checked={draft.tutorialsEnabled ?? false}")
write(path, text)

path = "src/screens/SettingsScreen.tsx"
text = read(path)
text = text.replace("  tutorialsEnabled: true,\n", "  tutorialsEnabled: false,\n")
text = text.replace('  deckViewDensity: "comfortable",\n', "")
text = text.replace("        deckViewDensity: DEFAULTS.deckViewDensity,\n", "")
write(path, text)

path = "src/ipc/settings.ts"
text = read(path)
text = text.replace('export type DeckViewDensity = "comfortable" | "compact";\n', "")
text = text.replace("  deckViewDensity?: DeckViewDensity;\n", "")
write(path, text)

path = "src-tauri/src/settings.rs"
text = read(path)
text = text.replace('const DECK_VIEW_DENSITIES: &[&str] = &["comfortable", "compact"];\n', "")
text = text.replace("    pub deck_view_density: String,\n", "")
text = text.replace('        tutorials_enabled: get_setting(connection, "tutorialsEnabled", true)?,\n        deck_view_density: get_setting(connection, "deckViewDensity", "comfortable".to_string())?,', '        tutorials_enabled: get_setting(connection, "tutorialsEnabled", false)?,')
text = sub_once(text, r'''        \(\n            "deckViewDensity",\n            serde_json::to_string\(&settings\.deck_view_density\),\n        \),\n''', "", path)
text = text.replace("            \"DELETE FROM app_settings WHERE key IN ('animationsEnabled', 'reduceMotion')\",", "            \"DELETE FROM app_settings WHERE key IN ('animationsEnabled', 'reduceMotion', 'deckViewDensity')\",")
text = sub_once(text, r'''    validate_choice\(\n        &settings\.deck_view_density,\n        DECK_VIEW_DENSITIES,\n        "invalid_deck_view_density",\n        "Select an available deck-list density\.",\n    \)\?;\n''', "", path)
text = re.sub(r"^.*deck_view_density.*\n", "", text, flags=re.M)
write(path, text)

for base in [ROOT / "src", ROOT / "src-tauri" / "src"]:
    for file in base.rglob("*"):
        if file.suffix not in {".ts", ".tsx", ".rs"}:
            continue
        content = file.read_text()
        updated = re.sub(r'^\s*deckViewDensity:\s*"(?:comfortable|compact)",\n', "", content, flags=re.M)
        updated = re.sub(r'^\s*"deckViewDensity":\s*"(?:comfortable|compact)",?\n', "", updated, flags=re.M)
        if updated != content:
            file.write_text(updated)

path = "src/screens/flashcards/FlashcardsWorkspace.module.css"
text = read(path)
text = text.replace("    .tabList { display: grid; width: 100%; min-width: 0; grid-template-columns: repeat(6, minmax(0, 1fr));", "    .tabList { display: grid; width: 100%; min-width: 0; grid-template-columns: repeat(4, minmax(0, 1fr));")
text = text.replace("    .tab { grid-column: span 2;", "    .tab { grid-column: span 1;")
text = text.replace("    .tab:nth-child(4) { grid-column: 2 / span 2; }\n    .tab:nth-child(5) { grid-column: 4 / span 2; }\n", "")
write(path, text)

(ROOT / "src/screens/flashcards/CreateCardsWorkspace.tsx").unlink()
