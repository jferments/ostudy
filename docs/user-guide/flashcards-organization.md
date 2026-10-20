# Subjects and deck organization

Subjects live in Library's subject list: beside the page on a wider window, and behind the subject's name at the top on a phone. The [Library Workbench guide](flashcards-workspace.md) covers the card list, authoring, selection, and contextual import/export; this guide explains how subjects hold decks and documents and how to keep them in order.

## Subjects hold decks and documents

A subject keeps one course's decks, documents, guides and lessons together. Choosing a subject in Library's list shows its decks on the **Flashcards** tab and its documents on the **Documents** tab; there is no second grouping inside either tab. A deck or document belongs to at most one subject. Decks and documents that are not filed anywhere appear under **No subject**, which the list shows only when it holds something. Subject membership does not alter card content, schedules, or review history.

Choose **New subject** at the end of Library's subject list, or on Home, to create a subject. On the Flashcards tab, the first entry lists the cards of all the subject's decks and offers **Practice…** for those decks and **New deck** to start a deck in it; **New deck** also ends the subject's deck list.

When subjects arrived, or when a sync met two choices, some entries may have had more than one sensible place. Each was given one, and **Check these (N)** appears at the top of the Workbench's decks, and Home shows the same **Check these**. It lists those entries with the reason for each. Move any that belong elsewhere with **Move to subject…**, and choose **Dismiss** once an entry looks right.

## Subject actions

The **…** beside the chosen subject's name offers **Rename…**, **Move up**, **Move down**, and **Delete subject…**. Move up and Move down change the subject's place in Library's list and on Home. No subject and Everything have no menu.

**Delete subject…** asks for confirmation. The subject's decks and documents move to No subject with their cards and schedules; the subject's study guide and lessons are deleted. It never deletes a deck. Use deck deletion only when you intend to remove study material and its associated data.

## Deck actions

Each deck in the tree has an actions menu with **Practice**, **Practice with options…**, **Rename…**, **Move to subject…**, **Move up**, **Move down**, **Merge into…**, **Export…**, **Archive**, and **Delete deck**. **Move up** and **Move down** reorder a deck within its subject. **Move to subject…** files it under another subject or under No subject; its cards keep their schedules.

A document is filed from its own **…** menu on the Documents tab: **Add to a subject…** or **Move to another subject…**, and **Remove from subject**.

There is no drag reordering in Library. Every move is a menu action, so it works the same with a pointer, keyboard, touch, or screen reader.

## Conflicts and refreshed state

Each organization change is checked against the library's current organization revision. If another window or a deck change got there first, the change is refused with a message and the tree reloads the current organization rather than silently overwriting the newer one. Review the refreshed tree before trying again. Creating, archiving, restoring, deleting, and undoing deck changes can also change the organization, even when you did not move anything yourself.

## Archive, delete, and recover

**Archive** keeps a deck for later and removes it from ordinary study availability. Archived decks are listed behind **Archived decks (N)** at the end of the Workbench's decks. Their menu offers **Restore**, **Export…**, and **Delete deck**; restoring makes the deck active again subject to its cards' scheduling and availability.

Deletion is different and can remove card-owned history after the recovery window is finalized. Use the offered **Undo** promptly when a deletion was accidental. Deleting a subject never substitutes for deleting a deck.

## Sharing and backup

A portable `.studydeck` is one deck's content and managed study assets, not a snapshot of its subject or the sender's schedules. A newly imported deck starts under No subject in the receiving library.

A complete `.ostudybackup` preserves coherent library organization along with personal study state and managed files. Use Settings → Backup and sync, and read [Backups, imports, and sharing](backups-imports-and-sharing.md) before restoring over an existing library.
