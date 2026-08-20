# ADR 0039: Root-activated Flashcards routes and shared responsive layout modes

- **Status:** Accepted
- **Date:** 2026-08-20
- **Scope:** Flashcards navigation and responsive UI stages 1–2
- **Supersedes:** The Create Cards workspace and implicit nested-navigation portions of ADR 0034

## Context

The route-backed Flashcards workspace introduced separate Decks, Templates, Search, Import/Export, and Create Cards destinations. In practice, Create Cards only selected a deck before opening the existing single-card editor or bulk-entry dialog. It duplicated controls that are more useful inside the destination deck.

Nested Decks navigation was also implicit. The application controller knew that the Decks workspace was selected, while the child screen independently tracked whether it showed the library, a deck, or a card editor. The parent detected nesting by searching rendered DOM for a `.back-button`. Re-selecting Decks therefore did nothing because the selected workspace had not changed.

Responsive behavior was similarly fragmented. The shell and feature screens used overlapping viewport breakpoints, producing an icon-only medium sidebar while individual workspaces independently switched between phone and desktop layouts. Tablet-sized devices could receive contradictory layout assumptions.

## Decision

Flashcards uses explicit root-activated route state:

- Decks has library and deck destinations;
- Templates, Search, and Import/Export have explicit root destinations;
- activating a workspace tab always navigates to that workspace root, even when it is already selected;
- activating the primary Flashcards navigation destination always navigates to the Decks library root;
- Android/platform Back follows explicit nested state rather than querying DOM for a back button;
- legacy device checkpoints that name the removed `create` workspace normalize to the Decks library.

Create Cards is removed as a workspace. Single-card and bulk-card authoring remain the existing canonical editor and bulk-entry workflows, launched from the current deck's card table. The deck is fixed as the bulk-entry destination when launched there.

The application establishes three shared responsive layout modes based on available CSS viewport width rather than platform identity:

- `compact`: phone navigation and single-column content;
- `medium`: a narrow labeled sidebar and tablet-oriented content behavior;
- `wide`: the full sidebar and wide workspace layouts.

The root publishes the active layout and pointer modes as data attributes. The shell consumes those modes, while feature workspaces use container queries for layouts that depend on their actual content width. Compact visual density does not reduce semantic touch targets.

The deck-list density preference is removed. Deck rows use one compact presentation everywhere, and the obsolete persisted settings key is discarded during settings saves.

## Consequences

- Re-selecting Decks, Templates, Search, Import/Export, or the primary Flashcards destination has deterministic root behavior.
- Nested navigation and restoration are testable without relying on rendered class names.
- Card authoring is available where its destination is already known, reducing tab and hero-panel clutter.
- Tablet layouts no longer inherit an unlabeled icon rail or a mixture of unrelated phone/desktop breakpoints.
- Existing checkpoints and settings remain readable; removed presentation choices do not become long-lived compatibility APIs.
- CSS container queries become the preferred mechanism for feature layouts whose available width can differ from the viewport because of navigation chrome.

## Validation

Validation covers:

- same-tab and primary-navigation root activation;
- explicit Back behavior for deck details and card editing;
- legacy `create` checkpoint normalization;
- deck-scoped single and bulk authoring;
- removal of the density setting from TypeScript, Rust, persistence, tests, help, and documentation;
- compact Q/A previews without an arrow column;
- labeled medium navigation and compact/medium/wide root attributes;
- representative phone, tablet portrait, tablet landscape, and desktop viewport contracts;
- keyboard, coarse-pointer, reduced-motion, and semantic-theme behavior.
