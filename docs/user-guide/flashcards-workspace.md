# Flashcards workspace

The **Flashcards** area is the main place to organize study material, create cards, find existing cards, and transfer study content.

Its compact heading is followed immediately by four workspace tabs:

- **Decks**
- **Templates**
- **Search**
- **Import/Export**

On compact mobile layouts the tabs form a two-by-two grid. They do not require horizontal scrolling. Tablet and compact-desktop layouts retain a labeled main navigation sidebar rather than reducing primary destinations to unexplained icons.

Selecting a workspace tab always opens that workspace's root. Selecting **Decks** while a deck is open therefore returns to the main deck listing. Selecting the primary **Flashcards** navigation destination has the same effect. Unsaved card or template changes still receive the normal discard confirmation before navigation proceeds.

The active workspace and an explicitly open deck are stored in the local presentation checkpoint so ostudy can restore the destination after restart. Card content, review history, credentials, and runtime transition IDs are not stored in that checkpoint. Older checkpoints that named the removed Create Cards workspace return safely to the Decks root. A resumable Practice session can reopen over this logical Flashcards destination and returns with a newly created route transition; if a saved deck no longer exists, the session remains resumable and its destination falls back to the Decks root.

## Decks

Use **Decks** to create decks, open them, add cards, start practice, organize them into folders, archive them, and remove them.

### Folders and Unsorted decks

Folders are optional. A deck belongs to at most one folder. A deck without a folder appears under **Unsorted decks**.

Folders do not own cards and do not change scheduling or analytics. Deleting a folder returns its decks to Unsorted decks while preserving every card, schedule, review event, tag, and managed asset.

### Dragging and keyboard organization

Use the dedicated drag handle to:

- reorder a folder;
- reorder a deck within its current group;
- move a deck into another folder;
- move a deck into **Unsorted decks**.

Touch dragging uses the same durable organization command as mouse dragging. A highlighted drop target shows where the folder or deck will be placed.

Dragging is not required. Focus a folder handle and use:

- **Up** or **Down** to move one position;
- **Home** or **End** to move to the beginning or end.

Focus a deck handle and use:

- **Up**, **Down**, **Home**, or **End** within the current group;
- **Left** or **Right** to move between folder groups and Unsorted decks.

The interface announces the completed move. Every operation includes the organization revision that was current when the action started. If another window changed the organization first, ostudy reloads the authoritative state rather than overwriting the newer change.

### Collapsing folders

Select a folder heading to collapse or expand it. Hovering the open-folder control displays **Collapse folder**; a collapsed folder displays **Expand folder**. **Collapse all** closes every open folder at once.

Collapse state is presentational only. It does not alter folder membership, deck order, practice availability, or backup contents.

### Compact deck rows

Deck rows use one compact presentation on desktop, tablet, and mobile. There is no Comfortable/Compact preference. The compact visual spacing does not reduce keyboard focus, drag handles, or coarse-pointer touch targets.

The drag handle is identified by its grip icon, focus ring, pointer feedback, and accessible instructions; it does not use a permanent vertical separator.

### Practice buttons

Every active deck with generated cards has a **Practice** button. Starting from a deck is an explicit request to study that deck, so the session follows the deck-practice workload rules rather than merely opening its editor.

Practice opens as an internal workflow, not another primary navigation
destination. Ending early returns to the same logical Flashcards context: the
Decks root for a library launch or the exact deck detail for a detail launch.

### Adding cards inside a deck

Open a deck to add study material. The card-table heading contains:

- **Add flashcard**, which opens the full rich card editor for the current deck;
- **Bulk add**, which opens the multi-row plain-text entry workflow with the current deck fixed as its destination.

Bulk entry does not ask you to choose the destination again. It retains duplicate analysis, durable drafts, tags, and the same canonical card-creation commands used elsewhere.

Card-list previews use compact question and answer lines. On narrow layouts each row displays one truncated `Q:` line and one truncated `A:` line, using the available horizontal width. The old arrow column and blank label rows are not used.

## Templates

Use **Templates** to define reusable card structures. A template contains named fields and safe placement rules for the question and answer sides. Templates are data-only; they do not execute scripts or arbitrary HTML.

A custom template is useful when many cards share the same shape, such as medication name, mechanism, adverse effects, and nursing considerations.

## Search

Use **Search** to find cards across decks and perform bulk actions.

The common controls stay visible:

- text query;
- deck or folder scope;
- sort order;
- **All filters**.

Open **All filters** for availability, scheduling status, last-studied date, individual decks, folders, and tags. Applied criteria appear as removable chips above the results.

**Select visible** selects only the loaded rows. **Select all matching** resolves the complete filtered result set before a bulk action.

Choose **Practice selected** to start an exact-card session for the selected
results. Ending that contextual session early returns to the Search workspace
with a fresh route transition rather than selecting a standing Practice
destination.

## Import/Export

Use **Import/Export** for deck-scoped content transfers and complete-library protection.

- `.studydeck` transfers one deck and its managed study assets without carrying the original learner's review history.
- `.ostudybackup` protects the complete local library, including folders, deck placement, templates, schedules, review events, analytics sources, settings, managed images, and retained documents. Provider credentials are excluded.

A newly imported deck begins in **Unsorted decks** because folder membership is organization state belonging to the receiving library.
