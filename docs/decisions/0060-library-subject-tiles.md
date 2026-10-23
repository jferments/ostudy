# ADR 0060: Library opens on subject tiles; Notes is a destination

- **Status:** Accepted
- **Date:** 2026-10-23
- **Supersedes:** the Library layout in [ADR 0059](0059-one-subject-list-in-library.md)

## Context

[ADR 0059](0059-one-subject-list-in-library.md) made Library's left pane the
one list of subjects, with Overview, Lessons, Study guides, Documents, Notes and
Flashcards as tabs under it, and an Everything scope over the whole library.
The maintainer found the Overview page unhelpful and the Everything title
confusing: the tabs read as Library's own pages rather than a subject's, and
they asked for "Library" to be a page of subject tiles with the tabs only inside
a subject, and for Notes to be its own destination (2026-10-23).

## Decision

- Library opens on its own page, titled Library: each subject as a tile with a
  figure picked from its documents (or its initials), what it holds, its due
  count, its menu and a button for each kind of material it has, then No
  subject and New subject. The section keeps the `overview` id so saved
  checkpoints still name it.
- A subject's pages are tabs under its header: Flashcards, Study guides,
  Lessons, Podcasts and Documents, with a back link to Library. Podcasts is a
  new section and checkpoint value.
- All cards (the Workbench on every card, which Search also opens) shows no
  subject tabs. There is no Everything page for documents, guides or lessons.
- Notes is a main navigation destination between Library and Tutor. Settings
  is no longer a destination: it sits with Search at the rail's foot and in a
  phone's top bar.
- The home summary reports each subject's guide, lesson and podcast counts and
  its tile figure: the most worth-studying, most important figure covering at
  least 3% of its page.
- The UI checkpoint keeps no subject, so restoring a subject's page other than
  Flashcards opens Library's page.

## Consequences

- No migration. The checkpoint gains the `podcasts` section; an older build
  that reads a checkpoint saved on Podcasts sets it aside as invalid and opens
  on Home; no library data is affected.
- The Library subject pane, its phone sheet, the Overview page and the
  Everything grouping in Lessons are removed.
- Later work in the same redesign moves every material into a subject, adds a
  New study set sheet, gives Notes folders and makes the lecture the lesson's
  default view; each records its own decision where it changes a contract.
