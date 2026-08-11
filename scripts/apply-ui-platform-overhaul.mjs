import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const file = (...parts) => path.join(root, ...parts);
const read = (relative) => fs.readFileSync(file(relative), "utf8");
const write = (relative, content) => {
  const destination = file(relative);
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.writeFileSync(destination, content.endsWith("\n") ? content : `${content}\n`);
};

function replaceRequired(source, search, replacement, label) {
  if (!source.includes(search)) {
    throw new Error(`Could not apply ${label}: expected source text was not found.`);
  }
  return source.replace(search, replacement);
}

function replaceRegexRequired(source, pattern, replacement, label) {
  if (!pattern.test(source)) {
    throw new Error(`Could not apply ${label}: expected source pattern was not found.`);
  }
  pattern.lastIndex = 0;
  return source.replace(pattern, replacement);
}

function insertImport(source, importLine) {
  if (source.includes(importLine)) return source;
  const imports = [...source.matchAll(/^import[\s\S]*?;\n/gm)];
  if (imports.length === 0) return `${importLine}\n${source}`;
  const last = imports.at(-1);
  const index = (last?.index ?? 0) + (last?.[0].length ?? 0);
  return `${source.slice(0, index)}${importLine}\n${source.slice(index)}`;
}

function ensureReactNames(source, names) {
  const importPattern = /import\s+\{([^}]+)\}\s+from\s+"react";/;
  const match = source.match(importPattern);
  if (!match) {
    return `import { ${names.join(", ")} } from "react";\n${source}`;
  }
  const current = match[1]
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
  for (const name of names) if (!current.includes(name)) current.push(name);
  return source.replace(importPattern, `import { ${current.join(", ")} } from "react";`);
}

function matchingBrace(source, openIndex) {
  let depth = 0;
  let quote = null;
  let escaped = false;
  for (let index = openIndex; index < source.length; index += 1) {
    const character = source[index];
    if (quote) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === quote) quote = null;
      continue;
    }
    if (character === '"' || character === "'" || character === "`") {
      quote = character;
      continue;
    }
    if (character === "{") depth += 1;
    if (character === "}") {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  throw new Error("Could not match source brace.");
}

