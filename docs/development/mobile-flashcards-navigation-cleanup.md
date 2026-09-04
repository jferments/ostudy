# Mobile flashcards and navigation cleanup

This working specification records the acceptance boundary for the current UI cleanup. The pull request remains the implementation ledger; this document exists so the user-visible requirements are reviewable beside the source.

## Acceptance criteria

- **AC-01 — Empty library:** A fresh library shows one quiet empty-deck call to action. Empty folder and unsorted placeholders, zero-count Select all, irrelevant Collapse all, and folder help are absent.
- **AC-02 — Library controls:** New folder is an icon-only plus beside the Folders heading. Select all sits with the Collapse all / Expand all toggle and is shown only when selectable decks exist.
- **AC-03 — Flashcards destinations:** Flashcards exposes only Decks and Search/Filter. Card templates move to Settings, and Import/Export is no longer a workspace tab.
- **AC-04 — Contextual transfer:** Deck import is launched from the Decks page. Single-deck and selected-deck export use a save icon and preserve the current portable deck/share behavior through focused dialogs.
- **AC-05 — Compact navigation:** The bottom bar contains Flashcards, Study Guides, Documents, and Analytics. Settings is an upper-right gear action.
- **AC-06 — Study Guide re-selection:** Selecting Study Guides while a guide is open returns to the Study Guides library.
- **AC-07 — Compatibility:** Legacy checkpoints that name removed Flashcards workspaces recover to a valid current destination, and existing import/export, template, deck, Android, and accessibility behavior remains covered.

## Non-goals

- No scheduling, analytics, document-processing, AI-provider, database-schema, or portable-file-format changes.
- No removal of card-template or deck-transfer capabilities; only their presentation and navigation ownership change.

## Presentation ownership

- The Flashcards workspace owns only Decks and Search/Filter.
- Portable deck-file orchestration is presentation-independent in `DeckTransferController`; the Decks page and focused dialogs supply the UI entry points.
- Card-template management remains backed by the existing template controller and IPC contracts, but Settings owns its navigation entry.
- Compact primary navigation owns four direct learning destinations. Settings is a distinct upper-right app-bar action rather than a fifth bottom item or a More-menu destination.
- Re-selecting the active Study Guides destination is an explicit reset-to-library request, so nested viewer state does not trap the user.
