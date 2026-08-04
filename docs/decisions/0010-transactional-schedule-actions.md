# ADR 0010 — Transactional multi-card schedule actions

- **Status:** Accepted
- **Date:** 2026-08-04
- **Base:** `0978ee26fbc1c1c126d2f84691853a7aab032fd2`

## Context

The first scheduling-controls implementation accepted one exact generated card per native command. Search/Filter implemented a learner's multi-card action as a sequence of independent calls. That meant a later card could fail after earlier cards had already committed, leaving a partially changed selection. It also gave each card a different mutation identity and made ambiguous-response retry behavior difficult to reason about.

Stage 3.2 promotes the complete selected set to the durable operation boundary.

## Decision

### One bounded operation and one transaction

`apply_schedule_action` accepts a set of exact generated-card IDs. Rust validates every ID, card, current schedule, optional practice session, and session-card relationship before the first write. IDs are deduplicated and sorted into a canonical order, and a request is limited to 10,000 cards.

One immediate SQLite transaction commits:

- the parent schedule operation;
- suspension and burial state;
- schedule due times and revisions;
- changed-card before/after audit rows;
- practice-session dispositions; and
- refreshed practice-session counters and completion state.

Any validation or write error rolls back the entire selected set.

### Parent operation and changed-card events

Migration 10 introduces `schedule_action_operations`. One row stores the mutation ID, canonical request fingerprint, optional practice-session and due-date context, action, requested/changed/unchanged/rejected counts, and timestamps.

`schedule_action_events` remains the per-card audit ledger. Each changed card references its parent operation and records schedule and control state before and after the action.

Existing schema-8 one-card events are migrated losslessly into one-card parent operations. Their event IDs, card IDs, action, timestamps, and before/after values remain unchanged.

### Retry identity

The request fingerprint covers:

- sorted unique card IDs;
- action;
- optional reschedule timestamp; and
- optional practice-session ID.

Repeating the same mutation ID with the same canonical request returns the stored aggregate result without additional writes. Reusing it for a different request returns `idempotency_conflict`.

The frontend retains the exact mutation request after an ambiguous transport or native response and permits only that request to be retried. A structured backend rejection abandons the mutation ID.

### Explicit no-ops

Already-satisfied actions are successful no-ops:

- suspend an already suspended card;
- resume an already active card;
- bury a card already buried through at least the next local study day;
- unbury an unburied card;
- reset a card already in the canonical New-and-due state; or
- reschedule a card to its existing due timestamp.

A no-op does not increment the schedule revision and does not create a child audit event. The parent row records the authoritative unchanged count.

A required practice-session dismissal is still durable work even when the card-level control was already satisfied. In that case the event can contain equal before/after schedule revisions while the session disposition and counters commit atomically.

### Rejected count

A request containing an invalid card or relationship commits nothing, so committed operations currently have `rejected_count = 0`. The field is retained in the durable result so a future partial-acceptance policy would require an explicit contract decision rather than another storage redesign.

## Consequences

- Search/Filter schedule actions are all-or-nothing.
- One batch replaces repeated IPC calls and repeated SQLite transactions.
- One-card Practice actions continue through the same batch contract.
- Lost responses can be retried without duplicate mutations.
- True no-ops do not create revision or audit churn.
- Existing review history, FSRS schedules, cards, and deck packages remain valid.
- Future maintenance or undo tooling can reason about one parent operation rather than reconstructing a frontend loop.
