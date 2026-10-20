# Subjects and deck organization

Organization lives in the **Subjects** section of the Library Workbench's tree. The [Library Workbench guide](flashcards-workspace.md) covers the card list, authoring, selection, and contextual import/export; this guide explains how subjects hold decks and documents and how to keep them in order.

## Subjects hold decks and documents

A subject keeps one course's decks and documents together. It is the same grouping that **Library → Documents** shows, so a document filed under a subject appears under that subject in the tree, and choosing it opens Documents. A deck belongs to at most one subject. Decks and documents that are not filed anywhere appear under **No subject**, which the tree shows only when it holds something. Subject membership does not alter card content, schedules, or review history.

Choose **+** (**New subject**) beside the Subjects heading to create a subject, or **New subject** on Home. Choosing a subject lists the cards of all its decks; its header offers **Practice…** for those decks and **New deck** to start a deck in it.

When subjects arrived, or when a sync met two choices, some entries may have had more than one sensible place. Each was given one, and **Check these (N)** appears at the top of the section, and Home shows the same **Check these**. It lists those entries with the reason for each. Move any that belong elsewhere with **Move to subject…**, and choose **Dismiss** once an entry looks right.

## Expand and collapse

Use the arrow beside a subject to show or hide its decks and documents. When you have three subjects or fewer they start expanded; otherwise the subject holding the open deck does. Expanding and collapsing changes presentation only. It does not change membership or order, and it does not mean the cards left the library.

## Subject actions

Each subject's actions menu offers **New deck here**, **Rename**, **Move up**, **Move down**, and **Delete subject**. No subject offers only **New deck here**.

**Delete subject** asks for confirmation. The subject's decks and documents move to No subject with their cards and schedules, and the subject's overview guide is deleted. It never deletes a deck. Use deck deletion only when you intend to remove study material and its associated data.

## Deck actions in the tree

Each deck in the tree has an actions menu with **Practice**, **Practice with options…**, **Rename…**, **Move to subject…**, **Move up**, **Move down**, **Merge into…**, **Export…**, **Archive**, and **Delete deck**. **Move up** and **Move down** reorder a deck within its subject. **Move to subject…** files it under another subject or under No subject; its cards keep their schedules.

A document in the tree has **Open in Documents** and **Move to subject…**.

There is no drag reordering in the Workbench. Every move is a menu action, so it works the same with a pointer, keyboard, touch, or screen reader.

## Conflicts and refreshed state

Each organization change is checked against the library's current organization revision. If another window or a deck change got there first, the change is refused with a message and the tree reloads the current organization rather than silently overwriting the newer one. Review the refreshed tree before trying again. Creating, archiving, restoring, deleting, and undoing deck changes can also change the organization, even when you did not move anything yourself.

## Archive, delete, and recover

**Archive** keeps a deck for later and removes it from ordinary study availability. Archived decks are listed behind **Archived decks (N)** at the end of the Subjects section. Their menu offers **Restore**, **Export…**, and **Delete deck**; restoring makes the deck active again subject to its cards' scheduling and availability.

Deletion is different and can remove card-owned history after the recovery window is finalized. Use the offered **Undo** promptly when a deletion was accidental. Deleting a subject never substitutes for deleting a deck.

## Sharing and backup

A portable `.studydeck` is one deck's content and managed study assets, not a snapshot of its subject or the sender's schedules. A newly imported deck starts under No subject in the receiving library.

A complete `.ostudybackup` preserves coherent library organization along with personal study state and managed files. Use Settings → Backup and sync, and read [Backups, imports, and sharing](backups-imports-and-sharing.md) before restoring over an existing library.