function interfaceBlock(source, predicate) {
  const pattern = /export interface\s+(\w+)\s*\{/g;
  for (const match of source.matchAll(pattern)) {
    const open = source.indexOf("{", match.index);
    const close = matchingBrace(source, open);
    const block = source.slice(match.index, close + 1);
    if (predicate(block, match[1])) return { name: match[1], start: match.index, open, close, block };
  }
  return null;
}

function patchNavigation() {
  write(
    "src/app/navigation.ts",
    `import type { AppSection } from "../controllers/useAppController";

export interface AppNavigationItem {
  section: AppSection;
  icon: string;
  label: string;
}

/**
 * Product navigation is platform-neutral. Platform shells may present these
 * destinations as a sidebar, bottom bar, command palette, or native menu.
 * Card search and template management intentionally remain nested in Decks.
 */
export const APP_NAVIGATION: ReadonlyArray<AppNavigationItem> = [
  { section: "decks", icon: "▤", label: "Decks" },
  { section: "documents", icon: "▣", label: "Documents" },
  { section: "practice", icon: "▶", label: "Practice" },
  { section: "analytics", icon: "▥", label: "Analytics" },
  { section: "settings", icon: "⚙", label: "Settings" },
];

export const COMPACT_PRIMARY_SECTIONS = new Set<AppSection>([
  "decks",
  "practice",
  "analytics",
]);
`,
  );

  write(
    "src/components/NavigationSidebar.tsx",
    `import { useState } from "react";

import { APP_NAVIGATION, COMPACT_PRIMARY_SECTIONS } from "../app/navigation";
import type { AppSection } from "../controllers/useAppController";
import { useMediaQuery } from "../ui/hooks/useMediaQuery";
import { Sheet } from "../ui/overlays/Dialog";
import styles from "./NavigationSidebar.module.css";

interface NavigationSidebarProps {
  section: AppSection;
  onNavigate: (section: AppSection) => void | Promise<void>;
}

export function NavigationSidebar({ section, onNavigate }: NavigationSidebarProps) {
  const compact = useMediaQuery("(max-width: 760px)");
  const [moreOpen, setMoreOpen] = useState(false);

  function navigate(next: AppSection): void {
    setMoreOpen(false);
    void onNavigate(next);
  }

  if (compact) {
    const primary = APP_NAVIGATION.filter((item) =>
      COMPACT_PRIMARY_SECTIONS.has(item.section),
    );
    return (
      <>
        <nav className={styles.compactNav} aria-label="Main navigation">
          {primary.map((item) => (
            <button
              key={item.section}
              type="button"
              className={styles.compactButton}
              data-active={section === item.section}
              aria-current={section === item.section ? "page" : undefined}
              onClick={() => navigate(item.section)}
            >
              <span aria-hidden="true">{item.icon}</span>
              <span>{item.label}</span>
            </button>
          ))}
          <button
            type="button"
            className={styles.compactButton}
            data-active={!COMPACT_PRIMARY_SECTIONS.has(section)}
            aria-expanded={moreOpen}
            onClick={() => setMoreOpen(true)}
          >
            <span aria-hidden="true">•••</span>
            <span>More</span>
          </button>
        </nav>
        {moreOpen ? (
          <Sheet title="More" description="Additional ostudy sections" onClose={() => setMoreOpen(false)}>
            <div className={styles.moreList}>
              {APP_NAVIGATION.filter(
                (item) => !COMPACT_PRIMARY_SECTIONS.has(item.section),
              ).map((item) => (
                <button key={item.section} type="button" onClick={() => navigate(item.section)}>
                  <span aria-hidden="true">{item.icon}</span>
                  <span>{item.label}</span>
                </button>
              ))}
            </div>
          </Sheet>
        ) : null}
      </>
    );
  }

  return (
    <aside className={\`sidebar \${styles.sidebar}\`}>
      <div className="brand-block">
        <strong>ostudy</strong>
      </div>
      <nav className="sidebar-nav" aria-label="Main navigation">
        {APP_NAVIGATION.map((item) => (
          <button
            key={item.section}
            type="button"
            className={section === item.section ? "nav-button active" : "nav-button"}
            aria-current={section === item.section ? "page" : undefined}
            onClick={() => navigate(item.section)}
          >
            <span aria-hidden="true">{item.icon}</span>
            {item.label}
          </button>
        ))}
      </nav>
    </aside>
  );
}
`,
  );
}

function patchSettings() {
  let ipc = read("src/ipc/settings.ts");
  ipc = replaceRequired(
    ipc,
    "  reduceMotion: boolean;\n",
    "  reduceMotion: boolean;\n  cardFlipAnimation: boolean;\n",
    "settings IPC animation field",
  );
  write("src/ipc/settings.ts", ipc);

  let controller = read("src/controllers/useAppController.ts");
  controller = replaceRequired(
    controller,
    "  reduceMotion: false,\n",
    "  reduceMotion: false,\n  cardFlipAnimation: true,\n",
    "default animation setting",
  );
  write("src/controllers/useAppController.ts", controller);

  let rust = read("src-tauri/src/settings.rs");
  rust = replaceRequired(
    rust,
    "    pub reduce_motion: bool,\n",
    "    pub reduce_motion: bool,\n    pub card_flip_animation: bool,\n",
    "Rust settings DTO animation field",
  );
  rust = replaceRequired(
    rust,
    "        reduce_motion: get_setting(connection, \"reduceMotion\", false)?,\n",
    "        reduce_motion: get_setting(connection, \"reduceMotion\", false)?,\n        card_flip_animation: get_setting(connection, \"cardFlipAnimation\", true)?,\n",
    "load animation setting",
  );
  rust = replaceRequired(
    rust,
    "        (\n            \"confirmDeletions\",\n",
    "        (\n            \"cardFlipAnimation\",\n            serde_json::to_string(&settings.card_flip_animation),\n        ),\n        (\n            \"confirmDeletions\",\n",
    "save animation setting",
  );
  write("src-tauri/src/settings.rs", rust);

  for (const relative of walk("src", [".ts", ".tsx"])) {
    let source = read(relative);
    if (!source.includes("reduceMotion:") || source.includes("cardFlipAnimation:")) continue;
    source = source.replace(
      /(^\s*)reduceMotion:\s*(true|false),\n/gm,
      "$1reduceMotion: $2,\n$1cardFlipAnimation: true,\n",
    );
    write(relative, source);
  }

  write(
    "src/screens/settings/AppearanceSettings.tsx",
    `import type { AppSettings } from "../../ipc/settings";
import { FontControl, RangeControl, ThemeOption } from "./SettingsControls";
import styles from "./Settings.module.css";

interface AppearanceSettingsProps {
  draft: AppSettings;
  update: (patch: Partial<AppSettings>) => void;
}

export function AppearanceSettings({ draft, update }: AppearanceSettingsProps) {
  return (
    <div className={styles.panelGrid}>
      <section className={styles.fullWidthCard}>
        <div className={styles.sectionHeading}>
          <div>
            <h2>Theme</h2>
            <p>Theme and typography changes preview immediately throughout ostudy.</p>
          </div>
        </div>
        <div className={styles.themeGrid} role="radiogroup" aria-label="Application theme">
          <ThemeOption name="Light" value="light" selected={draft.theme === "light"} onSelect={() => update({ theme: "light" })} />
          <ThemeOption name="Dark" value="dark" selected={draft.theme === "dark"} onSelect={() => update({ theme: "dark" })} />
          <ThemeOption name="Console" value="console" selected={draft.theme === "console"} onSelect={() => update({ theme: "console" })} />
          <ThemeOption name="Synthwave" value="synthwave" selected={draft.theme === "synthwave"} onSelect={() => update({ theme: "synthwave" })} />
        </div>
      </section>

      <section className={styles.settingsCard}>
        <h2>Typography</h2>
        <FontControl label="Interface font" value={draft.uiFont} onChange={(uiFont) => update({ uiFont })} />
        <FontControl label="Default card font" value={draft.cardFont} onChange={(cardFont) => update({ cardFont })} />
      </section>

      <section className={styles.settingsCard}>
        <h2>Scale and motion</h2>
        <RangeControl
          label="Interface scale"
          min={80}
          max={140}
          step={5}
          value={draft.uiScale}
          suffix="%"
          onChange={(uiScale) => update({ uiScale })}
        />
        <RangeControl
          label="Default card text"
          min={12}
          max={32}
          step={1}
          value={draft.cardBaseSize}
          suffix=" px"
          onChange={(cardBaseSize) => update({ cardBaseSize })}
        />
        <label className={styles.switchRow}>
          <span>
            <strong>Flip card animation</strong>
            <small>Rotate the practice card to reveal its answer when the card is selected.</small>
          </span>
          <input
            type="checkbox"
            checked={draft.cardFlipAnimation}
            disabled={draft.reduceMotion}
            onChange={(event) => update({ cardFlipAnimation: event.currentTarget.checked })}
          />
        </label>
        <label className={styles.switchRow}>
          <span>
            <strong>Reduce motion</strong>
            <small>Disable card flipping and minimize nonessential transitions and effects.</small>
          </span>
          <input
            type="checkbox"
            checked={draft.reduceMotion}
            onChange={(event) => update({ reduceMotion: event.currentTarget.checked })}
          />
        </label>
      </section>
    </div>
  );
}
`,
  );
}

function patchPractice() {
  write(
    "src/screens/practice/ActivePractice.module.css",
    `.flipStage {
  perspective: 1400px;
}

.flipCard {
  display: grid;
  width: 100%;
  min-height: clamp(260px, 46vh, 560px);
  transform-style: preserve-3d;
  transition: transform var(--motion-slow) var(--ease-emphasized);
}

.flipCard[data-revealed="true"] {
  transform: rotateY(180deg);
}

.flipFace {
  grid-area: 1 / 1;
  min-width: 0;
  min-height: inherit;
  backface-visibility: hidden;
  -webkit-backface-visibility: hidden;
}

.flipFront {
  cursor: pointer;
}

.flipFront:focus-visible {
  outline: 3px solid var(--accent);
  outline-offset: 4px;
}

.flipBack {
  transform: rotateY(180deg);
}

.flipCard[data-revealed="false"] .flipBack,
.flipCard[data-revealed="true"] .flipFront {
  pointer-events: none;
  visibility: hidden;
}

@media (max-width: 760px) {
  .flipCard {
    min-height: clamp(230px, 42vh, 430px);
  }
}
`,
  );

  write(
    "src/screens/practice/ActivePractice.tsx",
    `import { useEffect, useRef } from "react";

import { RichTextView } from "../../editor/RichTextView";
import { type PracticeCard, type PracticeSession, type ReviewPreviewSet } from "../../ipc/practice";
import { type RescheduleIntent, type ScheduleAction } from "../../ipc/scheduling";
import { useExperience } from "../../ui/feedback/ExperienceProvider";
import { Menu } from "../../ui/overlays/Menu";
import { useOverlayContext } from "../../ui/overlays/overlayContext";
import { Progress } from "../../ui/primitives/Progress";
import type { ConfidenceScore } from "./confidence";
import styles from "./ActivePractice.module.css";

interface ActivePracticeProps {
  session: PracticeSession;
  current: PracticeCard;
  position: number;
  revealed: boolean;
  ratedCount: number;
  previewSet: ReviewPreviewSet | null;
  animateCardFlip?: boolean;
  isRevealing: boolean;
  isRating: boolean;
  isEnding: boolean;
  isManaging: boolean;
  errorMessage: string | null;
  onReveal: () => void;
  onScore: (score: ConfidenceScore) => void;
  onScheduleAction: (action: ScheduleAction, intent?: RescheduleIntent) => void;
  onEnd: () => void;
  onLinkError: (message: string) => void;
}

export function ActivePractice({
  session,
  current,
  position,
  revealed,
  ratedCount,
  previewSet,
  animateCardFlip = false,
  isRevealing,
  isRating,
  isEnding,
  isManaging,
  errorMessage,
  onReveal,
  onScore,
  onScheduleAction,
  onEnd,
  onLinkError,
}: ActivePracticeProps) {
  const ratingPanelRef = useRef<HTMLElement | null>(null);
  const wasRevealed = useRef(revealed);
  const busy = isRevealing || isRating || isEnding || isManaging;
  const { topId } = useOverlayContext();
  const experience = useExperience();

  useEffect(() => {
    if (revealed) ratingPanelRef.current?.focus();
    if (revealed && !wasRevealed.current) experience.emit({ type: "answer_revealed" });
    wasRevealed.current = revealed;
  }, [experience, revealed]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (busy || event.repeat || topId !== null) return;
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT", "BUTTON", "A", "SUMMARY"].includes(target.tagName))
      ) {
        return;
      }
      if (!revealed && (event.key === " " || event.key === "Enter")) {
        event.preventDefault();
        onReveal();
        return;
      }
      if (revealed && ["0", "1", "2", "3"].includes(event.key)) {
        event.preventDefault();
        onScore(Number(event.key) as ConfidenceScore);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busy, onReveal, onScore, revealed, topId]);

  const front = (
    <section className="practice-card-surface">
      <span className="practice-side-label">Question</span>
      <RichTextView
        document={current.presentation.frontDoc}
        assets={current.presentation.assets}
        onLinkError={onLinkError}
      />
    </section>
  );
  const back = (
    <section className="practice-card-surface">
      <span className="practice-side-label">Answer</span>
      <RichTextView
        document={current.presentation.backDoc}
        assets={current.presentation.assets}
        onLinkError={onLinkError}
      />
    </section>
  );

  return (
    <div className="screen practice-screen active-practice-screen">
      <header className="practice-session-header">
        <div>
          <span className="eyebrow">
            {session.mode === "extra"
              ? "EXTRA PRACTICE"
              : session.mode === "custom"
                ? "CUSTOM PRACTICE"
                : "PRACTICE"}
          </span>
          <h1>{current.deckName}</h1>
          {current.needsMorePractice ? (
            <span className="schedule-status-badge needs-practice">Needs more practice</span>
          ) : null}
        </div>
        <div className="practice-progress-copy">
          <strong>{position + 1} / {session.cardCount}</strong>
          <span>{ratedCount} reviewed</span>
        </div>
        <div className="practice-header-actions">
          <Menu
            label="More"
            disabled={busy}
            items={[
              { label: "Bury until tomorrow", onSelect: () => onScheduleAction("bury") },
              { label: "Suspend card", onSelect: () => onScheduleAction("suspend") },
              {
                label: "Reschedule for tomorrow",
                onSelect: () => onScheduleAction("reschedule", { kind: "tomorrow" }),
              },
              { label: "Reset schedule", destructive: true, onSelect: () => onScheduleAction("reset") },
            ]}
          />
          <button type="button" className="secondary-button" disabled={busy} onClick={onEnd}>
            {isEnding ? "Ending…" : "End session"}
          </button>
        </div>
      </header>

      <Progress
        className="practice-progress-track"
        label="Practice session progress"
        value={Math.min(position + 1, session.cardCount)}
        max={session.cardCount}
      />

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

      {animateCardFlip ? (
        <div className={styles.flipStage}>
          <div className={styles.flipCard} data-revealed={revealed}>
            <div
              className={\`${styles.flipFace} ${styles.flipFront}\`}
              role={!revealed ? "button" : undefined}
              tabIndex={!revealed && !busy ? 0 : -1}
              aria-label={!revealed ? "Reveal answer" : undefined}
              aria-hidden={revealed}
              onClick={() => {
                if (!revealed && !busy) onReveal();
              }}
            >
              {front}
            </div>
            <div className={\`${styles.flipFace} ${styles.flipBack}\`} aria-hidden={!revealed}>
              {back}
            </div>
          </div>
        </div>
      ) : (
        <section className="practice-card-surface">
          <span className="practice-side-label">Question</span>
          <RichTextView
            document={current.presentation.frontDoc}
            assets={current.presentation.assets}
            onLinkError={onLinkError}
          />
          {revealed ? (
            <>
              <div className="practice-answer-divider" />
              <span className="practice-side-label">Answer</span>
              <RichTextView
                document={current.presentation.backDoc}
                assets={current.presentation.assets}
                onLinkError={onLinkError}
              />
            </>
          ) : null}
        </section>
      )}

      {!revealed ? (
        <div className="practice-primary-action">
          <button
            type="button"
            className="primary-button large-action-button"
            disabled={isRevealing}
            onClick={onReveal}
          >
            {isRevealing ? "Calculating intervals…" : "Show answer"}
          </button>
        </div>
      ) : (
        <section
          ref={ratingPanelRef}
          tabIndex={-1}
          className="review-rating-panel confidence-rating-panel"
          aria-labelledby="confidence-rating-heading"
        >
          <div className="confidence-rating-heading">
            <h2 id="confidence-rating-heading">How difficult was that answer?</h2>
          </div>
          <div className="confidence-score-actions">
            <ConfidenceButton score={0} label="Wrong" disabled={isRating || !previewSet} onClick={() => onScore(0)} />
            <ConfidenceButton score={1} label="Hard" disabled={isRating || !previewSet} onClick={() => onScore(1)} />
            <ConfidenceButton score={2} label="Medium" disabled={isRating || !previewSet} onClick={() => onScore(2)} />
            <ConfidenceButton score={3} label="Easy" disabled={isRating || !previewSet} onClick={() => onScore(3)} />
          </div>
        </section>
      )}
    </div>
  );
}

function ConfidenceButton({
  score,
  label,
  disabled,
  onClick,
}: {
  score: ConfidenceScore;
  label: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={\`confidence-score-button confidence-score-\${score}\`}
      disabled={disabled}
      aria-label={\`\${score} \${label}\`}
      aria-keyshortcuts={String(score)}
      onClick={onClick}
    >
      <span className="confidence-score-number" aria-hidden="true">{score}</span>
      <span className="confidence-score-copy"><strong>{label}</strong></span>
    </button>
  );
}
`,
  );

  let practiceScreen = read("src/screens/PracticeScreen.tsx");
  practiceScreen = replaceRequired(
    practiceScreen,
    "        previewSet={previewSet}\n",
    "        previewSet={previewSet}\n        animateCardFlip={settings.cardFlipAnimation && !settings.reduceMotion}\n",
    "practice flip setting",
  );
  write("src/screens/PracticeScreen.tsx", practiceScreen);

  let setup = read("src/screens/practice/PracticeSetup.tsx");
  setup = insertImport(setup, 'import styles from "./PracticeSetup.module.css";');
  setup = replaceRequired(
    setup,
    "  return (\n    <>\n      <section className=\"practice-workload-panel\" aria-live=\"polite\">",
    "  return (\n    <div className={styles.layout}>\n      <section className={`practice-workload-panel ${styles.workload}`} aria-live=\"polite\">",
    "practice setup responsive wrapper",
  );
  setup = replaceRequired(
    setup,
    "      <section className=\"practice-setup-panel\">",
    "      <section className={`practice-setup-panel ${styles.setup}` }>",
    "practice deck-selection order class",
  );
  setup = replaceRequired(
    setup,
    "      <section className=\"recent-practice-section\" aria-labelledby=\"recent-practice-heading\">",
    "      <section className={`recent-practice-section ${styles.recent}`} aria-labelledby=\"recent-practice-heading\">",
    "recent practice order class",
  );
  const lastFragment = setup.lastIndexOf("    </>\n  );");
  if (lastFragment < 0) throw new Error("Could not close PracticeSetup responsive wrapper.");
  setup = `${setup.slice(0, lastFragment)}    </div>\n  );${setup.slice(lastFragment + "    </>\n  );".length)}`;
  write("src/screens/practice/PracticeSetup.tsx", setup);

  write(
    "src/screens/practice/PracticeSetup.module.css",
    `.layout {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.workload { order: 1; }
.setup { order: 2; }
.recent { order: 3; }

@media (max-width: 760px) {
  .setup { order: 1; }
  .workload { order: 2; }
}
`,
  );

  write(
    "src/screens/practice/ActivePractice.test.tsx",
    read("src/screens/practice/ActivePractice.test.tsx")
      .replace(
        '    expect(screen.getByRole("button", { name: /0 Wrong, About 1 min/u })).toBeTruthy();\n    expect(screen.getByRole("button", { name: /1 Hard, About 1 hr/u })).toBeTruthy();\n    expect(screen.getByRole("button", { name: /2 Medium, About 1 day/u })).toBeTruthy();\n    expect(screen.getByRole("button", { name: /3 Easy, About 1 month/u })).toBeTruthy();',
        '    expect(screen.getByRole("button", { name: "0 Wrong" })).toBeTruthy();\n    expect(screen.getByRole("button", { name: "1 Hard" })).toBeTruthy();\n    expect(screen.getByRole("button", { name: "2 Medium" })).toBeTruthy();\n    expect(screen.getByRole("button", { name: "3 Easy" })).toBeTruthy();\n    expect(screen.queryByText(/About 1/u)).toBeNull();',
      )
      .replace(
        '  it("shows all FSRS previews and submits 0-3 keyboard scores after reveal", () => {',
        '  it("keeps compact rating labels and submits 0-3 keyboard scores after reveal", () => {',
      )
      .replace(
        "    previewSet: null,\n",
        "    previewSet: null,\n    animateCardFlip: false,\n",
      ),
  );
}

function patchDeckWorkspace() {
  let app = read("src/App.tsx");
  app = replaceRequired(
    app,
    "                onPracticeDeck={controller.practiceDeck}\n",
    "                onPracticeDeck={controller.practiceDeck}\n                onPracticeCards={controller.practiceCards}\n                onManageTemplates={() => controller.navigate(\"templates\")}\n",
    "Decks workspace actions",
  );
  write("src/App.tsx", app);

  let decks = read("src/screens/DecksScreen.tsx");
  decks = insertImport(decks, 'import { useMediaQuery } from "../ui/hooks/useMediaQuery";');
  decks = insertImport(decks, 'import { BulkEntryModal } from "./cards/BulkEntryModal";');
  decks = insertImport(decks, 'import { CardsScreen } from "./CardsScreen";');
  decks = insertImport(decks, 'import styles from "./DecksWorkspace.module.css";');
  decks = replaceRequired(
    decks,
    "  onPracticeDeck: (deckId: string) => void | Promise<void>;\n",
    "  onPracticeDeck: (deckId: string) => void | Promise<void>;\n  onPracticeCards?: (cardIds: string[]) => void | Promise<void>;\n  onManageTemplates?: () => void | Promise<void>;\n",
    "Decks workspace prop types",
  );
  decks = replaceRequired(
    decks,
    "  onPracticeDeck,\n  onDirtyChange,\n",
    "  onPracticeDeck,\n  onPracticeCards = () => undefined,\n  onManageTemplates = () => undefined,\n  onDirtyChange,\n",
    "Decks workspace prop defaults",
  );
  decks = replaceRequired(
    decks,
    '  const [cardSort, setCardSort] = useState<"updated_desc" | "question_asc">("updated_desc");\n',
    '  const [cardSort, setCardSort] = useState<"created_desc" | "practiced_desc" | "practiced_asc">("created_desc");\n  const [showArchived, setShowArchived] = useState(false);\n  const [bulkEntryOpen, setBulkEntryOpen] = useState(false);\n  const [mobileSearchOpen, setMobileSearchOpen] = useState(false);\n',
    "deck sort and workspace state",
  );
  decks = replaceRequired(
    decks,
    "  const confirm = useConfirm();\n\n  const selectedDeckId",
    "  const confirm = useConfirm();\n  const compact = useMediaQuery(\"(max-width: 760px)\");\n  const activeDecks = decks.filter((deck) => deck.archivedAt === null);\n  const archivedDecks = decks.filter((deck) => deck.archivedAt !== null);\n\n  const selectedDeckId",
    "deck collections and mobile capability",
  );
  decks = decks.replaceAll('setCardSort("updated_desc")', 'setCardSort("created_desc")');
  decks = replaceRequired(
    decks,
    '                    setCardSort(event.currentTarget.value as "updated_desc" | "question_asc")\n',
    '                    setCardSort(event.currentTarget.value as "created_desc" | "practiced_desc" | "practiced_asc")\n',
    "deck sort selection type",
  );
  decks = replaceRequired(
    decks,
    '                  <option value="updated_desc">Recently updated</option>\n                  <option value="question_asc">Question A–Z</option>',
    '                  <option value="created_desc">Time created</option>\n                  <option value="practiced_desc">Most recently practiced</option>\n                  <option value="practiced_asc">Least recently practiced</option>',
    "deck sort options",
  );
  decks = decks.replace("          {decks.length === 0 ? (", "          {activeDecks.length === 0 ? (");
  decks = decks.replace("              {decks.map((deck) => {", "              {activeDecks.map((deck) => {");
  decks = decks.replaceAll(' ? "source note" : "source notes"', ' ? "card" : "cards"');
  decks = decks.replaceAll("source notes", "cards").replaceAll("source note", "card");
  decks = replaceRequired(
    decks,
    '            <div className="header-actions">\n              <button\n                type="button"\n                className="secondary-button"\n                disabled={isTransferring}\n                onClick={() => void handleImport()}\n              >',
    '            <div className="header-actions">\n              <button type="button" className="secondary-button" onClick={() => void onManageTemplates()}>\n                Manage templates\n              </button>\n              <button type="button" className="secondary-button" onClick={() => setBulkEntryOpen(true)}>\n                Bulk add\n              </button>\n              <button\n                type="button"\n                className="secondary-button"\n                disabled={isTransferring}\n                onClick={() => void handleImport()}\n              >',
    "Decks workspace header actions",
  );

  const searchLabel = `              <label className="search-field">
                <span>Search this deck</span>
                <input
                  type="search"
                  value={search}
                  onChange={(event) => setSearch(event.currentTarget.value)}
                  placeholder="Search questions and answers"
                />
              </label>`;
  const responsiveSearch = `              {compact ? (
                <button
                  type="button"
                  className={styles.mobileSearchButton}
                  aria-label="Search this deck"
                  onClick={() => setMobileSearchOpen(true)}
                >
                  <span aria-hidden="true">⌕</span>
                  <span>{search.trim() ? "Search active" : "Search"}</span>
                </button>
              ) : (
                <label className="search-field">
                  <span>Search this deck</span>
                  <input
                    type="search"
                    value={search}
                    onChange={(event) => setSearch(event.currentTarget.value)}
                    placeholder="Search questions and answers"
                  />
                </label>
              )}`;
  decks = replaceRequired(decks, searchLabel, responsiveSearch, "responsive deck search");
  decks = replaceRegexRequired(
    decks,
    /<span className="card-count-summary">[\s\S]*?<\/span>\n            <\/div>/,
    `<span className={\`card-count-summary \${styles.cardCountSummary}\`}>
                {cardTotalCount} {cardTotalCount === 1 ? "Card" : "Cards"}
                {selectedDeck.studyableDueCount > 0
                  ? \` · \${selectedDeck.studyableDueCount} ready now\`
                  : ""}
              </span>
            </div>`,
    "deck card count summary",
  );

  const libraryClose = "          )}\n        </>\n      ) : null}";
  const workspaceTail = `          )}

          <section className={styles.archivedSection} aria-labelledby="archived-decks-heading">
            <button
              type="button"
              className={styles.archivedLink}
              aria-expanded={showArchived}
              onClick={() => setShowArchived((current) => !current)}
            >
              <span id="archived-decks-heading">Archived decks</span>
              <span>{archivedDecks.length}</span>
            </button>
            {showArchived ? (
              archivedDecks.length === 0 ? (
                <p className={styles.archivedEmpty}>No archived decks.</p>
              ) : (
                <div className="deck-grid" aria-label="Archived decks">
                  {archivedDecks.map((deck) => (
                    <article key={deck.id} className="deck-card archived">
                      <div className="deck-card-main">
                        <span className="deck-card-mark" aria-hidden="true">▤</span>
                        <span className="deck-card-copy">
                          <strong>{deck.name}</strong>
                          <span>Archived · cards suspended</span>
                        </span>
                        <span className="deck-card-count">
                          <strong>{deck.generatedCardCount}</strong>
                          <small>{deck.generatedCardCount === 1 ? "card" : "cards"}</small>
                        </span>
                      </div>
                      <div className="deck-card-actions">
                        <button type="button" onClick={() => void handleArchiveDeck(deck, false)}>Restore</button>
                        <button type="button" disabled={isTransferring} onClick={() => void handleExport(deck)}>Export</button>
                        <button type="button" className="danger-text" onClick={() => void handleDeleteDeck(deck)}>Delete</button>
                      </div>
                    </article>
                  ))}
                </div>
              )
            ) : null}
          </section>

          <section className={styles.searchWorkspace} aria-labelledby="deck-card-search-heading">
            <div className={styles.workspaceHeading}>
              <div>
                <h2 id="deck-card-search-heading">Search and filter cards</h2>
                <p>Search every active deck or limit the results to selected decks and tags.</p>
              </div>
              <button type="button" className="secondary-button" onClick={() => setBulkEntryOpen(true)}>
                Bulk add
              </button>
            </div>
            <CardsScreen
              decks={decks}
              refreshDecks={refreshDecks}
              onPracticeCards={onPracticeCards}
              onDirtyChange={onDirtyChange}
              showToast={showToast}
            />
          </section>
        </>
      ) : null}`;
  decks = replaceRequired(decks, libraryClose, workspaceTail, "integrated deck workspace");

  const modalMarker = "      {deckDialog ? (";
  const modalInsertion = `      {mobileSearchOpen ? (
        <Modal title="Search this deck" onClose={() => setMobileSearchOpen(false)} width="small">
          <label className="search-field">
            <span>Search questions and answers</span>
            <input
              data-autofocus="true"
              type="search"
              value={search}
              onChange={(event) => setSearch(event.currentTarget.value)}
              placeholder="Enter a word or phrase"
            />
          </label>
          <div className="modal-actions">
            {search ? <button type="button" className="secondary-button" onClick={() => setSearch("")}>Clear</button> : null}
            <button type="button" className="primary-button" onClick={() => setMobileSearchOpen(false)}>Done</button>
          </div>
        </Modal>
      ) : null}

      {bulkEntryOpen ? (
        <BulkEntryModal
          decks={decks}
          defaultDeckId={selectedDeck?.id}
          onClose={() => setBulkEntryOpen(false)}
          onSaved={async () => {
            setBulkEntryOpen(false);
            if (selectedDeck) await refreshSelectedDeckAfterMutation(selectedDeck.id);
            else await refreshDecks();
          }}
          showToast={showToast}
        />
      ) : null}

${modalMarker}`;
  decks = replaceRequired(decks, modalMarker, modalInsertion, "Decks workspace dialogs");
  write("src/screens/DecksScreen.tsx", decks);

  write(
    "src/screens/DecksWorkspace.module.css",
    `.searchWorkspace {
  display: grid;
  gap: var(--space-4);
  margin-top: var(--space-7);
  min-width: 0;
  border-top: 1px solid var(--border);
  padding-top: var(--space-6);
}

.searchWorkspace :global(.screen.cards-screen) {
  min-width: 0;
  padding: 0;
}

.workspaceHeading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.workspaceHeading h2,
.workspaceHeading p {
  margin: 0;
}

.workspaceHeading p {
  margin-top: var(--space-1);
  color: var(--muted);
}

.archivedSection {
  display: grid;
  gap: var(--space-3);
  margin-top: var(--space-5);
}

.archivedLink {
  display: flex;
  width: 100%;
  min-height: var(--touch-target-min);
  align-items: center;
  justify-content: space-between;
  border: 0;
  border-top: 1px solid var(--border);
  padding: var(--space-3) 0;
  background: transparent;
  color: var(--accent);
  font-weight: 700;
  text-align: left;
}

.archivedEmpty {
  margin: 0;
  color: var(--muted);
}

.mobileSearchButton {
  display: inline-flex;
  min-height: var(--touch-target-min);
  align-items: center;
  gap: var(--space-2);
}

.cardCountSummary {
  min-width: 0;
  max-width: 100%;
  overflow-wrap: anywhere;
  white-space: normal;
}

@media (max-width: 760px) {
  .workspaceHeading {
    align-items: stretch;
    flex-direction: column;
  }

  .searchWorkspace {
    margin-top: var(--space-5);
    padding-top: var(--space-5);
  }
}
`,
  );

  let ipc = read("src/ipc/decks.ts");
  ipc = ipc.replace(
    '  sort: "updated_desc" | "question_asc";',
    '  sort: "created_desc" | "practiced_desc" | "practiced_asc";',
  );
  const cardInterface = interfaceBlock(
    ipc,
    (block, name) => name === "DeckCardSummary" && block.includes("updatedAt"),
  );
  if (cardInterface && !cardInterface.block.includes("lastPracticedAt")) {
    const replacement = cardInterface.block.replace(
      "  updatedAt: number;",
      "  createdAt?: number;\n  updatedAt: number;\n  lastPracticedAt?: number | null;",
    );
    ipc = `${ipc.slice(0, cardInterface.start)}${replacement}${ipc.slice(cardInterface.close + 1)}`;
  }
  write("src/ipc/decks.ts", ipc);

  patchDeckCardBackend();
}

function parseSqlTables() {
  const migrations = walk("src-tauri/migrations", [".sql"])
    .map((relative) => read(relative))
    .join("\n");
  const tables = [];
  const pattern = /CREATE TABLE(?: IF NOT EXISTS)?\s+["`]?([A-Za-z0-9_]+)["`]?\s*\(([\s\S]*?)\);/gi;
  for (const match of migrations.matchAll(pattern)) {
    const columns = [...match[2].matchAll(/^\s*["`]?([A-Za-z0-9_]+)["`]?\s+/gm)].map(
      (column) => column[1],
    );
    tables.push({ name: match[1], columns, body: match[2] });
  }
  return tables;
}

