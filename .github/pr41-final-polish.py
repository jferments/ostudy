from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8")


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected exactly one match, found {count}: {old[:100]!r}")
    write(path, text.replace(old, new, 1))


# ---------------------------------------------------------------------------
# Deck detail projection: expose reversibility explicitly instead of forcing the
# UI to infer it from generated card count (which is also used by Cloze notes).
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/src/deck_cards.rs",
    """    pub generated_card_count: i64,\n    pub image_count: i64,\n    pub updated_at: i64,\n""",
    """    pub generated_card_count: i64,\n    pub image_count: i64,\n    pub reversible: bool,\n    pub updated_at: i64,\n""",
)
replace_once(
    "src-tauri/src/deck_cards.rs",
    """             COUNT(DISTINCT CASE\n                 WHEN a.deleted_at IS NULL AND a.lifecycle_state = 'ready' THEN a.id\n             END),\n             n.updated_at\n""",
    """             COUNT(DISTINCT CASE\n                 WHEN a.deleted_at IS NULL AND a.lifecycle_state = 'ready' THEN a.id\n             END),\n             MAX(CASE\n                 WHEN n.note_type = 'basic' AND n.reverse_enabled = 1 THEN 1 ELSE 0\n             END),\n             n.updated_at\n""",
)
replace_once(
    "src-tauri/src/deck_cards.rs",
    """                generated_card_count: row.get(4)?,\n                image_count: row.get(5)?,\n                updated_at: row.get(6)?,\n""",
    """                generated_card_count: row.get(4)?,\n                image_count: row.get(5)?,\n                reversible: row.get::<_, i64>(6)? != 0,\n                updated_at: row.get(7)?,\n""",
)
replace_once(
    "src/ipc/decks.ts",
    """  generatedCardCount: number;\n  imageCount: number;\n  updatedAt: number;\n""",
    """  generatedCardCount: number;\n  imageCount: number;\n  reversible: boolean;\n  updatedAt: number;\n""",
)

# Update the two canonical DeckCardSummary fixtures.
fixture_markers = {
    "src/screens/DecksScreen.test.tsx": (
        "  imageCount: 0,\n  updatedAt: 1,\n",
        "  imageCount: 0,\n  reversible: false,\n  updatedAt: 1,\n",
    ),
    "src/screens/DecksScreen.extended.test.tsx": (
        "    imageCount: 0,\n    updatedAt: 1,\n",
        "    imageCount: 0,\n    reversible: false,\n    updatedAt: 1,\n",
    ),
}
for path, (old, new) in fixture_markers.items():
    replace_once(path, old, new)

# Make the primary DecksScreen fixture reversible so the icon behavior is covered.
replace_once(
    "src/screens/DecksScreen.test.tsx",
    """  imageCount: 0,\n  reversible: false,\n  updatedAt: 1,\n""",
    """  imageCount: 0,\n  reversible: true,\n  updatedAt: 1,\n""",
)

# Replace variable-width variant metadata with a fixed reversible indicator.
replace_once(
    "src/screens/DecksScreen.tsx",
    """                    <div className=\"card-row-actions\">\n                      <span>\n                        {card.generatedCardCount}{\" \"}\n                        {card.generatedCardCount === 1 ? \"card\" : \"card variants\"} ·{\" \"}\n                        {card.imageCount > 0\n                          ? `${card.imageCount} image${card.imageCount === 1 ? \"\" : \"s\"}`\n                          : \"Text\"}\n                      </span>\n                      <button type=\"button\" onClick={() => void openCardEditor(card)}>\n""",
    """                    <div className=\"card-row-actions\">\n                      <span\n                        className=\"card-row-reversible\"\n                        aria-label={card.reversible ? \"Reversible\" : undefined}\n                        title={card.reversible ? \"Reversible card\" : undefined}\n                        aria-hidden={card.reversible ? undefined : \"true\"}\n                      >\n                        {card.reversible ? \"🔄\" : null}\n                      </span>\n                      <button type=\"button\" onClick={() => void openCardEditor(card)}>\n""",
)

