# Flashcards workspaces, folders, ordering, and Search

The user-facing Flashcards area contains four independently mounted workspaces:

- **Decks** organizes and opens study decks and owns card authoring for each deck.
- **Templates** manages reusable data-only card templates.
- **Search** finds cards across decks and supports bulk management.
- **Import/Export** groups deck transfer and complete-library protection tasks.

Switching between these workspaces changes presentation state only. The active workspace and open-deck destination are stored in the device-local UI checkpoint so the app can restore a useful destination without putting card content or credentials into that checkpoint. Selecting a workspace that is already active returns it to that workspace's root; selecting **Decks** while a deck is open returns to the main deck listing.

## Deck folders

Folders provide one optional organization level above decks. A deck belongs to at most one folder; a deck without a folder appears under **Unsorted decks**. Folders do not own cards and do not alter scheduling or analytics attribution.

Creating, renaming, moving, reordering, or deleting a folder is a transactional native operation. Each organization snapshot has a revision number. A write based on a stale revision fails visibly instead of silently overwriting another window's change. The interface immediately reloads the authoritative organization snapshot so a subsequent action uses the current revision rather than repeatedly retrying stale state.

Deleting a folder moves its decks to Unsorted decks. It never deletes those decks, their cards, review history, schedules, tags, assets, or analytics.

Deck creation, archive changes, restoration, soft deletion, and permanent purge also advance the shared organization revision. The Decks workspace refreshes organization state when those lifecycle changes appear in the deck list, including after returning from nested deck editing or using Undo. This prevents an apparently unrelated deck operation from leaving a stale drag or reorder operation eligible to commit.

## Moving and ordering

Pointer users can drag a deck by its dedicated handle. Dragging is an enhancement rather than a required input method. Keyboard users can focus the same handle: Up and Down reorder inside a group, Home and End move to an edge, and Left or Right moves a deck between folder groups and Unsorted decks. Folder handles support Up, Down, Home, and End.

Nested drop targets stop propagation once a deck row or folder accepts a drop, so one pointer gesture can issue at most one revisioned move. The native command receives the target folder, target index, and expected organization revision. It rewrites the affected sequence to stable spaced sort values inside one immediate transaction. If the operation fails, the optimistic presentation is replaced with the authoritative native snapshot before the interface accepts another organization mutation.

## One compact deck presentation

The deck library uses one compact presentation instead of maintaining Comfortable and Compact modes. Every row keeps the deck name, card count, ready count, drag/keyboard handle, Practice action, and delete action. Coarse-pointer media rules preserve usable touch targets without expanding the ordinary visual density.

The compact presentation is device-independent and is not persisted as an application preference. Existing installations discard the obsolete `deckViewDensity` setting the next time settings are saved.

## Deck-scoped card authoring

Card creation belongs to the open deck. The card-table toolbar contains **Bulk add** and **Add flashcard**. Bulk entry fixes the current deck as the destination and uses the existing durable draft and duplicate-detection workflow. There is no separate Create Cards workspace.

Card previews are optimized for constrained widths. Narrow layouts show two text lines, **Q:** and **A:**, using the available horizontal space before ellipsis. The old arrow column and separate vertical Question/Answer labels are removed.

## Responsive layout ownership

The application shell resolves a compact, medium, or wide layout mode from available viewport space. The medium layout retains text labels in its navigation instead of becoming an icon-only rail. Feature workspaces then use container queries for their own internal layout decisions, so a tablet does not receive contradictory phone and desktop rules merely because the sidebar changed its available content width.

## Search and progressive disclosure

Search keeps the common controls visible:

- text query;
- deck or folder scope;
- sort order;
- one **All filters** action.

The filter sheet contains less frequently changed criteria: availability, current scheduling status, local review date, individual decks, folders, and tags. Changes apply immediately, and active criteria return to the results view as removable filter chips.

A folder scope expands to its current active deck identifiers. It does not create a temporary deck and does not change folder membership. Archived decks remain available through their dedicated library area but are excluded from ordinary active Search scope unless the underlying Search status explicitly includes them.

## Practice ownership

Review history and scheduling belong to generated cards, not folders. Moving a deck between folders has no effect on its cards. Moving a card between decks transfers that card and its review evidence according to the existing card-ownership rules; the folder containing either deck does not become an analytics owner.

The practice screen presents a decorative card stack based on cards remaining in the session. Only the current card is exposed to assistive technology. Submitted reviews are durable before the completion screen appears, and the completion summary reports cards studied, average confidence, remembered percentage, difficult answers, and the fully labeled confidence distribution.

## Backup and restoration

Folders and deck placement are SQLite library state introduced in schema version 26. Complete-library backup uses the same durable database snapshot and therefore preserves folder records, membership, ordering, and organization revision together with decks, cards, templates, assets, review history, and analytics.

Individual `.studydeck` exports remain deck-scoped. They do not export a containing folder because folder membership is library organization rather than a property required to study the deck. An imported deck therefore appears under Unsorted decks until the user moves it.
