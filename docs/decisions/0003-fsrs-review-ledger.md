# ADR 0003: Separate the FSRS schedule from the immutable review ledger

- **Status:** Accepted
- **Applies from:** ostudy 0.1.0 / database schema 3
- **Phase 2 amendment:** ADR 0005 replaces the two-step learner interaction with a direct 0–3 rating while preserving the same canonical FSRS ratings and compatibility columns.

## Context

The application grades scheduled reviews using four outcomes that map to the
FSRS ratings Again, Hard, Good, and Easy. Schema 3 originally exposed those
outcomes through a two-step Right/Wrong and confidence flow. Phase 2 exposes the
same durable mapping directly as scores 0–3; see ADR 0005.

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

The durable mapping is:

| Phase 2 learner score | Compatibility confidence | FSRS rating |
| --- | --- | --- |
| 0 — Wrong | `wrong` | Again (1) |
| 1 — Hard | `not_confident` | Hard (2) |
| 2 — OK | `somewhat_confident` | Good (3) |
| 3 — Easy | `very_confident` | Easy (4) |

The confidence values remain compatibility data in the released schema and are
not a second user prompt.

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