function reviewStorage() {
  const tables = parseSqlTables();
  const candidates = tables.filter(
    (table) => table.columns.includes("card_id") && /review|answer|attempt|history/i.test(table.name),
  );
  for (const table of candidates) {
    const time = ["reviewed_at", "answered_at", "occurred_at", "created_at", "submitted_at"].find(
      (column) => table.columns.includes(column),
    );
    if (time) return { table: table.name, time };
  }
  return { table: "review_logs", time: "reviewed_at" };
}

function patchDeckCardBackend() {
  let source = read("src-tauri/src/deck_cards.rs");
  const review = reviewStorage();
  source = replaceRequired(
    source,
    "    pub image_count: i64,\n    pub updated_at: i64,\n",
    "    pub image_count: i64,\n    pub created_at: i64,\n    pub updated_at: i64,\n    pub last_practiced_at: Option<i64>,\n",
    "deck card practice timestamps",
  );
  source = replaceRegexRequired(
    source,
    /    let order_sql = match input\.sort\.as_str\(\) \{[\s\S]*?    \};/,
    `    let order_sql = match input.sort.as_str() {
        "practiced_desc" => "last_practiced_at IS NULL ASC, last_practiced_at DESC, n.created_at DESC, n.id ASC",
        "practiced_asc" => "last_practiced_at IS NULL DESC, last_practiced_at ASC, n.created_at ASC, n.id ASC",
        _ => "n.created_at DESC, n.id ASC",
    };`,
    "deck sort SQL",
  );
  source = replaceRequired(
    source,
    "         LEFT JOIN cards c ON c.note_id = n.id\n         LEFT JOIN note_assets na ON na.note_id = n.id",
    `         LEFT JOIN cards c ON c.note_id = n.id
         LEFT JOIN ${review.table} practice_review ON practice_review.card_id = c.id
         LEFT JOIN note_assets na ON na.note_id = n.id`,
    "deck practice review join",
  );
  source = replaceRequired(
    source,
    "             n.updated_at\n         FROM notes n",
    `             n.created_at,
             n.updated_at,
             MAX(practice_review.${review.time}) AS last_practiced_at
         FROM notes n`,
    "deck practice review projection",
  );
  source = replaceRequired(
    source,
    "         GROUP BY n.id, n.front_text, n.back_text, n.updated_at, n.created_at\n",
    "         GROUP BY n.id, n.front_text, n.back_text, n.created_at, n.updated_at\n",
    "deck practice review grouping",
  );
  source = replaceRequired(
    source,
    "                image_count: row.get(5)?,\n                updated_at: row.get(6)?,\n",
    "                image_count: row.get(5)?,\n                created_at: row.get(6)?,\n                updated_at: row.get(7)?,\n                last_practiced_at: row.get(8)?,\n",
    "deck practice review row mapping",
  );
  source = source.replace(
    '!matches!(input.sort.as_str(), "updated_desc" | "question_asc")',
    '!matches!(input.sort.as_str(), "created_desc" | "practiced_desc" | "practiced_asc")',
  );
  source = source.replace(
    '"Sort deck cards by recent update or question."',
    '"Sort deck cards by creation time or recent practice."',
  );
  source = source.replace('"updated_desc".to_string()', '"created_desc".to_string()');
  write("src-tauri/src/deck_cards.rs", source);
}

