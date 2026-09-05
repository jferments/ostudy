# Folders, deck ordering, and selection

Organization lives in **Flashcards → Decks**. The [Flashcards guide](flashcards-workspace.md) covers authoring, Search/Filter, and contextual import/export; this guide explains how the library's folders and selection work.

## Folders are optional

A deck belongs to at most one folder. Active decks with no folder appear in **Unsorted decks**. Folder membership does not alter card content, schedules, or review history.

When active decks exist, the **+** beside Folders creates a folder. There is no separate full-size New folder button. The library shows Unsorted decks only when it contains decks. A completely empty library instead offers one create-deck state; an archived-only library still exposes its archived content.

Deleting a folder moves its decks back to Unsorted decks. It does not delete the decks or anything they own. Use deck deletion only when you intend to remove study material and its associated data.

## Collapse and expand

Select a folder heading to collapse or expand it. **Collapse all** is offered when there are folders to collapse; when they are all collapsed it becomes **Expand all**. The selection control sits beside it when the current scope contains selectable decks.

Collapse state changes presentation only. It does not remove membership, change order, alter practice availability, or mean the cards disappeared from the library. No collapse control is shown when there is nothing to collapse.

## Select decks deliberately

Use a row's checkbox to select a deck. **Select all** and **Deselect all** reflect the current scope and count; they are not shown for an empty scope. A contextual selected-deck toolbar provides the valid practice, export, move, archive/restore, and delete actions.

Selection and opening are different actions. Opening a selected deck and returning can preserve unchanged active selections during the same application process. Entries that changed revision or lifecycle are removed rather than allowing a bulk operation to act on stale assumptions. The selection itself does not persist through the restart checkpoint.

An unchanged empty deck may stay selected so it can be moved, archived, exported, or deleted, but it cannot start card practice. Archived selections cannot move or start practice until restored. A mixed active/archived selection explains which lifecycle subset Archive or Restore will affect; read that guidance before applying a bulk operation.

## Pointer movement

Start a drag from an open, non-interactive part of an active deck or folder row. Buttons, checkboxes, menus, links, editable content, and selected text keep their normal behavior. A small motion remains an ordinary click until the drag activates.

The preview names the destination. Highlighting identifies a valid or invalid target, and a separate insertion line shows the exact before/end/empty boundary. You can reorder folders, reorder decks inside a group, or move decks between folders and Unsorted decks. A folder can expand after hovering during a drag.

Canceling, hovering, scrolling, dropping on an invalid target, or dropping at the unchanged position does not save an organization mutation. The completed move, not every pointer update, becomes the durable operation.

## Keyboard and non-drag alternatives

Dragging is not required. Focus a reorder control and use Up/Down or Home/End to move within the current group. Deck controls also support Left/Right to move between folder groups and Unsorted decks. Screen-reader move actions and the selected-deck **Move** sheet provide alternatives.

The interface announces meaningful targets and completed moves. Compact rows retain touch-sized controls and visible keyboard focus; there is no separate Comfortable/Compact preference to change the underlying behavior.

## Conflicts and refreshed state

Organization operations use the revision of the library snapshot that was displayed. Another window or a deck lifecycle change may make that snapshot stale. In that case the app reloads authoritative state rather than silently overwriting the newer organization.

Review a conflict/error message and the refreshed layout before trying again. A temporary optimistic preview is not proof a move was saved. Creating, archiving, restoring, deleting, and undoing deck changes can also affect the organization revision, even when you did not directly drag a folder.

## Archive, delete, and recover

Archive keeps a deck for later and removes it from ordinary study availability. Restore makes it active again subject to its cards' scheduling/availability. The archived library area is not a second place to drag active decks around.

Deletion is different and can remove card-owned history after the recovery window is finalized. Use the offered Undo promptly when a deletion was accidental. Folder deletion never substitutes for deck deletion.

## Sharing and backup

A portable `.studydeck` is one deck's content and managed study assets, not a snapshot of its containing folder or the sender's schedules. A newly imported deck starts unsorted in the receiving library.

A complete `.ostudybackup` preserves coherent library organization along with personal study state and managed files. Use Settings → Backup/Sync, and read [Backups, imports, and sharing](backups-imports-and-sharing.md) before restoring over an existing library.
