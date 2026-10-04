# ADR 0055: Subjects replace deck folders and document collections

- **Status:** Accepted
- **Date:** 2026-10-04
- **Applies from:** `ostudy-release-v1` schema 6

## Context

A course used to live in two unrelated groupings. Deck folders grouped decks in Flashcards. Document collections grouped documents in Documents, could share a document among several collections, and built a study bundle whose deck held copies of the documents' generated cards. A copy had its own schedule and its own review history, so studying a collection split one fact into two schedules and two histories, and the original card never learned from reviews of its copy.

## Decision

A **subject** is the overhauled collection. It holds a course's documents, guides and decks. Every deck and every document belongs to at most one subject, or to **No subject**. Subjects do not nest. Studying a subject is a live view over the original cards of the decks filed in it: one card and one schedule per fact, never copies.

### Storage

Subjects reuse the `document_collections` table and its ids, because study targets, collection guides, knowledge views and generation operations already reference it. The table is not dropped and recreated, which would cascade into `study_targets`. Schema 6:

| Change | Meaning |
| --- | --- |
| `document_collections.parent_id` dropped | It was never used. Its one-level triggers and index go first, because SQLite refuses to drop a column they name. |
| `decks.subject_id`, `documents.subject_id` | A deck's or document's subject. `ON DELETE SET NULL`: deleting a subject sends its decks and documents to No subject. They replace `decks.folder_id`, `deck_folders` and `document_collection_memberships`. |
| `document_study_ranges` | "Study only these parts" of a document inside its subject. It replaces `document_collection_ranges`. A document that leaves every subject loses its ranges. |
| `card_merges` | One row per retired collection copy card, naming the surviving original card and both notes. Merges are one level deep, and triggers check that each row names the notes that own its cards. |
| `subject_filing_reviews` | The **Check these** list: every subject, deck, document and kept copy where the upgrade or a sync had to choose. Entries can be dismissed. The list stays on its device. |

The library revision (`library_organization`) guards the subject list and deck filing and order. The document filing revision (`document_collection_organization`) guards document filing and study ranges, so moving a deck never makes a document's study materials look out of date.

### Upgrade rules

Release migration 6 is deterministic: it reads no clock and makes no random ids, so two devices that upgrade the same synchronized rows reach the same result.

- Every deck folder becomes a subject and keeps its id. A folder merges into the collection with the same name only when the match is unambiguous: case and surrounding spaces are ignored, and exactly one folder and one collection use the name. Otherwise both stay, and both are listed.
- A document in several collections stays in the one it joined first; the others lose it, and it is listed. A document in no collection follows the folder of its generated deck.
- Page ranges are kept from the collection the document stays in. Ranges set in the collections that lost it are dropped, and their count is listed.
- A study-bundle deck goes to its collection's subject. A deck that documents generated or link to goes to their subject. Every other deck stays in its folder's subject. Decks that left their folder's subject are listed.
- A collection copy merges back into its original only when it has exactly one origin, is live and unedited, its original is live in a deck that is neither deleted nor archived while the copy is studied, and every copy card pairs with an original card by direction and template. The copy is retired (`deleted_at`), moves into its original's deck and is recorded in `card_merges`. Copies the learner edited, and copies that could not pair, stay as the subject's own cards and are listed.
- The surviving card keeps the whole schedule of the pair member studied, reset or rescheduled most recently, read from the review and schedule-action ledgers. `card_schedules.last_review_at` is not used, because a new schedule stores its creation time there. Ties go to the original. Its schedule revision moves past every member's.
- Tags added to an unedited copy carry over to the original.
- A study-bundle deck the merge leaves without live notes is archived, as deck merging does with an emptied source deck.

### Review history stays where it was recorded

The upgrade never moves or rewrites a review. A retired copy keeps its `review_events` rows, and every reader of review history resolves merges with `COALESCE(merged.surviving_card_id, r.card_id)`: analytics, history and recent reviews, Search/Filter reviewed filters, the learning signals that practice selection and workload limits use, and the deck card list's practiced order. A merged copy's reviews count under its original.

The schema-6 preservation contract in the per-version registry declares the projected, added and dropped tables. Review evidence still has to come through identical, so a migration that rewrote or moved a review would be refused.

### Live study

A subject's flashcards are its live, unarchived decks when practice starts. A collection's **Create study materials** no longer copies cards: its Flashcards step succeeds when the subject has a card to study, and its button starts practice over the subject. The optional flashcard instructions for collections are gone, since there are no copies left to select from. Collection guides are unchanged.

### Native API and adapters

Typed commands list, create, rename, reorder and delete subjects, move a deck or a document to a subject or to No subject, set a document's study ranges, and read and dismiss the Check these list. The deck folder and document collection commands are adapters over the same storage, so today's screens keep working until the Library replaces them. Deleting a folder that also holds documents or study materials is refused there and done from Documents instead.

### Synchronization

Subjects travel with either the Flashcards or the Documents category. `card_merges` travels with Flashcards and `document_study_ranges` with Documents. The Check these list never travels; the merge adds its own entries when it settles a filing conflict or receives an edit to a copy this device retired. When both devices changed a card's schedule, the merge keeps the whole row studied last (then changed last) and records the other as a conflict, so a merged original never mixes the fields of two schedules.

Packages now record 30 plus the release schema version as their schema number. Builds before schema 6 refuse a number above 30 with "Update this device first", so they refuse a package from an updated device. An updated device migrates a package from an older device before merging it.

### Deletion

Finally purging a surviving card's deck or note also purges the retired copies merged into it, so `card_merges` never outlives a survivor. Purging a deck that holds retired copies whose original lives elsewhere moves those copies to their original's deck first, so their history is kept.

## Consequences

- An older build cannot open a schema-6 library or take a synchronization package from a schema-6 device. Upgrading takes the usual pre-upgrade checkpoint, which can restore the schema-5 library.
- A `.ostudybackup` made before the update still restores: the restore upgrades its private copy with the same migration, so it gets the subjects an in-place upgrade makes.
- A collection's study bundle deck is no longer where its flashcards live. Existing bundle decks stay as ordinary decks, archived when the merge emptied them.
- A synchronization package from a device without the update can repeat choices the upgrade made, such as a folder and collection name merge, or file a deck differently. The merge keeps one result and lists the item. Upgrading every device before the next sync avoids this.
- The Library screens that replace folders and collections are a later change. Until then, folder and collection screens show subjects through the adapters, and the Check these list has no screen of its own.