function patchCredentialReveal() {
  const relative = "src/screens/settings/AiLlmSettings.tsx";
  let source = read(relative);
  source = source.replace(
    'import type { ReactNode } from "react";',
    'import { useState, type ReactNode } from "react";',
  );
  source = replaceRequired(
    source,
    "function CredentialPanel({ controller }: { controller: AiProfilesController }) {\n  const profile = controller.selectedProfile;",
    "function CredentialPanel({ controller }: { controller: AiProfilesController }) {\n  const [showCredential, setShowCredential] = useState(false);\n  const profile = controller.selectedProfile;",
    "credential visibility state",
  );
  source = replaceRegexRequired(
    source,
    /        <input\n          type="password"\n          autoComplete="off"\n          value=\{controller\.credentialDraft\}\n          placeholder=\{hasCredential \? "Enter a replacement" : "Credential is never readable after saving"\}\n          onChange=\{\(event\) => controller\.setCredentialDraft\(event\.currentTarget\.value\)\}\n        \/>/,
    `        <span className={aiStyles.secretField}>
          <input
            type={showCredential ? "text" : "password"}
            autoComplete="off"
            value={controller.credentialDraft}
            placeholder={hasCredential ? "Enter a replacement" : "Credential is never readable after saving"}
            onChange={(event) => controller.setCredentialDraft(event.currentTarget.value)}
          />
          <button
            type="button"
            className={aiStyles.secretToggle}
            aria-label={showCredential ? "Hide API credential" : "Show API credential"}
            aria-pressed={showCredential}
            onClick={() => setShowCredential((current) => !current)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M2.5 12s3.4-6 9.5-6 9.5 6 9.5 6-3.4 6-9.5 6-9.5-6-9.5-6Z" />
              <circle cx="12" cy="12" r="2.8" />
            </svg>
          </button>
        </span>`,
    "credential visibility control",
  );
  write(relative, source);

  const cssPath = "src/screens/settings/AiLlmSettings.module.css";
  let css = read(cssPath);
  if (!css.includes(".secretField")) {
    css += `

.secretField {
  position: relative;
  display: block;
}

.secretField input {
  width: 100%;
  padding-right: 46px;
}

.secretToggle {
  position: absolute;
  top: 50%;
  right: 6px;
  display: grid;
  width: 36px;
  height: 36px;
  place-items: center;
  transform: translateY(-50%);
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
}

.secretToggle:hover,
.secretToggle:focus-visible {
  background: var(--surface-strong);
  color: var(--text);
}

.secretToggle svg {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.8;
}
`;
  }
  write(cssPath, css);
}

