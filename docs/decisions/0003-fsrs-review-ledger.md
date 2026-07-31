# ADR 0003: Separate the FSRS schedule from the immutable review ledger

- **Status:** Accepted
- **Applies from:** ostudy 0.1.0 / database schema 3

## Context

The application now grades scheduled reviews. A learner first chooses whether an
answer was wrong or right; a right answer is refined as not confident, somewhat
confident, or very confident. Those four product choices map to the four FSRS
ratings Again, Hard, Good, and Easy.

A single mutable database row is sufficient to find the next due date, but it is
not sufficient for trustworthy analytics, scheduler migrations, or debugging. If
the current schedule were the only record, every review would erase the state
that preceded it.

## Decision

Store review data in two related structures:

1. `card_schedules` contains one mutable, materialized FSRS state per active
   card. It is optimized for due-card queries.
2. `review_events` is append-only. It stores the learner-facing outcome, the
   canonical FSRS rating, response time, and complete before/after scheduling
   state for every accepted review.

Submitting a review is one `BEGIN IMMEDIATE` SQLite transaction. The transaction
validates the session, calculates the FSRS transition, inserts the review event,
updates the materialized schedule, marks the session card as rated, updates
session counters, and commits. Any failure rolls the entire operation back.

Each frontend submission includes a unique `submission_id`. The database has a
unique constraint on that value, and the command returns the existing result
when the same submission is retried. This prevents double clicks and IPC retries
from recording duplicate reviews.

The user-facing mapping is:

| Learner choice | FSRS rating |
| --- | --- |
| Wrong | Again (1) |
| Right → Not confident | Hard (2) |
| Right → Somewhat confident | Good (3) |
| Right → Very confident | Easy (4) |

The external `rs-fsrs` crate is isolated in `src-tauri/src/scheduler.rs`.
Database rows and Tauri DTOs use application-owned types so a dependency update
cannot silently redefine the durable schema.

## Consequences

- Due-card queries remain fast.
- Analytics can be reproduced from immutable events.
- A future scheduler version can replay or migrate history explicitly.
- Review submission code is more complex because it must preserve transactional
  and idempotency invariants.
- Imported decks receive fresh local schedules. Personal review history is not
  included in the current `.studydeck` interchange format.
