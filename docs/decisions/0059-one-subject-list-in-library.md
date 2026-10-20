# ADR 0059: One subject list in Library, with no collections in the interface

- **Status:** Accepted
- **Date:** 2026-10-20

## Context

[Subjects](0055-subjects.md) folded deck folders and document collections into
one subject per course, but the screens kept their old shapes. Library listed
subjects in its left pane, the Flashcards Workbench listed them again as the top
level of its tree, and the Documents page still showed collections with their
own create, rename and drag-and-drop actions. A learner saw the same course
under two names and in two lists, and the Overview tab repeated what the other
tabs already showed.

## Decision

- Library's left pane is the only list of subjects. Choosing one scopes every
  tab: Overview, Lessons, Study guides, Documents and Flashcards. Everything
  shows the whole library and No subject shows what is not filed.
- The subject header carries the subject's badge, name and counts, Practice a
  mix and Review N due, and one actions menu for renaming, reordering and
  deleting the subject. Subjects are created with New subject… in the pane.
- Overview is the subject's home: four facts (due now, remembered over 30 days,
  days studied this week, due in the next 7 days), up to four suggestions for
  what to do next, how each deck is going, and the last 7 days of reviews.
- The Workbench tree has no subject level. It lists the chosen subject's cards,
  its decks (each with its subject's badge under Everything) and its tags.
- Documents shows the chosen subject's documents as one list; Everything groups
  them by subject with No subject last. Filing, reading ranges and making the
  subject's study materials are row and page actions.
- The interface does not say "collection". Native code keeps the name: the
  `document_collections` rows are the subjects, the `collections.rs` commands
  and `src/ipc/collections.ts` gateway remain a compatibility API, and a
  subject's study target has kind `collection`.

## Consequences

- No migration and no new stored data. An older build opens the library.
- The collections workspace, its controller and its drag-and-drop helper are
  removed. Removing the native collection commands is a separate cleanup that
  must first trace their remaining callers and the generated command contract.
- [UI architecture](../architecture/ui.md) and the user guides describe the
  current screens.
