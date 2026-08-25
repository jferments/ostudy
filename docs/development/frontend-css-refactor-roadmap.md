# Frontend and CSS ownership refactor roadmap

This is the execution ledger for ADR 0048. Checkboxes are mirrored in PR #74 and updated after each exact-head qualification stage.

## Stage 0 — Inventory, characterization, and ratchet

- [ ] Add ADR 0048 and this execution ledger.
- [ ] Inventory legacy selectors and map each group to one final owner.
- [ ] Add `verify-style-ownership.mjs` and Python fixtures.
- [ ] Expand UI policy discovery to all authored CSS.
- [ ] Add a legacy non-growth baseline.
- [ ] Add missing deck-library characterization tests.
- [ ] Establish current static and fast qualification baseline.

## Stage 1 — Pure deck-library organization policy

- [ ] Add `deck-library/organizationModel.ts`.
- [ ] Extract ordering, lifecycle partitioning, move validation, and optimistic transformations.
- [ ] Guarantee immutable inputs and normalized sort orders.
- [ ] Add table-driven pure model tests.
- [ ] Route the existing workspace through the model without changing markup.

## Stage 2 — Deck-library controller

- [ ] Add `useDeckLibraryController.ts`.
- [ ] Move organization load/reload and lifecycle-signature refresh.
- [ ] Move deck/folder mutations and synchronous mutation exclusion.
- [ ] Move optimistic state, stale-revision recovery, partial-refresh handling, delete/undo, sheets, collapse state, toasts, and announcements.
- [ ] Add controller tests independent of detailed presentation.
- [ ] Ensure only the controller imports deck-folder IPC and `DecksController`.

## Stage 3 — Unified reorder adapter and view decomposition

- [ ] Define one `LibraryMoveIntent` for every input method.
- [ ] Add `useDeckLibraryReorder.ts` for keyboard, HTML drag, and pointer/touch translation.
- [ ] Extract `DeckLibraryView`, `FolderSection`, `DeckItem`, `ArchivedDecksSection`, and sheets.
- [ ] Split the deck-library CSS Module by meaningful visual owner.
- [ ] Remove direct DOM drag-state mutation outside the adapter.
- [ ] Add parity tests for all interaction modes and accessibility behavior.
- [ ] Reduce `DeckLibraryWorkspace.tsx` to route/detail composition.

## Stage 4 — Foundations and application shell

- [ ] Add `reset.css`, `document.css`, `platform.css`, and `utilities.css`.
- [ ] Move root/document/platform rules without changing values.
- [ ] Remove duplicated hard-coded theme definitions.
- [ ] Make generated themes the only theme authority.
- [ ] Remove legacy `.app-shell`, `.main-area`, `.sidebar`, and `.sidebar-nav` classes from JSX.
- [ ] Remove shell-specific legacy responsive rules and `!important` workarounds.
- [ ] Add module-owned application loading/fatal status UI.
- [ ] Preserve layout modes, safe areas, Android IME behavior, forced colors, and motion preferences.

## Stage 5 — Shared controls, feedback, and overlays

- [ ] Migrate legacy button classes to `Button`/shared icon-button contracts.
- [ ] Migrate global alerts to `InlineNotice`.
- [ ] Migrate global empty states and panels to shared primitives/patterns.
- [ ] Move native control visuals to explicit shared control modules.
- [ ] Remove legacy modal/backdrop styling after overlay audit.
- [ ] Remove legacy toast styling.
- [ ] Eliminate production JSX dependencies on global button/alert/modal classes.

## Stage 6 — Rich editor and rendered educational content

- [ ] Add `RichTextEditor.module.css` and `RichContent.module.css`.
- [ ] Move editor toolbar/chrome styling.
- [ ] Move rendered prose, lists, tables, code, links, images, and indentation styling.
- [ ] Scope ProseMirror global exceptions to the third-party wrapper.
- [ ] Update card, template, guide, preview, and practice renderers.
- [ ] Verify editor and renderer parity across all themes and scales.

## Stage 7 — Feature-by-feature global CSS retirement

- [ ] Cards/authoring/templates/scheduling migrated to feature modules and shared owners.
- [ ] Practice migrated to setup/active/complete modules and shared primitives.
- [ ] Analytics migrated to Metrics patterns and Analytics modules.
- [ ] Settings/Appearance/Diagnostics migrated; obsolete global theme-preview rules removed.
- [ ] Documents/assets migrated to existing feature modules.
- [ ] Embedded CSS icons replaced with `Icon` components or semantic artwork.
- [ ] Delete migrated global feature stylesheets as each slice reaches zero ownership.

## Stage 8 — Delete the legacy system and normalize the cascade

- [ ] Delete `src/styles/base/application.css`.
- [ ] Delete `ui-overhaul.css`, `page-density.css`, and all remaining global feature stylesheets.
- [ ] Remove legacy imports and the `legacy` cascade layer.
- [ ] Remove temporary compatibility aliases.
- [ ] Normalize the final layer order.
- [ ] Switch the ratchet to a zero-legacy assertion.
- [ ] Update maintained UI architecture/development documentation.
- [ ] Run final exact-head static, fast, authoritative Linux/Tauri, Android package, and API-36 emulator qualification.
- [ ] Record manual visual/accessibility acceptance matrix as human-required evidence.

## Compatibility checklist

- [ ] No Rust, migration, IPC, route, scheduling, document, study-guide, or backup schema change.
- [ ] Light, Dark, Console, and Synthwave remain complete.
- [ ] 80%, 100%, and 140% scale remain usable.
- [ ] Full, reduced, and no-motion modes remain usable.
- [ ] Rich and simple materials remain usable.
- [ ] Keyboard, mouse, pointer/touch, and screen-reader alternatives remain available.
- [ ] No document-level horizontal overflow at representative widths.
- [ ] Android safe areas, IME displacement, compact navigation, and overlay-first Back remain correct.
