# Frontend and CSS ownership refactor roadmap

This is the durable implementation ledger for ADR 0048. PR #75 completed the deck-library, foundation, shared-control, and rich-content work; PR #76 completes the remaining global CSS retirement and zero-legacy enforcement.

## Stage 0 — Inventory, characterization, and ratchet

- [x] Add ADR 0048 and the execution ledger.
- [x] Inventory legacy selectors and map each group to one final owner.
- [x] Add style ownership verification and fixtures.
- [x] Expand UI policy discovery to authored CSS.
- [x] Establish a non-growth ratchet and deck-library characterization baseline.

## Stage 1 — Pure deck-library organization policy

- [x] Add `deck-library/organizationModel.ts`.
- [x] Extract ordering, lifecycle partitioning, move validation, and optimistic transformations.
- [x] Guarantee immutable inputs and normalized sort orders.
- [x] Add table-driven model tests and route existing behavior through the model.

## Stage 2 — Deck-library controller

- [x] Add `useDeckLibraryController.ts`.
- [x] Move organization load/reload, lifecycle refresh, deck/folder mutations, optimistic state, stale-revision recovery, delete/undo, sheets, toasts, and announcements into the controller boundary.
- [x] Add controller tests independent of presentation.

## Stage 3 — Unified reorder adapter and view decomposition

- [x] Define one `LibraryMoveIntent` for every input method.
- [x] Route keyboard, HTML drag, pointer/touch, and fallback movement through the same policy.
- [x] Extract deck-library view components and split their CSS Module ownership.
- [x] Preserve accessibility and non-drag alternatives.

## Stage 4 — Foundations and application shell

- [x] Add `reset.css`, `document.css`, `platform.css`, and `utilities.css`.
- [x] Move root/document/platform rules to explicit owners.
- [x] Make generated semantic themes the appearance authority.
- [x] Replace shell/navigation global classes with module ownership.
- [x] Preserve layout modes, safe areas, Android IME behavior, forced colors, and motion preferences.

## Stage 5 — Shared controls, feedback, and overlays

- [x] Move reusable buttons, notices, empty/loading states, panels, controls, dialogs, sheets, menus, and toasts to shared owners.
- [x] Remove global modal/backdrop/toast ownership.
- [x] Eliminate production dependencies on global shared-control classes.

## Stage 6 — Rich editor and rendered educational content

- [x] Add scoped `RichTextEditor.module.css` and `RichContent.module.css` ownership.
- [x] Move editor chrome and rendered headings/prose/lists/tables/code/links/images/indentation.
- [x] Scope the ProseMirror exception to the third-party wrapper.
- [x] Route card, template, guide, preview, and practice rendering through the shared rich-content owner.

## Stage 7 — Feature-by-feature global CSS retirement

- [x] Cards/authoring/templates/scheduling migrated to feature modules and shared owners.
- [x] Practice migrated to setup/active/complete modules and shared primitives.
- [x] Analytics migrated to feature modules and Metrics patterns.
- [x] Settings/Appearance/Diagnostics migrated to module/shared ownership.
- [x] Documents/assets migrated to feature modules.
- [x] CSS semantic icons replaced with owned icon/artwork components where applicable.
- [x] Delete `DecksScreen.stage12.css`, `CardListing.css`, and every global feature stylesheet.

Exact-head checkpoint: `9cf1937a8e19b6b5f09939c3624753840f996c60` passed static and fast qualification after the feature-global retirement and migration-test reconciliation.

## Stage 8 — Delete the legacy system and normalize the cascade

- [x] Delete `src/styles/base/application.css` and the remaining `src/styles` tree.
- [x] Remove the `legacy` layer and global feature imports.
- [x] Normalize the final cascade to `reset, tokens, theme, base, components, patterns, features, utilities`.
- [x] Switch style ownership from ratchet mode to a zero-legacy assertion.
- [x] Require exactly one global TypeScript CSS import (`src/main.tsx` → `src/ui/foundations/index.css`).
- [x] Require zero temporary style-policy allowances and no authored global CSS outside the foundation.
- [x] Preserve QR quiet-zone contrast through generated reference token `color.neutral.0` pinned to `#ffffff`.
- [x] Remove temporary `data-animations-enabled` and `data-reduce-motion` compatibility attributes; `data-motion` is canonical.
- [x] Isolate the native-smoke/manual-launch standalone executable from mutable Cargo debug output.
- [x] Update ADR/development documentation.

Exact-head implementation checkpoint: `cffe1b6dd9a24f33ba034cc9be45e3945b5ab865` passed static and fast qualification with zero legacy styles and compatibility attributes removed.

## Final merge gate

Final qualification is recorded on PR #76 rather than by editing this ledger after the run, because any post-qualification source edit would create a new unqualified head. Before merge, PR #76 must show all of the following against its exact final SHA:

- authoritative Linux/Tauri `check.sh`, including startup and native WebDriver smoke;
- Android compile/package qualification;
- API-36 emulator smoke;
- the staged standalone artifact launches successfully via `./run-app.sh`;
- the human visual/accessibility matrix is accepted for Light, Dark, Console, Synthwave; representative compact/medium/wide widths; 80/100/140% scale; full/reduced/no-motion; rich/simple materials; keyboard and pointer/touch alternatives; safe-area/IME behavior; and representative overflow/focus paths.

The PR remains draft until the automated gate is green. The final human matrix is explicitly human-required and is not represented as an automated assertion.
