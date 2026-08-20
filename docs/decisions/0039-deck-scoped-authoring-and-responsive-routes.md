# ADR 0039: Deck-scoped authoring and explicit responsive Flashcards routes

- **Status:** Accepted
- **Date:** 2026-08-20
- **Scope:** Flashcards navigation, deck authoring, deck-list density, and responsive shell behavior
- **Supersedes in part:** ADR 0034's five-destination Flashcards workspace and medium icon-only navigation assumptions

## Context

ADR 0034 established a route-backed Flashcards workspace and semantic responsive UI foundation. The first implementation exposed Decks, Templates, Search, Import/Export, and Create Cards as peer destinations, while the deck library also carried a Comfortable/Compact presentation preference.

In use, card creation is more understandable when it is attached to the deck that will own the cards. A separate Create Cards destination duplicates deck selection and creates another navigation state for the same `CardEditorScreen` and `BulkEntryModal` workflows. The density preference similarly creates two visual variants without changing domain behavior.

The original responsive implementation also allowed individual features to select phone/desktop breakpoints independently. At intermediate tablet widths this could combine an icon-only shell rail with partially collapsed desktop feature layouts, producing an incoherent middle state.

Finally, nested Decks navigation was partly inferred from rendered `.back-button` elements. DOM shape should not be the application's navigation state or Android Back contract.

## Decision

Flashcards has four peer workspace roots: **Decks**, **Templates**, **Search**, and **Import/Export**.

Card authoring is deck-scoped. Opening a deck exposes **Bulk add** and **Add flashcard** in the card-table toolbar. Both entry points reuse the existing authoring controllers, validation, durable-draft, duplicate-detection, template, and managed-asset boundaries. The standalone Create Cards workspace and route identifier are removed.

Flashcards navigation uses an explicit route value. The Decks route distinguishes library, deck detail, and editor presentation and carries the current deck identity. Re-selecting a workspace activates that workspace's root. Device-local checkpoints persist only the stable workspace/root-or-deck destination; obsolete checkpoints that name Create Cards normalize to the Decks root. Back/navigation behavior is derived from route state rather than querying DOM elements.

The deck library has one compact visual presentation. The `deckViewDensity` application setting is removed from the bundled TypeScript/Rust settings contract, and a subsequent settings save deletes an obsolete persisted key. Compact visual density does not reduce keyboard or coarse-pointer interaction targets.

The application shell resolves three layout modes from available viewport width: compact, medium, and wide. Medium navigation remains labeled rather than becoming an icon-only rail. Feature workspaces use container queries for internal breakpoints so sidebar width and feature width do not create contradictory responsive states. Phone card previews use two horizontally efficient `Q:`/`A:` lines and omit the old arrow column.

## Consequences

- A learner chooses a destination deck before creating cards, eliminating duplicate deck-selection UI.
- The Flashcards tab strip is smaller and stable across phone, tablet, and desktop widths.
- Selecting Decks while viewing a deck has a deterministic root-navigation meaning.
- Android/platform Back no longer depends on a `.back-button` selector.
- Existing durable learner data is unchanged. Only device-local navigation checkpoint interpretation and an obsolete presentation setting are migrated.
- Existing `deckViewDensity` values have no semantic migration because both choices represented presentation only; the application converges on the compact presentation.
- Tablet behavior is governed by the shared shell mode plus each feature's actual container width, not by device-name detection.

## Validation

Validation must cover:

- workspace-root reactivation and nested Decks route restoration;
- legacy Create Cards checkpoint normalization;
- deck-scoped single-card and bulk authoring;
- absence of the Create Cards route and density setting from bundled contracts;
- keyboard and pointer folder/deck reordering after the compact-row change;
- two-line Q/A card previews without the arrow column;
- compact, medium, and wide shell/navigation behavior at representative phone, tablet portrait/landscape, and desktop widths;
- static, frontend, Rust compile/test, desktop native, Android package, and Android emulator qualification on the exact pull-request head.
