# Flashcards workspace

The **Flashcards** area is the main place to organize study material, create cards, find existing cards, and transfer study content.

Its compact heading is followed immediately by five workspace tabs:

- **Decks**
- **Templates**
- **Search**
- **Import/Export**
- **Create Cards**

On compact mobile layouts the tabs form a centered two-row arrangement: three tabs on the first row and two tabs on the second. They do not require horizontal scrolling.

Switching tabs changes presentation state only. The active tab is stored in the local presentation checkpoint so ostudy can restore the previous workspace after restart. Card content, review history, and credentials are not stored in that checkpoint.

## Decks

Use **Decks** to create decks, open them, start practice, organize them into folders, archive them, and remove them.

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

### Practice buttons

Every active deck with generated cards has a **Practice** button. Starting from a deck is an explicit request to study that deck, so the session follows the deck-practice workload rules rather than merely opening its editor.

### Comfortable and Compact density

**Comfortable** density shows more description and progress detail. **Compact** density fits more deck rows on screen. Density is presentation state only and never changes card content or durable ordering.

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

## Import/Export

Use **Import/Export** for deck-scoped content transfers and complete-library protection.

- `.studydeck` transfers one deck and its managed study assets without carrying the original learner's review history.
- `.ostudybackup` protects the complete local library, including folders, deck placement, templates, schedules, review events, analytics sources, settings, managed images, and retained documents. Provider credentials are excluded.

A newly imported deck begins in **Unsorted decks** because folder membership is organization state belonging to the receiving library.

## Create Cards

Use **Create Cards** for focused single-card authoring or bulk entry. Both routes use the same validation, duplicate analysis, durable draft, template, and managed-asset boundaries as card creation from inside a deck.