# Add a UI regression assertion for the reversible indicator after card hydration.
replace_once(
    "src/screens/DecksScreen.test.tsx",
    """    await waitFor(() => {\n      expect(screen.getByRole(\"heading\", { name: \"Dog facts\" })).toBeTruthy();\n    });\n    expect(await screen.findByLabelText(\"Reversible\")).toBeTruthy();\n    expect(screen.queryByText(/card variants/u)).toBeNull();\n\n    const detailPractice = screen.getByRole(\"button\", { name: \"Practice\" });\n""",
    """    await waitFor(() => {\n      expect(screen.getByRole(\"heading\", { name: \"Dog facts\" })).toBeTruthy();\n    });\n    expect(await screen.findByLabelText(\"Reversible\")).toBeTruthy();\n    expect(screen.queryByText(/card variants/u)).toBeNull();\n\n    const detailPractice = screen.getByRole(\"button\", { name: \"Practice\" });\n""",
)

# ---------------------------------------------------------------------------
# Practice header and compact vertical layout.
# ---------------------------------------------------------------------------
replace_once(
    "src/screens/practice/ActivePractice.tsx",
    """        <div>\n          <span className=\"eyebrow\">\n            {session.mode === \"extra\"\n              ? \"EXTRA PRACTICE\"\n              : session.mode === \"custom\"\n                ? \"CUSTOM PRACTICE\"\n                : \"PRACTICE\"}\n          </span>\n          <h1>{current.deckName}</h1>\n          {current.needsMorePractice ? (\n            <span className=\"schedule-status-badge needs-practice\">Needs more practice</span>\n          ) : null}\n        </div>\n        <div className=\"practice-progress-copy\">\n          <strong>\n            {position + 1} / {session.cardCount}\n          </strong>\n          <span>{ratedCount} reviewed</span>\n        </div>\n        <div className=\"practice-header-actions\">\n""",
    """        <div className=\"practice-session-title\">\n          <span className=\"eyebrow\">\n            {session.mode === \"extra\"\n              ? \"EXTRA PRACTICE\"\n              : session.mode === \"custom\"\n                ? \"CUSTOM PRACTICE\"\n                : \"PRACTICE\"}\n          </span>\n          <h1>{current.deckName}</h1>\n          <div className=\"practice-session-meta\">\n            <span\n              className=\"practice-progress-chip\"\n              aria-label={`${position + 1} of ${session.cardCount} cards, ${ratedCount} reviewed`}\n            >\n              <strong>{position + 1} / {session.cardCount}</strong>\n              <span>· {ratedCount} reviewed</span>\n            </span>\n            {current.needsMorePractice ? (\n              <span className=\"schedule-status-badge needs-practice\">Needs more practice</span>\n            ) : null}\n          </div>\n        </div>\n        <div className=\"practice-header-actions\">\n""",
)
replace_once(
    "src/screens/practice/ActivePractice.tsx",
    """          <button\n            type=\"button\"\n            className=\"secondary-button practice-header-button\"\n            disabled={busy}\n            onClick={onEdit}\n""",
    """          <button\n            type=\"button\"\n            className=\"secondary-button practice-header-button\"\n            disabled={busy}\n            onClick={onEdit}\n""",
)
replace_once(
    "src/screens/practice/ActivePractice.tsx",
    """          <Menu\n            label=\"More\"\n            disabled={busy}\n""",
    """          <Menu\n            label=\"Schedule\"\n            className=\"practice-header-button\"\n            disabled={busy}\n""",
)
replace_once(
    "src/screens/practice/ActivePractice.tsx",
    """            className=\"secondary-button\"\n            disabled={busy}\n            onClick={onEnd}\n""",
    """            className=\"practice-end-button practice-header-button\"\n            disabled={busy}\n            onClick={onEnd}\n""",
)