function tokenUsageExpression() {
  const tables = parseSqlTables();
  const totalNames = [
    "total_tokens",
    "actual_tokens",
    "tokens_consumed",
    "total_token_count",
    "api_tokens",
  ];
  const inputNames = ["input_tokens", "prompt_tokens", "prompt_token_count"];
  const outputNames = ["output_tokens", "completion_tokens", "completion_token_count"];
  for (const table of tables) {
    const total = totalNames.find((name) => table.columns.includes(name));
    const input = inputNames.find((name) => table.columns.includes(name));
    const output = outputNames.find((name) => table.columns.includes(name));
    if (!total && !(input && output)) continue;
    const value = total
      ? `COALESCE(usage.${total}, 0)`
      : `COALESCE(usage.${input}, 0) + COALESCE(usage.${output}, 0)`;
    if (table.columns.includes("document_id")) {
      return `(SELECT SUM(${value}) FROM ${table.name} usage WHERE usage.document_id = documents.id)`;
    }
    if (table.columns.includes("job_id")) {
      return `(SELECT SUM(${value}) FROM ${table.name} usage JOIN document_generation_jobs usage_job ON usage_job.id = usage.job_id WHERE usage_job.document_id = documents.id)`;
    }
  }
  return "NULL";
}

function patchDocuments() {
  let rust = read("src-tauri/src/documents.rs");
  rust = replaceRequired(
    rust,
    "    pub revision: i64,\n}",
    "    pub revision: i64,\n    pub processing_duration_ms: Option<i64>,\n    pub api_tokens_consumed: Option<i64>,\n}",
    "document processing metrics DTO",
  );
  const duration = "CASE WHEN completed_at IS NULL THEN NULL ELSE MAX(0, completed_at - created_at) END";
  const tokens = tokenUsageExpression();
  rust = replaceRequired(
    rust,
    "                    completed_at, archived_at, revision\n             FROM documents",
    `                    completed_at, archived_at, revision,
                    ${duration},
                    ${tokens}
             FROM documents`,
    "document list metrics projection",
  );
  rust = replaceRequired(
    rust,
    "                    completed_at, archived_at, revision, source_relative_path,\n",
    `                    completed_at, archived_at, revision,
                    ${duration},
                    ${tokens},
                    source_relative_path,
`,
    "document detail metrics projection",
  );
  rust = replaceRequired(
    rust,
    "                    source_relative_path: row.get(30)?,\n                    extraction_backend: row.get(31)?,\n                    render_backend: row.get(32)?,\n                    semantic_chunk_count: row.get(33)?,\n                    source_format: row.get(34)?,\n                    source_unit_kind: row.get(35)?,\n",
    "                    source_relative_path: row.get(32)?,\n                    extraction_backend: row.get(33)?,\n                    render_backend: row.get(34)?,\n                    semantic_chunk_count: row.get(35)?,\n                    source_format: row.get(36)?,\n                    source_unit_kind: row.get(37)?,\n",
    "document detail metric indexes",
  );
  rust = replaceRequired(
    rust,
    "        revision: row.get(29)?,\n    })",
    "        revision: row.get(29)?,\n        processing_duration_ms: row.get(30)?,\n        api_tokens_consumed: row.get(31)?,\n    })",
    "document metric row mapping",
  );
  rust = rust.replace(
    "                revision: 1,\n            },",
    "                revision: 1,\n                processing_duration_ms: None,\n                api_tokens_consumed: None,\n            },",
  );
  write("src-tauri/src/documents.rs", rust);

  let ipc = read("src/ipc/documents.ts");
  const documentInterface = interfaceBlock(
    ipc,
    (block) => block.includes("ocrPageCount") && block.includes("generatedCardCount"),
  );
  if (!documentInterface) throw new Error("Could not find the document IPC interface.");
  if (!documentInterface.block.includes("processingDurationMs")) {
    const replacement = documentInterface.block.replace(
      /\n}\s*$/,
      "\n  processingDurationMs?: number | null;\n  apiTokensConsumed?: number | null;\n}",
    );
    ipc = `${ipc.slice(0, documentInterface.start)}${replacement}${ipc.slice(documentInterface.close + 1)}`;
  }
  write("src/ipc/documents.ts", ipc);

  const screenPath = "src/screens/DocumentsScreen.tsx";
  let screen = read(screenPath);
  screen = ensureReactNames(screen, ["useEffect", "useRef"]);
  screen = screen.replace(
    /\s*<p[^>]*>\s*The generated deck is available in Decks and uses the normal editing, scheduling, archive, and deletion tools\.\s*<\/p>/g,
    "",
  );
  screen = screen.replaceAll(
    "The generated deck is available in Decks and uses the normal editing, scheduling, archive, and deletion tools.",
    "",
  );
  screen = screen.replace(
    /\s*<[^>]+>\s*Native PDF text was unreliable[\s\S]*?Mean OCR confidence:[\s\S]*?<\/[^>]+>/g,
    "",
  );

  const documentsState = screen.match(/const \[(\w+),\s*set\w+\] = useState<[^;]*(?:Document|Pdf)[^;]*>\(\[\]\\?\);/);
  const stateName = documentsState?.[1] ?? "documents";
  const componentStart = screen.indexOf("export function DocumentsScreen");
  if (componentStart >= 0 && !screen.includes("ocrToastDocumentIds")) {
    const bodyOpen = screen.indexOf("{", componentStart);
    const insertionPoint = screen.indexOf("\n", bodyOpen) + 1;
    screen = `${screen.slice(0, insertionPoint)}  const ocrToastDocumentIds = useRef(new Set<string>());\n${screen.slice(insertionPoint)}`;
    const stateDeclarationEnd = screen.indexOf(";", screen.indexOf(`const [${stateName},`, insertionPoint));
    if (stateDeclarationEnd > 0) {
      const effect = `

  useEffect(() => {
    for (const document of ${stateName}) {
      if (
        document.ocrPageCount > 0 &&
        document.lifecycleState === "ready" &&
        !ocrToastDocumentIds.current.has(document.id)
      ) {
        ocrToastDocumentIds.current.add(document.id);
        showToast(
          "info",
          \`OCR recovered \${document.ocrPageCount} page\${document.ocrPageCount === 1 ? "" : "s"}. Review generated cards against the source document.\`,
        );
      }
    }
  }, [${stateName}, showToast]);`;
      screen = `${screen.slice(0, stateDeclarationEnd + 1)}${effect}${screen.slice(stateDeclarationEnd + 1)}`;
    }
  }
  if (!screen.includes("function DocumentProcessingMetrics")) {
    const filenameMarker = "{document.originalFilename}";
    if (screen.includes(filenameMarker)) {
      screen = screen.replace(
        filenameMarker,
        `${filenameMarker}\n                  <DocumentProcessingMetrics document={document} />`,
      );
    }
    screen += `

function DocumentProcessingMetrics({
  document,
}: {
  document: { processingDurationMs?: number | null; apiTokensConsumed?: number | null };
}) {
  const duration = document.processingDurationMs;
  const tokens = document.apiTokensConsumed;
  if (duration == null && tokens == null) return null;
  return (
    <small className={styles.processingMetrics}>
      {duration == null ? "Processing time unavailable" : \`Processed in \${formatProcessingDuration(duration)}\`}
      <span aria-hidden="true"> · </span>
      {tokens == null ? "API token use not reported" : \`\${tokens.toLocaleString()} API tokens\`}
    </small>
  );
}

function formatProcessingDuration(milliseconds: number): string {
  const seconds = Math.max(0, Math.round(milliseconds / 1000));
  if (seconds < 60) return \`\${seconds} sec\`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return remainingSeconds === 0 ? \`\${minutes} min\` : \`\${minutes} min \${remainingSeconds} sec\`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes === 0 ? \`\${hours} hr\` : \`\${hours} hr \${remainingMinutes} min\`;
}
`;
  }
  write(screenPath, screen);

  const cssPath = "src/screens/DocumentsScreen.module.css";
  let css = read(cssPath);
  if (!css.includes(".processingMetrics")) {
    css += `

.processingMetrics {
  display: block;
  margin-top: 4px;
  color: var(--muted);
  font-size: 0.76rem;
  line-height: 1.4;
}
`;
  }
  write(cssPath, css);
}

