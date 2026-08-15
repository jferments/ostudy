# Flashcards workspaces, folders, ordering, and Search

The user-facing Flashcards area contains five independently mounted workspaces:

- **Decks** organizes and opens study decks.
- **Templates** manages reusable data-only card templates.
- **Search** finds cards across decks and supports bulk management.
- **Import/Export** groups deck transfer and complete-library backup tasks.
- **Create Cards** provides single-card and bulk-entry authoring.

Switching between these workspaces changes presentation state only. The active workspace is stored in the device-local UI checkpoint so the app can restore a useful destination without putting card content or credentials into that checkpoint.

## Deck folders

Folders provide one optional organization level above decks. A deck belongs to at most one folder; a deck without a folder appears under **Unfiled decks**. Folders do not own cards and do not alter scheduling or analytics attribution.

Creating, renaming, moving, reordering, or deleting a folder is a transactional native operation. Each organization snapshot has a revision number. A write based on a stale revision fails visibly instead of silently overwriting another window's change. The interface immediately reloads the authoritative organization snapshot so a subsequent action uses the current revision rather than repeatedly retrying stale state.

Deleting a folder moves its decks to Unfiled. It never deletes those decks, their cards, review history, schedules, tags, assets, or analytics.

Deck creation, archive changes, restoration, soft deletion, and permanent purge also advance the shared organization revision. The Decks workspace refreshes organization state when those lifecycle changes appear in the deck list, including after returning from nested deck editing or using Undo. This prevents an apparently unrelated deck operation from leaving a stale drag or reorder operation eligible to commit.

## Moving and ordering

Pointer users can drag a deck by its dedicated handle. Dragging is an enhancement rather than a required input method. Every deck also exposes a **Move** action and explicit up/down controls. Folders have equivalent up/down controls.

Nested drop targets stop propagation once a deck row or folder accepts a drop, so one pointer gesture can issue at most one revisioned move. The native command receives the target folder, target index, and expected organization revision. It rewrites the affected sequence to stable spaced sort values inside one immediate transaction. If the operation fails, the optimistic presentation is replaced with the authoritative native snapshot before the interface accepts another organization mutation.

## Comfortable and compact views

Comfortable view shows descriptions and progress detail. Compact view reduces vertical space while preserving deck name, card count, ready count, movement controls, and study actions. Density is a presentation preference and does not change the durable folder or deck order.

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

Individual `.studydeck` exports remain deck-scoped. They do not export a containing folder because folder membership is library organization rather than a property required to study the deck. An imported deck therefore appears under Unfiled until the user moves it.
