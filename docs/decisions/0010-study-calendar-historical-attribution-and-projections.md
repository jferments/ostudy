# ADR 0010 — Study calendar, historical attribution, and bounded projections

- **Status:** Accepted for Stages 3.6–3.8 implementation
- **Date:** 2026-08-04
- **Base:** `5e1187a136c253e20b856a96ef6f301f40d28527`

## Context

Scheduling, workload, deck summaries, Search/Filter, and Analytics currently
reconstruct overlapping concepts with separate SQL. The most visible mismatch is
that deck badges count buried overdue cards while scheduled practice excludes
them. Calendar-day features also mix local-midnight boundaries with fixed
86,400,000-millisecond arithmetic. Historical review events are immutable, but
deck and tag attribution is reconstructed through mutable current organization.
Several normal list and practice paths additionally issue one or more SQL
statements for every result row.

Stages 3.6–3.8 address these concerns together so the application has one meaning
for studyability, one calendar boundary service, one explicit historical
attribution policy, and bounded list/detail projections.

## Decision

### Exact schedules and local calendar policy

FSRS continues to own exact due instants. Due timestamps are never rounded to a
calendar day. A command captures one study-time context containing the current
instant, local date, current local-day start, and next local-day start. Burial,
daily limits, reviewed-recency filters, manual date rescheduling, and Analytics
buckets use those calendar boundaries.

A **studyable** card belongs to a nondeleted, nonarchived deck; has a nondeleted
note and generated card; is not suspended; and is not buried beyond the captured
instant. A **studyable-due** card is studyable and has `due_at <= now`. This is the
user-facing count shown by decks, Practice, Search/Filter, and current-inventory
Analytics.

Archive is a reversible organization state. Archiving a deck suspends all of its
active cards in one transaction and removes the deck from normal studyable lists.
Unarchiving resumes only cards suspended by that archive operation; cards that
were already individually suspended remain suspended.

Manual date rescheduling sends a calendar intent to Rust rather than a browser-
computed noon timestamp. Today resolves to now; future dates resolve to the first
valid instant of that local date. The canonical intent is part of the retry
fingerprint, while the resolved timestamp is stored with the committed operation.

### Historical attribution policy

Accepted reviews record immutable event-time deck ID, tag IDs, presentation
variant, rendered question excerpt/hash, and local review date. Historical
Analytics groups by those stable IDs, but display names are resolved from the
current deck and tag rows. Renaming a deck or tag therefore changes its displayed
Analytics name without moving the underlying reviews to another identity.

Archived decks remain eligible for historical Analytics. Deleted decks do not:
all review and Analytics evidence owned by a deleted deck is hidden immediately
and permanently removed when the deletion Undo window expires. Undo before expiry
restores the deck and its still-retained history. Deleting a tag removes that tag's
historical attribution; renaming a retained tag changes the displayed name.

Current inventory and historical evidence are separate response sections. Current
inventory uses current studyability and organization. Historical metrics use
review attribution plus current names and deletion visibility.

Older reviews are upgraded once through a versioned repair. Their recoverable
attribution is marked as reconstructed rather than presented as exact event-time
capture. Canonical startup remains read-only after migration and repair ledgers
are complete.

### Bounded projections

List commands return summary rows. Full rich documents, asset lists, and reusable
Template graphs load only for one opened detail/editor item. Potentially large
lists use validated pages or windows.

Normal read paths must use a fixed number of SQL statements, or a documented
fixed-size ID chunk, rather than issuing queries per card, note, deck, tag, asset,
or template. Workload selection and weak-card classification use shared CTEs.
Search rendering operates from one page projection. Practice sessions return a
small first window and load later windows by position. Deck-card and template
libraries use summary projections.

## Consequences

- A forward migration adds archive-operation metadata and review-attribution
  tables/indexes.
- A one-time repair backfills historical attribution for existing review rows.
- Deck and tag DTO count names change to explicit source-note, generated-card,
  and studyable-due units; all callers are updated instead of retaining aliases.
- Deleted-deck history is retained only during the existing Undo window and is
  purged during deletion finalization.
- Complete library backups preserve all new state. Shareable `.studydeck` files
  continue to exclude schedules, sessions, reviews, and Analytics attribution.
- Query-count regressions become part of the native test suite.