function patchOcrFallback() {
  const relative = "src-tauri/src/pdf_backend.rs";
  let source = read(relative);
  source = source.replace(
    '"oxidize-pdf-4.2.2-rag-quality-ocr-v1"',
    '"oxidize-pdf-4.2.2-rag-quality-ocr-on-failure-v2"',
  );
  const start = source.indexOf("pub(crate) fn plan_text_recovery");
  const end = source.indexOf("pub(crate) fn assess_text_quality", start);
  if (start < 0 || end < 0) throw new Error("Could not locate PDF recovery planner.");
  const replacement = `pub(crate) fn plan_text_recovery(document: &ExtractedDocument) -> ExtractionRecoveryPlan {
    let failed_pages = document
        .native_text_failed_pages
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut semantic_suspicious = BTreeSet::<usize>::new();

    for element in &document.elements {
        if assess_text_quality(&element.text).suspicious
            || element
                .parent_heading
                .as_deref()
                .is_some_and(|value| assess_text_quality(value).suspicious)
            || element
                .heading_path
                .iter()
                .any(|value| assess_text_quality(value).suspicious)
        {
            semantic_suspicious.insert(element.page_number);
        }
    }
    for chunk in &document.chunks {
        if assess_text_quality(&chunk.text).suspicious
            || assess_text_quality(&chunk.full_text).suspicious
            || chunk
                .heading_context
                .as_deref()
                .is_some_and(|value| assess_text_quality(value).suspicious)
        {
            semantic_suspicious.extend(chunk.page_numbers.iter().copied());
        }
    }

    let mut ocr_pages = Vec::new();
    let mut native_semantic_fallback_pages = BTreeSet::new();
    for page in &document.pages {
        let native_text_missing = page.text.trim().is_empty();
        let extraction_failed = failed_pages.contains(&page.page_number) || native_text_missing;
        if extraction_failed {
            ocr_pages.push(OcrPageCandidate {
                page_number: page.page_number,
                required: true,
                reasons: vec![if native_text_missing {
                    "missing_native_text"
                } else {
                    "native_extraction_failed"
                }],
            });
            continue;
        }

        if document.semantic_analysis_degraded
            || semantic_suspicious.contains(&page.page_number)
        {
            native_semantic_fallback_pages.insert(page.page_number);
        }
    }

    ExtractionRecoveryPlan {
        ocr_pages,
        native_semantic_fallback_pages: native_semantic_fallback_pages.into_iter().collect(),
    }
}

`;
  source = `${source.slice(0, start)}${replacement}${source.slice(end)}`;
  source = replaceRecoveryPlannerTests(source);
  write(relative, source);
}

function replaceRecoveryPlannerTests(source) {
  const moduleMarker = "#[cfg(test)]\nmod tests";
  const moduleStart = source.indexOf(moduleMarker);
  if (moduleStart < 0) return source;
  const moduleOpen = source.indexOf("{", moduleStart);
  const moduleClose = matchingBrace(source, moduleOpen);
  let body = source.slice(moduleOpen + 1, moduleClose);
  const testPattern = /\n\s*#\[test\]\s*\n\s*fn\s+([A-Za-z0-9_]+)\s*\(\)\s*\{/g;
  const removals = [];
  for (const match of body.matchAll(testPattern)) {
    const open = body.indexOf("{", match.index);
    const close = matchingBrace(body, open);
    const block = body.slice(match.index, close + 1);
    if (block.includes("plan_text_recovery")) removals.push([match.index, close + 1]);
  }
  for (const [start, end] of removals.reverse()) body = `${body.slice(0, start)}${body.slice(end)}`;
  body += `

    #[test]
    fn recovery_plan_uses_ocr_only_when_native_text_extraction_failed() {
        let mut failed = base_document("Readable native text with enough words.", "Readable semantic text.");
        failed.native_text_failed_pages = vec![1];
        let failed_plan = plan_text_recovery(&failed);
        assert_eq!(failed_plan.ocr_pages.len(), 1);
        assert!(failed_plan.ocr_pages[0].required);

        let empty = base_document("", "");
        let empty_plan = plan_text_recovery(&empty);
        assert_eq!(empty_plan.ocr_pages.len(), 1);
        assert!(empty_plan.ocr_pages[0].required);
    }

    #[test]
    fn suspicious_structured_text_rebuilds_from_native_text_without_ocr() {
        let document = base_document(
            "Readable native page text remains authoritative.",
            "\u{e000}\u{e001} suspicious structured text",
        );
        let plan = plan_text_recovery(&document);
        assert!(plan.ocr_pages.is_empty());
        assert_eq!(plan.native_semantic_fallback_pages, vec![1]);
    }
`;
  return `${source.slice(0, moduleOpen + 1)}${body}${source.slice(moduleClose)}`;
}

