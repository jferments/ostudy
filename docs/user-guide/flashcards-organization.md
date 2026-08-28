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

Pointer users can drag a deck or folder by its dedicated handle. Dragging is an enhancement rather than a required input method. The same focused handle provides a keyboard alternative:

- **Up**, **Down**, **Home**, and **End** reorder within the current group;
- **Left** and **Right** move a deck between folders and Unsorted decks.

The visible arrow and Move controls are not duplicated in each row. Screen-reader-only move actions remain available, and successful operations are announced through the live region.

Nested drop targets stop propagation once a deck row or folder accepts a drop, so one pointer gesture can issue at most one revisioned move. The native command receives the target folder, target index, and expected organization revision. It rewrites the affected sequence to stable spaced sort values inside one immediate transaction. If the operation fails, the optimistic presentation is replaced with the authoritative native snapshot before the interface accepts another organization mutation.

## Compact deck presentation

Decks use one compact row presentation on every platform. The removed Comfortable/Compact preference no longer creates divergent layout states or persists an obsolete setting. Compact spacing preserves deck name, card count, ready count, movement handle, and study actions while coarse-pointer controls retain full touch targets.

The drag handle no longer uses a permanent vertical separator. Its icon, focus state, hover state, cursor, accessible instructions, and touch target identify the interaction without adding visual clutter.

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
