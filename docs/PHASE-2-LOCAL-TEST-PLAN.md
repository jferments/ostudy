# Phase 2 local test plan

Use this checklist on the Ubuntu workstation before merging the Phase 2 pull
request. It is intentionally broader than the native smoke test because it
covers real persistence, desktop rendering, and an existing local library.

## 1. Prepare the branch without losing local work

```bash
cd ~/code/ostudy
git status --short
git fetch origin --prune
git switch agent/phase-2-standalone-flashcard-core
git pull --ff-only
```

If `git switch` reports an unstaged `src-tauri/Cargo.lock` change left by an
older build, inspect and restore only that accidental generated change:

```bash
git diff -- src-tauri/Cargo.lock
git restore --worktree src-tauri/Cargo.lock
git switch agent/phase-2-standalone-flashcard-core
git pull --ff-only
```

Do not restore unrelated files.

## 2. Run the automated gate and launch

```bash
./scripts/setup-ubuntu.sh
./scripts/check.sh
./run-app.sh
```

Setup may be skipped when the exact pinned environment is already installed, but
`./scripts/check.sh` must pass. Keep the terminal open and note any Rust/Tauri
errors.

## 3. Protect the existing library

Use Settings to note the database location. Make an ordinary filesystem backup
of the application-data directory before exercising a schema-changing branch on
important data. The migration is designed to be in-place and tested, but the
manual acceptance pass should still begin with a backup.

## 4. Create card types

Create a temporary deck named `Phase 2 test`.

### Basic and reverse

1. Create a Basic note and confirm one card is listed.
2. Edit it and enable reverse generation.
3. Confirm two generated cards and preview both directions.
4. Disable reverse and confirm the forward card remains while the reverse card
   is removed from active browsing.

### Fill in the blank

Create:

```text
Warfarin inhibits [[vitamin K epoxide reductase]] and decreases synthesis of
[[factors II, VII, IX, and X]].
```

Confirm two generated cards, one blank at a time, with the complete statement on
the answer side. Try unmatched or empty brackets and confirm saving is rejected
with an ordinary validation message.

### Template

1. Select the built-in Pharmacology template.
2. Enter warfarin fields and confirm the generated previews.
3. Save and confirm multiple cards are listed with meaningful display names.
4. Create a small custom template, use it, edit its name, and verify a template
   with an undeclared placeholder cannot be saved.

## 5. Tags, search, duplicates, and bulk entry

1. Add tags such as `pharmacology`, `anticoagulant`, and `high-alert`.
2. Search by ordinary front/back text and a template field value.
3. Enter characters that resemble FTS operators or SQL and confirm they are
   treated as search text rather than causing an error or changing data.
4. Filter by tag, card mode, due/new/suspended/buried/Struggling status, and sort
   order.
5. Attempt to create an exact duplicate; cancel once, then explicitly save it.
6. Open Bulk card entry, add several `Question | Answer` lines, include a
   duplicate, and verify created/skipped totals.

## 6. Draft recovery

1. Begin a new card, enter content, and navigate away without saving.
2. Reopen the editor and restore the autosaved draft.
3. Make another draft and discard it.
4. Save a restored draft and restart the application; confirm the saved card is
   present and the draft prompt no longer appears.

## 7. Move/copy and card state

1. Create a second temporary deck.
2. Use Move/Copy to copy one note and verify both decks contain independent note
   records.
3. Move another note and verify it leaves the source deck.
4. Suspend/resume, bury/unbury, reset, and reschedule cards; verify each browser
   status filter.
5. Confirm resetting returns the generated cards to New without deleting the
   note.

## 8. Practice setup and direct ratings

1. Open Practice and confirm there is no large Session options panel.
2. Confirm the compact checkbox is labeled **Shuffle decks**.
3. Confirm recent practiced decks show Wrong/Hard/OK/Easy counts when history
   exists and that Select chooses that deck.
4. Select one or more decks and start practice.
5. Reveal with Space or Enter.
6. Confirm only one rating question appears, with 0 Wrong, 1 Hard, 2 OK, and
   3 Easy, and an interval preview for each.
7. Use number keys 0–3 and confirm they match button behavior.
8. Use `U` and Ctrl/Cmd+Z to undo the latest review; confirm the card reopens with
   its answer shown and the previous interval is restored.
9. Finish a session and confirm the completion counts use “not wrong” rather than
   the old two-stage Right/confidence flow.

## 9. Limits, related cards, and Struggling cards

1. Set small daily new/review limits and confirm a new session respects them.
2. Enable related-card burying and confirm only one card from the same reverse,
   Fill in the blank, or template note enters one initial session.
3. Temporarily lower the Struggling threshold and rate a test card Wrong until it
   becomes Struggling.
4. Confirm the browser and analytics counts update.
5. Enable automatic suspension, repeat on another test card, and confirm it is
   both Struggling and suspended.
6. Clear/reset the temporary state and restore preferred settings.

## 10. Analytics

Confirm:

- the distribution is Wrong/Hard/OK/Easy;
- per-deck totals and recent reviews update;
- recent Practice cards show the latest session confidence mix;
- an undone review disappears from active aggregates without deleting older
  history;
- Struggling and suspended counts match the browser.

## 11. SQLite deck exchange

1. Export the temporary deck to one `.studydeck` file.
2. Import it and verify content, tags, templates, generated cards, and images.
3. Confirm imported cards have fresh local schedules and no imported review
   history.
4. If a known legacy version 1 package is available, confirm it still imports.
5. Rename a copy to an unrelated extension or choose a non-SQLite file and
   confirm the importer fails safely without changing the library.

## 12. Images, links, keyboard, and themes

1. Add a managed image and exercise Small, Medium, Large, and Full width.
2. Exercise left, center, and right image alignment.
3. Restart and confirm size/alignment persist.
4. Test a safe HTTPS link and confirm unsupported schemes are rejected.
5. Open every modal by keyboard, cycle with Tab and Shift+Tab, close with Escape,
   and confirm focus returns to the opener.
6. Repeat representative Decks, editor, Practice, and Analytics paths in Light,
   Dark, Console, and Synthwave themes at narrow and wide window sizes.

## 13. Final repository check

After manual testing:

```bash
git status --short
git diff -- src-tauri/Cargo.lock
```

Both should be empty. Record any failure with the exact command, UI path, terminal
output, and diagnostics archive before merging.