function patchAnalytics() {
  write(
    "src/screens/analytics/CustomDateRange.module.css",
    `.panel {
  display: grid;
  gap: var(--space-3);
  margin-bottom: var(--space-5);
  border: 1px solid var(--border);
  border-radius: var(--radius-large);
  padding: var(--space-4);
  background: var(--surface);
}

.fields {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr)) auto;
  gap: var(--space-3);
  align-items: end;
}

.fields label {
  display: grid;
  gap: var(--space-1);
}

.actions {
  display: flex;
  gap: var(--space-2);
}

.note {
  margin: 0;
  color: var(--muted);
  font-size: 0.82rem;
}

@media (max-width: 760px) {
  .fields { grid-template-columns: 1fr; }
  .actions { width: 100%; }
  .actions button { flex: 1; }
}
`,
  );
  write(
    "src/screens/analytics/CustomDateRange.tsx",
    `import { useState } from "react";

import styles from "./CustomDateRange.module.css";

export interface CustomAnalyticsRange {
  start: number;
  end: number;
  startText: string;
  endText: string;
}

interface CustomDateRangeProps {
  value: CustomAnalyticsRange | null;
  onChange: (range: CustomAnalyticsRange | null) => void;
}

export function CustomDateRange({ value, onChange }: CustomDateRangeProps) {
  const today = localDateText(Date.now());
  const sevenDaysAgo = localDateText(Date.now() - 6 * 86_400_000);
  const [start, setStart] = useState(value?.startText ?? sevenDaysAgo);
  const [end, setEnd] = useState(value?.endText ?? today);
  const invalid = !start || !end || start > end;

  return (
    <section className={styles.panel} aria-labelledby="custom-analytics-range-heading">
      <div>
        <h2 id="custom-analytics-range-heading">Custom date range</h2>
        <p className={styles.note}>Choose exact local-calendar dates for timeline charts.</p>
      </div>
      <div className={styles.fields}>
        <label>
          <span>Start date</span>
          <input type="date" value={start} max={end || today} onChange={(event) => setStart(event.currentTarget.value)} />
        </label>
        <label>
          <span>End date</span>
          <input type="date" value={end} min={start} max={today} onChange={(event) => setEnd(event.currentTarget.value)} />
        </label>
        <div className={styles.actions}>
          <button
            type="button"
            className="primary-button"
            disabled={invalid}
            onClick={() => {
              const startDate = new Date(\`${start}T00:00:00\`);
              const endDate = new Date(\`${end}T23:59:59.999\`);
              onChange({ start: startDate.getTime(), end: endDate.getTime(), startText: start, endText: end });
            }}
          >
            Apply
          </button>
          {value ? <button type="button" className="secondary-button" onClick={() => onChange(null)}>Use preset</button> : null}
        </div>
      </div>
    </section>
  );
}

function localDateText(timestamp: number): string {
  const date = new Date(timestamp);
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return \`${year}-\${month}-\${day}\`;
}
`,
  );
  write(
    "src/screens/analytics/filterAnalyticsRange.ts",
    `import type { CustomAnalyticsRange } from "./CustomDateRange";

const DATE_KEYS = [
  "date",
  "day",
  "bucketStart",
  "bucketEnd",
  "startedAt",
  "reviewedAt",
  "timestamp",
  "createdAt",
  "completedAt",
] as const;

export function filterAnalyticsRange<T>(value: T, range: CustomAnalyticsRange | null): T {
  if (!range) return value;
  return filterValue(value, range) as T;
}

function filterValue(value: unknown, range: CustomAnalyticsRange): unknown {
  if (Array.isArray(value)) {
    return value
      .filter((item) => {
        const timestamp = timestampFor(item);
        return timestamp === null || (timestamp >= range.start && timestamp <= range.end);
      })
      .map((item) => filterValue(item, range));
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, nested]) => [key, filterValue(nested, range)]),
    );
  }
  return value;
}

function timestampFor(value: unknown): number | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  for (const key of DATE_KEYS) {
    const candidate = record[key];
    if (typeof candidate === "number" && Number.isFinite(candidate)) return candidate;
    if (typeof candidate === "string") {
      const parsed = Date.parse(candidate.length === 10 ? \`${candidate}T12:00:00\` : candidate);
      if (Number.isFinite(parsed)) return parsed;
    }
  }
  return null;
}
`,
  );
  write(
    "src/ui/enhancers/useAnalyticsLineCharts.ts",
    `import { useEffect } from "react";
import type { RefObject } from "react";

/** Converts any legacy SVG bar series in Analytics into line series while the
 * underlying chart components are migrated incrementally. Existing SVG line
 * charts are left untouched. The conversion is scoped to the Analytics root. */
export function useAnalyticsLineCharts(root: RefObject<HTMLElement | null>, dependency: unknown) {
  useEffect(() => {
    const element = root.current;
    if (!element) return;
    const convert = () => element.querySelectorAll("svg").forEach(convertSvg);
    convert();
    const observer = new MutationObserver(convert);
    observer.observe(element, { childList: true, subtree: true, attributes: true });
    return () => observer.disconnect();
  }, [dependency, root]);
}

function convertSvg(svg: SVGSVGElement): void {
  if (svg.dataset.ostudyLineConverted === "true") return;
  const bounds = svg.getBoundingClientRect();
  const rectangles = [...svg.querySelectorAll("rect")].filter((rectangle) => {
    const width = numberAttribute(rectangle, "width");
    const height = numberAttribute(rectangle, "height");
    return width > 0 && height > 0 && (bounds.width <= 0 || width < bounds.width * 0.55);
  });
  if (rectangles.length < 2) return;

  const groups = new Map<string, SVGRectElement[]>();
  for (const rectangle of rectangles) {
    const key = rectangle.getAttribute("fill") || rectangle.getAttribute("class") || "series";
    const group = groups.get(key) ?? [];
    group.push(rectangle);
    groups.set(key, group);
  }

  let converted = false;
  for (const [key, group] of groups) {
    if (group.length < 2) continue;
    const points = group
      .map((rectangle) => ({
        x: numberAttribute(rectangle, "x") + numberAttribute(rectangle, "width") / 2,
        y: numberAttribute(rectangle, "y"),
      }))
      .sort((left, right) => left.x - right.x);
    const stroke = key === "series" ? "currentColor" : key;
    const polyline = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
    polyline.setAttribute("points", points.map((point) => \`${point.x},\${point.y}\`).join(" "));
    polyline.setAttribute("fill", "none");
    polyline.setAttribute("stroke", stroke);
    polyline.setAttribute("stroke-width", "2.5");
    polyline.setAttribute("stroke-linecap", "round");
    polyline.setAttribute("stroke-linejoin", "round");
    polyline.setAttribute("vector-effect", "non-scaling-stroke");
    polyline.setAttribute("data-ostudy-series", "line");
    svg.append(polyline);
    for (const point of points) {
      const circle = document.createElementNS("http://www.w3.org/2000/svg", "circle");
      circle.setAttribute("cx", String(point.x));
      circle.setAttribute("cy", String(point.y));
      circle.setAttribute("r", "3");
      circle.setAttribute("fill", stroke);
      circle.setAttribute("data-ostudy-series", "point");
      svg.append(circle);
    }
    for (const rectangle of group) rectangle.style.opacity = "0";
    converted = true;
  }
  if (converted) svg.dataset.ostudyLineConverted = "true";
}

function numberAttribute(element: Element, name: string): number {
  const value = Number(element.getAttribute(name));
  return Number.isFinite(value) ? value : 0;
}
`,
  );

  const relative = "src/screens/AnalyticsScreen.tsx";
  let source = read(relative);
  source = ensureReactNames(source, ["useMemo", "useRef", "useState"]);
  source = insertImport(source, 'import { CustomDateRange, type CustomAnalyticsRange } from "./analytics/CustomDateRange";');
  source = insertImport(source, 'import { filterAnalyticsRange } from "./analytics/filterAnalyticsRange";');
  source = insertImport(source, 'import { useAnalyticsLineCharts } from "../ui/enhancers/useAnalyticsLineCharts";');

  const functionStart = source.indexOf("export function AnalyticsScreen");
  if (functionStart < 0) throw new Error("Could not locate AnalyticsScreen.");
  const functionOpen = source.indexOf("{", functionStart);
  let functionClose = matchingBrace(source, functionOpen);
  let body = source.slice(functionOpen + 1, functionClose);
  const stateMatches = [...body.matchAll(/const \[(\w+),\s*(set\w+)\] = useState<([^;]+)>\(null\);/g)];
  const reportState = stateMatches.find((match) => /Analytics|Overview|Snapshot|Report/i.test(match[3]));
  const reportName = reportState?.[1] ?? null;
  const rangeState = body.match(/const \[(\w+),\s*(set\w+)\] = useState<[^;]*AnalyticsRange[^;]*>\(([^;]+)\);/);
  const rangeName = rangeState?.[1] ?? null;
  const rangeSetter = rangeState?.[2] ?? null;
  const rangeLiterals = [...source.matchAll(/["'](all|365d|180d|90d|30d|7d)["']/g)].map((match) => match[1]);
  const broadest = ["all", "365d", "180d", "90d", "30d", "7d"].find((value) => rangeLiterals.includes(value)) ?? "90d";

  const bodyInsertion = `
  const analyticsRootRef = useRef<HTMLDivElement | null>(null);
  const [customDateRange, setCustomDateRange] = useState<CustomAnalyticsRange | null>(null);
`;
  body = `${bodyInsertion}${body}`;
  if (reportName) {
    const firstReturn = body.indexOf("return (");
    if (firstReturn > 0) {
      const beforeReturn = body.slice(0, firstReturn);
      let returnAndAfter = body.slice(firstReturn);
      returnAndAfter = returnAndAfter.replace(new RegExp(`\\b${reportName}\\b`, "g"), "displayedAnalyticsReport");
      body = `${beforeReturn}  const displayedAnalyticsReport = useMemo(
    () => filterAnalyticsRange(${reportName}, customDateRange),
    [${reportName}, customDateRange],
  );

${returnAndAfter}`;
    }
  }
  const dependency = reportName ? `displayedAnalyticsReport` : rangeName ?? "customDateRange";
  const hookMarker = "  const [customDateRange, setCustomDateRange] = useState<CustomAnalyticsRange | null>(null);\n";
  body = body.replace(hookMarker, `${hookMarker}  useAnalyticsLineCharts(analyticsRootRef, ${dependency});\n`);

  body = body.replace(
    /<div className="screen analytics-screen">/,
    '<div ref={analyticsRootRef} className="screen analytics-screen">',
  );
  const rootMarker = '<div ref={analyticsRootRef} className="screen analytics-screen">';
  const setBroadRange = rangeSetter ? `${rangeSetter}("${broadest}" as AnalyticsRange);` : "";
  body = body.replace(
    rootMarker,
    `${rootMarker}
      <CustomDateRange
        value={customDateRange}
        onChange={(next) => {
          setCustomDateRange(next);
          if (next) ${setBroadRange}
        }}
      />`,
  );
  source = `${source.slice(0, functionOpen + 1)}${body}${source.slice(functionClose)}`;
  write(relative, source);
}

function patchStaticBoundaryCheck() {
  write(
    "scripts/check-platform-boundaries.mjs",
    `import fs from "node:fs";
import path from "node:path";

const roots = ["src/screens", "src/components", "src/controllers", "src/ui"];
const violations = [];
for (const root of roots) visit(root);
if (violations.length > 0) {
  console.error("UI/platform boundary violations:\\n" + violations.map((item) => \`  - \${item}\`).join("\\n"));
  console.error("Move native APIs behind src/platform or src/ipc adapters.");
  process.exit(1);
}
console.log("OK   UI modules do not import Tauri/native APIs directly");

function visit(relative) {
  if (!fs.existsSync(relative)) return;
  for (const entry of fs.readdirSync(relative, { withFileTypes: true })) {
    const child = path.join(relative, entry.name);
    if (entry.isDirectory()) visit(child);
    else if (/\\.(ts|tsx)$/.test(entry.name)) {
      const source = fs.readFileSync(child, "utf8");
      if (/from\\s+["']@tauri-apps\\//.test(source)) violations.push(child);
    }
  }
}
`,
  );
  const checkPath = "scripts/check-static.sh";
  let check = read(checkPath);
  if (!check.includes("check-platform-boundaries.mjs")) {
    check += "\nnode ./scripts/check-platform-boundaries.mjs\n";
  }
  write(checkPath, check);
}

function patchDocs() {
  write(
    "docs/architecture/ui-platform-boundary.md",
    `# UI and platform boundary

ostudy's React UI is a product layer, not a Linux or Tauri UI. Screens, feature components, controllers, and shared primitives must not import native platform APIs directly.

## Dependency direction

1. Screens and components express user workflows with React and domain types.
2. Controllers coordinate asynchronous work and cancellation without knowing which operating system hosts the app.
3. IPC modules expose typed application commands.
4. Filesystem pickers, native open/save behavior, deep links, notifications, and future mobile integrations live behind \`src/platform\` adapters.
5. Tauri commands and Rust services own persistence, scheduling, document processing, and provider execution.

The static platform-boundary check rejects direct \`@tauri-apps/*\` imports from \`src/screens\`, \`src/components\`, \`src/controllers\`, and \`src/ui\`. Android, Windows, macOS, iOS, and Linux shells can therefore replace or extend adapters without forking the core feature UI.

## Responsive policy

Responsive layout is capability-driven rather than operating-system-driven. Compact navigation, modal deck search, touch targets, safe-area insets, and reduced motion use media queries and shared UI hooks. Product code must not branch on user-agent strings.

## Navigation ownership

Top-level navigation contains Decks, Documents, Practice, Analytics, and Settings. Card search, bulk entry, archived decks, and template management belong to the Decks workspace. This keeps the same information architecture viable for desktop sidebars and mobile bottom navigation.
`,
  );
  write(
    "docs/user-guide/practice.md",
    `# Practicing flashcards

Select one or more decks, choose scheduled practice when cards are ready, or explicitly start extra study when nothing is due. On compact screens, deck selection and the start controls appear before workload statistics so starting a session does not require scrolling past analytics.

## Revealing an answer

Select the card, press **Space**, press **Enter**, or use **Show answer**. When **Flip card animation** is enabled in **Settings → Appearance**, the card rotates to show the answer. Turning that option off—or enabling **Reduce motion**—keeps the question in place and displays the answer below it.

## Rating recall

After revealing the answer, choose **Wrong (0)**, **Hard (1)**, **Medium (2)**, or **Easy (3)**. The number keys are keyboard shortcuts; the buttons omit duplicate shortcut badges so the rating itself remains the visual focus.

FSRS calculates the next interval after each rating. Intervals vary with review history, desired retention, and prior difficulty, so interval estimates are not printed inside the rating buttons. Scheduling details remain available through practice history and analytics.
`,
  );
  write(
    "docs/user-guide/analytics.md",
    `# Analytics

Analytics uses line charts for time-series data. Cards practiced and cards remembered are separate series so trends can be compared without mixing bars and lines.

Use a preset range for common views or enter exact start and end dates in **Custom date range**. Dates use the local calendar, matching scheduling and daily workload calculations.
`,
  );
  let docsIndex = read("docs/README.md");
  if (!docsIndex.includes("user-guide/practice.md")) {
    docsIndex += "\n- [Practice and rating](user-guide/practice.md)\n- [Analytics](user-guide/analytics.md)\n- [UI and platform boundary](architecture/ui-platform-boundary.md)\n";
  }
  write("docs/README.md", docsIndex);

  const documentGuidePath = "docs/user-guide/documents-and-ai-generation.md";
  let documentGuide = read(documentGuidePath);
  documentGuide += `

## OCR fallback and processing details

ostudy uses native PDF text whenever extraction succeeds. OCR is reserved for pages where native extraction fails or produces no text; semantic-layout degradation alone does not trigger OCR. When OCR is needed, a temporary in-app notification identifies the recovered page count so the document list remains uncluttered.

Completed document entries show elapsed processing time and total API tokens when the configured provider reports token usage. Providers that omit usage metadata are labeled as not reported rather than estimated.
`;
  write(documentGuidePath, documentGuide);
}

function patchTestsAndLanguage() {
  for (const relative of walk("src", [".test.ts", ".test.tsx"])) {
    let source = read(relative);
    source = source.replaceAll('"question_asc"', '"created_desc"');
    source = source.replaceAll("Question A–Z", "Time created");
    source = source.replaceAll("Recently updated", "Time created");
    source = source.replaceAll("source notes", "cards").replaceAll("source note", "card");
    source = source.replace(
      /^.*The generated deck is available in Decks and uses the normal editing, scheduling, archive, and deletion tools.*\n/gm,
      "",
    );
    source = source.replace(/^.*Native PDF text was unreliable.*\n/gm, "");
    write(relative, source);
  }
  for (const relative of walk("src-tauri/src", [".rs"])) {
    let source = read(relative);
    if (!source.includes("DocumentDto {")) continue;
    source = source.replace(
      /(revision:\s*[^,]+,\n)(\s*)(?!processing_duration_ms:)/g,
      "$1$2processing_duration_ms: None,\n$2api_tokens_consumed: None,\n$2",
    );
    write(relative, source);
  }
}

function walk(relativeRoot, extensions) {
  const results = [];
  const absoluteRoot = file(relativeRoot);
  if (!fs.existsSync(absoluteRoot)) return results;
  for (const entry of fs.readdirSync(absoluteRoot, { withFileTypes: true })) {
    const relative = path.join(relativeRoot, entry.name).replaceAll(path.sep, "/");
    if (entry.isDirectory()) results.push(...walk(relative, extensions));
    else if (extensions.some((extension) => entry.name.endsWith(extension))) results.push(relative);
  }
  return results;
}

patchNavigation();
patchSettings();
patchPractice();
patchDeckWorkspace();
patchCredentialReveal();
patchDocuments();
patchOcrFallback();
patchAnalytics();
patchStaticBoundaryCheck();
patchDocs();
patchTestsAndLanguage();

fs.rmSync(file("scripts/apply-ui-platform-overhaul.mjs"), { force: true });
fs.rmSync(file(".github/workflows/apply-ui-platform-overhaul.yml"), { force: true });
console.log("Applied UI and platform overhaul.");
