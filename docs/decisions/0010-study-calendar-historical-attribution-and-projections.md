# ADR 0010: Study calendar, historical attribution, and bounded projections

## Status

Accepted for Stages 3.6–3.8.

## Decision

ostudy uses one captured `StudyTimeContext` per command. FSRS due timestamps remain
exact instants. Local-calendar rules are limited to date boundaries, burial,
manual date intent, daily accounting, and Analytics ranges.

A **current generated card** belongs to a nondeleted card, note, and deck, and
the deck is not archived. A **studyable card** is current, unsuspended, and no
longer buried. **Studyable due** means studyable with `due_at <= captured now`.
Every user-facing “ready” or due count uses that definition.

Current inventory and historical evidence are separate query domains. Accepted
reviews receive immutable card/deck/tag/presentation/text-hash/local-date
attribution in the same transaction as the review and FSRS schedule update.
Legacy reviews are reconstructed once and marked
`legacy_current_state_backfill`.

Historical aggregates group by stable deck/tag IDs. A retained entity is
displayed under its current name, so later renames are visible without moving
history between identities. Archived decks remain eligible for historical
Analytics while contributing no current inventory. Final deck deletion is
intentionally destructive: all review events, attribution, session snapshots,
and session-card data for that deck are purged, and the deleted deck is excluded
from every Analytics dimension immediately.

Library reads are classified as summary, detail, aggregate, or bounded-window
projections. SQL statement count must be fixed, or grow only by documented
250-ID chunks, rather than one query per returned entity.

## Consequences

- Manual rescheduling sends a canonical date intent; Rust resolves the instant.
- Search uses explicit Availability and local-date review-recency filters.
- Deck and tag count fields state their unit.
- Large deck, template, practice, and Analytics reads cannot return unbounded
  editable graphs.
- Complete backups preserve attribution; shareable `.studydeck` packages do not
  include learner history.
- Once migration and repair ledgers are complete, ordinary startup performs no
  application-data writes.
