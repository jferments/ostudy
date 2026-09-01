# ADR 0049: Learning-workflow navigation, compact actions, and editor isolation

- **Status:** Accepted
- **Date:** 2026-08-25
- **Related:** ADR 0048, `docs/architecture/ui.md`, `docs/development/ui-workflow-redesign-roadmap.md`

## Context

The document, AI-provider, study-guide, deck-library, and CSS ownership systems have recently been decomposed into explicit domain, IPC, controller, feature, and shared-UI boundaries. The UI now has a zero-legacy CSS baseline with one global foundation entry and CSS Modules for shared and feature ownership.

The current product information architecture still reflects older implementation boundaries:

- the top-level **Practice** destination mixes practice setup, recent activity, workload information, active-session orchestration, and the study-guide library;
- document rows expose persistent operational metadata and many full-width actions even after processing is complete;
- document archiving adds a lifecycle state that is not required to preserve generated flashcards;
- deck selection, reordering, and practice launch are split across screens;
- study guides are treated as a secondary panel and a constrained dialog rather than a first-class learning artifact;
- the public rich-editor component directly exposes Tiptap-specific commands and extension knowledge;
- several common actions are represented by large text buttons instead of compact, accessible controls;
- responsiveness and drag feedback are correct in principle but are difficult and slow in practical use on large libraries.

The redesign must simplify the visible product without undoing the recently completed architectural and CSS work.

## Decision

### 1. Use learning artifacts—not setup workflows—as top-level destinations

The visible primary navigation will be ordered as:

1. **Flashcards**
2. **Study Guides**
3. **Documents**
4. **Analytics**
5. **Settings**

Practice setup will no longer be a top-level destination. Deck, card, search-result, tag, and study-guide actions will launch practice through typed controller requests. An active practice session remains a dedicated internal application state so that session restoration, exit guards, checkpoints, and completion analytics remain isolated and reliable. It is not shown as a standing navigation item.

Existing saved checkpoints that name the old Practice destination will be migrated or normalized. A checkpoint containing a real resumable session will reopen that internal session; a setup-only checkpoint will return to Flashcards.

### 2. Add a shared action-density layer

Feature screens will use shared, token-driven primitives rather than inventing local button rows:

- `Tooltip`
- `IconButton`
- `ResponsiveActionGroup`
- `OverflowMenu`
- `SelectionToolbar`
- `MetadataDialog` / `InfoButton`

A familiar icon may replace text only when its meaning is standard and the control has an accessible name, keyboard focus treatment, and hover/focus tooltip. Primary or unfamiliar actions retain text. Destructive actions use compact red icon treatment in the surface, followed by a fully explicit confirmation dialog.

On compact layouts, secondary and destructive actions move into an accessible overflow menu. Common actions remain directly visible so minimalism does not become menu diving.

### 3. Separate deck selection from deck movement

Deck-library selection will be first-class controller state with checkboxes and a contextual selection toolbar. Practice, move, archive, and delete operations can act on selected decks.

Reordering continues to flow through the existing pure organization policy, `LibraryMoveIntent`, and unified input adapter. Pointer drag, keyboard movement, touch/fallback movement, and bulk move all produce the same validated domain intent. Dragging may begin from any non-interactive area of a deck or folder row, while buttons, links, inputs, and checkboxes remain interactive rather than accidental drag initiators.

### 4. Remove document archive semantics and preserve provenance independently

Documents are either present or deleted. Existing archived documents will be restored to their prior terminal processing state during migration; they will not be deleted.

Generated flashcards do not depend on the continued existence of the managed source. Before a document can be deleted, ostudy will retain a durable source-citation snapshot for surviving generated decks/cards. The citation snapshot is separate from deck title and description and can contain:

- human-readable source title;
- original filename;
- format;
- page/slide/chapter/section locators where available;
- stable source and generation metadata needed for attribution;
- an explicit indication that the local source file is no longer available.

Deleting a document continues to remove the managed source, extracted derivatives, document-bound study guides, and document-bound generation state. The generated flashcards survive with their citations. Deleting a study guide separately does not delete its source document or linked flashcards.

### 5. Generate learning-artifact metadata from subject matter

Document-derived deck titles and descriptions will be generated from the shared document learning plan, not copied from the filename. Titles and descriptions describe what the learner will study. Provenance remains in the citation area.

Generated metadata carries an origin/fingerprint so user edits are never overwritten by regeneration. A narrowly bounded repair may update legacy values only when they exactly match known old generator templates; it will not guess whether arbitrary user text is machine-generated.

### 6. Make study guides first-class and responsive

The Study Guides workspace owns listing, opening, deleting, regenerating, annotation entry, topic-linked practice, and PDF export. The guide reader uses available viewport space on wide desktop displays, a narrower readable text measure inside that workspace, and a full-screen/sheet presentation on compact layouts.

The PDF renderer remains a native, deterministic export path. It will gain a reusable print design system, embedded IBM Plex Sans weights, semantic block styles, source citations, and bounded pagination rather than scraping browser CSS or the React DOM.

### 7. Keep the persisted rich-document contract editor-independent

Feature and domain code continue to exchange the canonical, versioned `RichDocument` representation. A public editor surface exposes editor-neutral capabilities and commands. The Tiptap implementation, extensions, selection handling, and toolbar bindings live behind an adapter.

Subscript, superscript, tables, standard task lists, and answer-key checklists are added through that boundary. Renderers, import/export, backups, templates, practice display, and PDF paths must round-trip every new node or mark before the feature is considered complete.

### 8. Treat responsiveness as a measured product requirement

Performance work begins with reproducible traces and interaction marks. Optimizations target measured causes such as unnecessary React commits, synchronous IPC or state fan-out, layout measurement loops, excessive filter/backdrop work, or unthrottled pointer events. Virtualization and memoization are introduced only where traces show that they reduce work without breaking accessibility.

Desktop window maximize/full-screen behavior is routed through the platform adapter. The application shell and study-guide workspace must use the resulting viewport rather than retaining narrow fixed-size constraints.

### 9. Preserve the zero-legacy CSS architecture

The redesign does not reintroduce feature-global styles, direct palette values, global semantic classes, or screen-specific copies of shared controls. Every new visual primitive uses semantic tokens and explicit CSS Module ownership. The existing theme, scale, motion, material, safe-area, IME, keyboard, screen-reader, pointer, and touch contracts remain acceptance requirements.

## Consequences

- The visible navigation becomes simpler, but active practice still has a durable internal route and recovery model.
- Several changes require coordinated Rust, migration, IPC, controller, and UI work rather than CSS-only edits.
- Source attribution becomes more reliable because it no longer depends on keeping a document archived.
- The editor change adds an abstraction layer now, reducing the cost of replacing Tiptap later.
- The shared action-density primitives make future cleanup changes cheaper and more consistent.
- A large cross-feature PR must be executed as bounded stage commits with focused qualification at each stage and exact-head full qualification at the end.