# Test the clearer schedule label, danger treatment hook, and single-line progress chip.
replace_once(
    "src/screens/practice/ActivePractice.test.tsx",
    'fireEvent.click(screen.getByRole("button", { name: "More" }));',
    'fireEvent.click(screen.getByRole("button", { name: "Schedule" }));',
)
replace_once(
    "src/screens/practice/ActivePractice.test.tsx",
    """    expect(screen.getByText(\"Needs more practice\")).toBeTruthy();\n  });\n""",
    """    expect(screen.getByText(\"Needs more practice\")).toBeTruthy();\n    expect(screen.getByLabelText(\"1 of 1 cards, 0 reviewed\")).toBeTruthy();\n    expect(screen.getByRole(\"button\", { name: \"Schedule\" })).toBeTruthy();\n    expect(screen.getByRole(\"button\", { name: \"End session\" }).className).toContain(\n      \"practice-end-button\",\n    );\n  });\n""",
)

# ---------------------------------------------------------------------------
# UI containment + consistent deck columns + viewport-aware practice controls.
# This stylesheet is intentionally the last feature stylesheet and is the right
# place for cross-feature final-polish overrides.
# ---------------------------------------------------------------------------
ui_path = "src/styles/features/ui-overhaul.css"
ui = read(ui_path)
addition = r'''

/* Final PR #41 containment pass: grid/flex children must be allowed to shrink
   rather than forcing explanatory text or metadata across panel boundaries. */
.card-browser-panel,
.card-browser-panel > *,
.card-browser-toolbar > *,
.card-row > *,
.card-row-main > *,
.card-row-actions > *,
.bulk-entry-layout > *,
.bulk-entry-controls > *,
.bulk-entry-table > *,
.bulk-entry-row > *,
.duplicate-review-panel > *,
.screen-header > *,
.header-actions > * {
  min-width: 0;
  max-width: 100%;
}

.bulk-entry-controls > p,
.duplicate-review-panel p,
.inline-alert,
.modal-help-text {
  overflow-wrap: anywhere;
  word-break: normal;
}

.bulk-entry-controls {
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
}

.bulk-entry-controls > p {
  align-self: center;
}

/* Every deck row uses the same action-column width. Question and Answer then
   remain equal minmax(0, 1fr) tracks regardless of card metadata. */
.card-row {
  grid-template-columns: minmax(0, 1fr) 176px;
}

.card-row-main {
  grid-template-columns: 38px minmax(0, 1fr) auto minmax(0, 1fr);
}

.card-row-actions {
  display: grid;
  grid-template-columns: 32px minmax(0, 1fr) minmax(0, 1fr);
  align-items: center;
  justify-content: stretch;
  gap: 2px;
  width: 176px;
  padding-inline: 8px;
}

.card-row-actions button {
  min-width: 0;
  padding-inline: 5px;
}

.card-row-reversible {
  display: grid;
  width: 32px;
  min-width: 32px;
  height: 32px;
  place-items: center;
  margin: 0;
  color: var(--accent);
  font-size: 0.95rem;
  line-height: 1;
}

/* Active practice uses the available viewport as a layout budget. Long card
   content scrolls inside the card rather than pushing the grading controls off
   screen. */
.active-practice-screen {
  display: flex;
  height: 100%;
  min-height: 0;
  flex-direction: column;
  overflow: hidden;
  padding-top: clamp(18px, 3vh, 34px);
  padding-bottom: clamp(16px, 3vh, 30px);
}

.practice-session-header {
  flex: 0 0 auto;
  align-items: flex-start;
  gap: clamp(12px, 2vw, 24px);
  margin-bottom: clamp(8px, 1.4vh, 14px);
}

.practice-session-title {
  min-width: 0;
  flex: 1 1 auto;
}

.practice-session-title h1 {
  overflow-wrap: anywhere;
}

.practice-session-meta {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-top: 10px;
}

.practice-progress-chip {
  display: inline-flex;
  min-height: 30px;
  min-width: 0;
  align-items: center;
  gap: 6px;
  border: 1px solid color-mix(in srgb, var(--accent) 45%, var(--border));
  border-radius: 999px;
  padding: 4px 10px;
  background: var(--accent-soft);
  color: var(--text-soft);
  font-size: 0.82rem;
  white-space: nowrap;
}

.practice-progress-chip strong {
  color: var(--accent);
  font-size: 0.88rem;
}

.practice-header-actions {
  display: flex;
  min-width: 0;
  flex: 0 0 auto;
  flex-wrap: wrap;
  align-items: stretch;
  justify-content: flex-end;
  gap: 8px;
}

.practice-header-button {
  min-height: var(--control-height);
  padding: 0 var(--space-4);
  white-space: nowrap;
}

.practice-end-button {
  min-height: var(--control-height);
  border: 1px solid color-mix(in srgb, var(--danger) 78%, white 8%);
  border-radius: var(--radius-small);
  padding: 0 var(--space-4);
  background: var(--danger);
  color: #fff;
  font-weight: 760;
  white-space: nowrap;
}

.practice-end-button:hover:not(:disabled) {
  filter: brightness(1.08);
}

.active-practice-screen .practice-progress-track,
.active-practice-screen .practice-primary-action,
.active-practice-screen .confidence-rating-panel {
  flex: 0 0 auto;
}

.active-practice-screen .practice-card-surface,
.active-practice-screen .practice-flip-card {
  min-height: clamp(170px, 34vh, 420px);
  flex: 1 1 auto;
  overflow: auto;
}

.active-practice-screen .practice-flip-inner {
  height: 100%;
  min-height: 100%;
}

.active-practice-screen .confidence-rating-panel {
  margin-top: clamp(8px, 1.3vh, 14px);
  padding: clamp(10px, 1.8vh, 18px);
}

.active-practice-screen .confidence-rating-heading h2 {
  font-size: clamp(1rem, 2.1vw, 1.3rem);
}

.active-practice-screen .confidence-score-actions {
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: clamp(6px, 1vw, 12px);
  margin-top: clamp(8px, 1.3vh, 14px);
}

.active-practice-screen .confidence-score-button {
  min-height: clamp(58px, 9vh, 76px);
  gap: clamp(5px, 0.8vw, 10px);
  padding: clamp(7px, 1vw, 12px);
}

.active-practice-screen .confidence-score-number {
  width: clamp(1.8rem, 3vw, 2.25rem);
  height: clamp(1.8rem, 3vw, 2.25rem);
  font-size: clamp(0.88rem, 1.5vw, 1.05rem);
}

.active-practice-screen .confidence-score-copy strong {
  font-size: clamp(0.84rem, 1.5vw, 1rem);
}

@media (max-height: 720px) {
  .active-practice-screen {
    padding-top: 14px;
    padding-bottom: 14px;
  }

  .practice-session-header {
    margin-bottom: 6px;
  }

  .practice-session-title h1 {
    font-size: clamp(1.2rem, 2.6vw, 1.7rem);
  }

  .practice-session-meta {
    margin-top: 6px;
  }

  .active-practice-screen .practice-card-surface,
  .active-practice-screen .practice-flip-card {
    min-height: 150px;
  }

  .active-practice-screen .confidence-rating-heading p {
    display: none;
  }

  .active-practice-screen .confidence-score-actions {
    margin-top: 6px;
  }

  .active-practice-screen .confidence-score-button {
    min-height: 56px;
  }
}

@media (max-width: 760px) {
  .card-row {
    grid-template-columns: 1fr;
  }

  .card-row-actions {
    width: 100%;
    grid-template-columns: 32px auto auto;
    justify-content: end;
  }

  .practice-session-header {
    flex-wrap: wrap;
  }

  .practice-header-actions {
    width: 100%;
    justify-content: flex-start;
  }
}

@media (max-width: 600px) {
  .active-practice-screen .confidence-score-actions {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .active-practice-screen .confidence-score-button {
    min-height: 54px;
  }
}
'''
if "Final PR #41 containment pass" in ui:
    raise SystemExit("ui-overhaul final polish already applied")
write(ui_path, ui.rstrip() + addition.rstrip() + "\n")

# ---------------------------------------------------------------------------
# Startup geometry: wider initial desktop window pairs with the four-across
# review controls; CSS still adapts immediately if the user resizes smaller.
# ---------------------------------------------------------------------------
replace_once(
    "src-tauri/tauri.conf.json",
    '        "width": 1100,\n        "height": 720,\n',
    '        "width": 1180,\n        "height": 760,\n        "center": true,\n',
)

print("Final PR41 UI polish applied")
