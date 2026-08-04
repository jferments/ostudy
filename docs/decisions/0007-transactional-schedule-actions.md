# ADR 0007 — Transactional multi-card schedule actions

- **Status:** Accepted
- **Date:** 2026-08-04
- **Baseline:** PR #22 merge `0978ee26fbc1c1c126d2f84691853a7aab032fd2`

## Context

Manual scheduling controls originally accepted an array of exact generated-card IDs, but the backend required that array to contain exactly one card. Search/Filter therefore implemented a visible multi-card action as a sequence of separate native commands and separate SQLite transactions.

If a later card failed, earlier cards remained changed. The frontend also had no durable identity for the complete selection, so an ambiguous response could not be retried as one operation.

## Decision

### One bounded operation

`apply_schedule_action` accepts up to 10,000 unique exact card IDs. Rust sorts and deduplicates the IDs, validates every selected card and schedule, validates any supplied practice session and all session-card relationships, and calculates the complete change plan before the first write.

The backend uses an immediate SQLite transaction for:

- the parent schedule operation;
- card suspension and burial controls;
- schedule changes and revisions;
- per-card before/after audit rows;
- practice-session card dispositions; and
- practice-session counter and completion updates.

Any invalid member or failed write rolls back the complete selection.

### Parent operation ledger

Migration 10 adds `schedule_action_operations`. One row records the mutation ID, canonical request identity, optional session and reschedule context, requested count, changed count, unchanged count, rejected count, and timestamps.

`schedule_action_events` remains the immutable per-card audit ledger. Each changed card references its parent operation through `operation_id`.

Existing schema-8 events are preserved as one-card parent operations. Their event IDs and before/after values do not change.

### Canonical retry identity

The request fingerprint covers:

- sorted unique card IDs;
- action;
- optional due timestamp; and
- optional practice-session ID.

Repeating a mutation ID with the same canonical request returns the stored result without additional writes. Reusing it for another request returns `idempotency_conflict`.

The frontend retains one mutation ID after an ambiguous native response and reuses it only for the same complete selection and action.

### Explicit no-op semantics

Already-satisfied controls succeed without schedule revision or child-audit churn:

- suspend when already suspended;
- resume when already active;
- bury when already buried through at least the next local study day;
- unbury when already unburied;
- reset when already a canonical New schedule due now; and
- reschedule when the due timestamp is already exact.

The response preserves the existing `affectedCount` field for Stage 3.1 callers and adds authoritative requested, changed, unchanged, and rejected counts. A session dismissal is durable work even when the card-level control was already satisfied.

## Consequences

- A visible multi-card action is now all-or-nothing.
- Retry identity applies to the whole selected set.
- One-card Practice actions continue through the same command and response contract.
- Existing review history, FSRS schedules, and package formats remain valid.
- Future undo or maintenance tooling can group child events by one durable operation ID.
