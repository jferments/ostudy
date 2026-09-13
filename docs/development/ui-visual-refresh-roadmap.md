# UI visual refresh roadmap

Status: **source implementation for V0–V6 is integrated in PR #97; qualification is in progress.** PR #96 completed the shadcn/Base UI/Tailwind architecture migration. PR #97 changes ordinary presentation and removes migration-era visual compatibility while preserving product behavior. V7 remains the frozen automated qualification plus explicit human visual/device acceptance boundary.

## Design contract

The default rule is **stock first**:

> Use the owned shadcn/Base UI Nova component composition, sizing, spacing, hierarchy and ordinary variants unless oStudy has a concrete product, accessibility or platform reason to differ.

The application still owns semantic colors and all four themes, capability state, coarse-pointer touch targets, reduced motion/material behavior, safe areas/IME rules and domain semantics. Stock shadcn geometry does not make shadcn a theme authority.

Application facades remain useful when they express product meaning or platform behavior. A wrapper that only preserves the appearance of the pre-#96 UI should be removed or thinned.

## Non-goals

No scheduler, persistence, IPC, provider, knowledge/Tutor, document-processing, sync-domain, Chart.js or Tiptap schema redesign. PR #94 source permissions, optional grounding, private question criteria, citations, durable jobs and operation identities remain unchanged.

## V0 — baseline and contracts

**Implemented.** Public facades and specialized presentation were re-audited against current `main`. The 16 registered specialized CSS owners remain because they carry real rich-content, drag, Practice-card, QR, tutorial or native geometry. The historical Stage-0 screenshot matrix remains historical evidence rather than the new appearance specification.

## V1 — primitive normalization

**Implemented.** Button, Card, Badge, Tabs, Sidebar, native selects and dialog presentation move toward the Base UI Nova vocabulary. Card now exposes standard title/description/action/content/footer slots. Item and ButtonGroup are application-owned primitives. Vendor-only migration variants were removed; the public Button facade retains product-semantic names and loading/write-lock behavior. Fine-pointer controls are compact while coarse input still expands to the application touch minimum.

## V2 — abstraction cleanup

**Implemented for ordinary presentation.** Metric tiles compose Card directly, document provider status uses Item, Panel composes the public Card slots, SelectionToolbar no longer requires Surface, and generic Surface rendering is intentionally flattened while remaining available for product-semantic state. Badge/Chip, FormField, Toolbar/ButtonGroup and Card/Item responsibilities are clearer without moving domain behavior into UI primitives.

## V3 — shell and global hierarchy

**Implemented.** Sidebar navigation uses flatter Nova-like rows, desktop content uses a quiet inset/ring relationship, PageHeader no longer puts routine icons in decorative card tiles, command bars and inline workspaces are flatter, and overlays/tabs use the same compact visual grammar. Compact navigation, safe-area, IME, focus and native Back contracts remain application-owned.

## V4 — dense workflows

**Implemented.** Settings controls, provider status, Flashcards browser/search presentation, selection actions, rich-editor chrome and card-editor preview/authoring presentation use the refreshed density, rings, typography and action hierarchy. Existing operations, identities, drafts, import receipts, drag/reorder alternatives, templates and sync semantics remain unchanged.

## V5 — learning/content workflows

**Implemented.** Documents and Study Guide libraries/readers, Tutor, Practice and Analytics have fewer nested/elevated surfaces and stronger content hierarchy. Study Guide reading remains an inline nonmodal workspace with opaque sticky contents. Tutor transcript/composer is visually primary while history/sources/settings recede. Practice retains its custom two-face geometry and confidence semantics. Chart.js and analytics attribution are unchanged.

## V6 — cleanup, documentation and visual evidence

**Implemented in source; final commit/qualification evidence belongs to the PR head.** The #96 implementation/roadmap and ADR are reconciled as completed history, current UI architecture records the stock-first rule, agent code standards discourage presentation-only wrappers, and vendor provenance is pinned to a current reviewed upstream comparison. The visual-baseline wrapper gains a `candidate` mode that captures exact-head review evidence into ignored `.artifacts/` output without overwriting the historical Stage-0 matrix.

## V7 — visual and platform acceptance

**In progress.** Validation order remains focused tests -> UI policy/CSS compilation -> static -> fast -> targeted native tests -> frozen full Linux/Android qualification -> human visual/device acceptance.

Human review covers Light/Dark/Console/Synthwave, compact/medium/wide, enlarged text, keyboard focus, coarse input, reduced motion/materials, realistic empty/populated/error/busy states, editor content, Tutor, Study Guides, Practice and physical Android IME/Back behavior.

A new committed visual baseline is accepted only after the intended appearance is reviewed. Do not overwrite the historical Stage-0 baseline to make intentional differences disappear.
