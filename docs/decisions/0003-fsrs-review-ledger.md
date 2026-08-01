# ADR 0003: Separate the FSRS schedule from the immutable review ledger

- **Status:** Accepted
- **Applies from:** ostudy 0.1.0 / database schema 3

## Context

A learner must record how difficult recall was after revealing an answer. The
product presents one four-level scale numbered 0–3, while FSRS uses the canonical
ratings Again, Hard, Good, and Easy.

A single mutable database row is sufficient to find the next due date, but it is
not sufficient for trustworthy analytics, scheduler migrations, or debugging.
If current schedule state were the only record, each review would erase the
state that preceded it.

## Decision

Store review data in two related structures:

1. `card_schedules` contains one mutable, materialized FSRS state per active
   card. It is optimized for due-card queries.
2. `review_events` is append-only. It stores the learner-facing outcome, the
   canonical FSRS rating, response time, and complete before/after scheduling
   state for every accepted review.

The learner-facing mapping is:

| Score | Display label | Durable outcome | FSRS rating |
| --- | --- | --- | --- |
| 0 | Wrong | `wrong` | Again (1) |
| 1 | Hard | `not_confident` | Hard (2) |
| 2 | Medium | `somewhat_confident` | Good (3) |
| 3 | Easy | `very_confident` | Easy (4) |

The existing outcome strings are retained for schema compatibility. Analytics
converts the canonical rating to the displayed confidence score with
`rating - 1`.

Submitting a review is one `BEGIN IMMEDIATE` transaction. The transaction
validates the session, calculates the FSRS transition, inserts the review event,
updates the materialized schedule, marks the session card as rated, updates
session counters, and commits. Any failure rolls the whole operation back.

Each frontend submission includes a unique `submission_id`. The database has a
unique constraint on that value, and the command returns the existing result
when the same submission is retried. This prevents double clicks and IPC retries
from recording duplicate reviews.

The external `rs-fsrs` crate is isolated in `src-tauri/src/scheduler.rs`.
Database rows and Tauri DTOs use application-owned types so a dependency update
cannot silently redefine the durable schema.

## Consequences

- Due-card queries remain fast.
- Analytics can be reproduced from immutable events.
- Existing review history remains valid after the user-interface change.
- A future scheduler version can replay or migrate history explicitly.
- Review submission code must preserve transactional and idempotency invariants.
- Imported decks receive fresh local schedules; personal review history is not
  included in `.studydeck` files.
