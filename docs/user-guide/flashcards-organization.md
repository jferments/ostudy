# Flashcards workspaces, folders, ordering, and Search

The user-facing Flashcards area contains four independently mounted workspaces:

- **Decks** organizes and opens study decks. Single-card and bulk-card authoring are launched from the open deck.
- **Templates** manages reusable data-only card templates.
- **Search** finds cards across decks and supports bulk management.
- **Import/Export** groups deck transfer and complete-library backup tasks.

Activating a workspace always opens its root, even when the same tab is already selected. Activating the primary Flashcards navigation destination opens the Decks library root. The active workspace and an explicitly open deck are stored in the device-local UI checkpoint so the app can restore a useful destination without putting card content, credentials, or runtime request identities into that checkpoint. When a resumable Practice session was launched from Search or deck detail, that logical destination is restored with a fresh route transition after the session ends.

## Deck folders

Folders provide one optional organization level above decks. A deck belongs to at most one folder; a deck without a folder appears under **Unsorted decks**. Folders do not own cards and do not alter scheduling or analytics attribution.

Creating, renaming, moving, reordering, or deleting a folder is a transactional native operation. Each organization snapshot has a revision number. A write based on a stale revision fails visibly instead of silently overwriting another window's change. The interface immediately reloads the authoritative organization snapshot so a subsequent action uses the current revision rather than repeatedly retrying stale state.

Deleting a folder moves its decks to Unsorted decks. It never deletes those decks, their cards, review history, schedules, tags, assets, or analytics.

Deck creation, archive changes, restoration, soft deletion, and permanent purge also advance the shared organization revision. The Decks workspace refreshes organization state when those lifecycle changes appear in the deck list, including after returning from nested deck editing or using Undo. This prevents an apparently unrelated deck operation from leaving a stale drag or reorder operation eligible to commit.

## Moving and ordering

On a pointer device, start moving an active deck or folder from any open part of
its row. Buttons, checkboxes, menus, links, editable content, and selected text
continue to behave normally. A short movement remains an ordinary click; after
the drag activates, the preview names the destination and insertion position.
Folders accept a deck across their complete visible surface and can expand after
a brief hover.

Dragging is an enhancement rather than a required input method. The focused
keyboard reorder control retains these shortcuts:

- **Up**, **Down**, **Home**, and **End** reorder within the current group;
- **Left** and **Right** move a deck between folders and Unsorted decks.

The compact **Move selected** sheet and screen-reader-only move actions remain
available. Valid, invalid, and unchanged destinations are identified before a
move is saved, and meaningful target changes and completion are announced.

One completed pointer gesture can issue at most one validated revisioned move;
hovering, scrolling, cancellation, invalid destinations, and unchanged
positions issue none. The native command receives the target folder, target
index, and expected organization revision. It rewrites the affected sequence
to stable spaced sort values inside one immediate transaction. If the operation
fails, the optimistic presentation is replaced with the authoritative native
snapshot before the interface accepts another organization mutation.

## Compact deck presentation

Decks use one compact row presentation on every platform. The removed Comfortable/Compact preference no longer creates divergent layout states or persists an obsolete setting. Compact spacing preserves deck name, card count, ready count, keyboard movement, and study actions while coarse-pointer controls retain full touch targets.

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

The practice screen presents a decorative card stack based on cards remaining
in the session. Only the current card is exposed to assistive technology. A
thicker progress track displays the studied/remaining counts and rounded
percentage inside the track; its single accessible progressbar value also names
the current-card position. The active header does not duplicate that progress
or show a weak-card badge. Submitted reviews are durable before the completion
screen appears, and weak-card outcomes remain available to scheduling, Search,
completion, and Analytics. The completion summary reports cards studied,
average confidence, remembered percentage, difficult answers, and the fully
labeled confidence distribution.

## Backup and restoration

Folders and deck placement are SQLite library state introduced in schema version 26. Complete-library backup uses the same durable database snapshot and therefore preserves folder records, membership, ordering, and organization revision together with decks, cards, templates, assets, review history, and analytics.

Individual `.studydeck` exports remain deck-scoped. They do not export a containing folder because folder membership is library organization rather than a property required to study the deck. An imported deck therefore appears under Unsorted decks until the user moves it.
