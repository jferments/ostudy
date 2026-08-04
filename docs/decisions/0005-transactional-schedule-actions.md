# ADR 0005 — Transactional multi-card schedule actions

- **Status:** Accepted for Phase 2 Stage 3.2 implementation
- **Date:** 2026-08-04
- **Base:** `0978ee26fbc1c1c126d2f84691853a7aab032fd2`

## Context

The first scheduling-controls implementation accepted one exact card per native command. Search/Filter therefore applied a multi-card action through a sequence of independent commands. A later card could fail after earlier cards had already committed, so the selected set was not an atomic operation.

## Decision

### One bounded transaction

`apply_schedule_action` now accepts a deduplicated set of exact generated-card IDs, sorted into canonical order and limited to 10,000 cards. Rust validates every card, schedule, optional practice session, and session-card relationship before the first write.

The parent operation, card controls, schedule revisions, per-card audit events, practice-session dispositions, and session counters are written inside one immediate SQLite transaction. Any error rolls back the entire selected set.

### Parent and child audit records

Migration 10 adds `schedule_action_operations` as the operation ledger. It stores the mutation ID, canonical request identity, optional session and due-date context, action, aggregate counts, and timestamp.

`schedule_action_events` remains the per-card audit ledger. Changed cards reference one parent operation and preserve their before/after schedule and control values. Existing one-card events migrate losslessly into one-card parent operations.

### Retry identity

The canonical request identity covers sorted unique card IDs, action, optional due time, and optional practice-session ID. Repeating the same mutation ID with the same request returns the stored result without additional writes. Reusing it for another request returns `idempotency_conflict`.

The frontend retains one mutation ID after an ambiguous response and reuses it only for the same complete selection and action.

### Explicit no-ops

Already-satisfied cards are successful but unchanged. Examples include suspending an already suspended card, resuming an active card, repeating the same bury deadline, resetting a card already New and due, or rescheduling to the existing due time.

No-op cards do not increment revisions or create child audit rows. The parent operation records authoritative requested, changed, and unchanged counts. A necessary practice-session dismissal still counts as durable work even when the card-level control was already satisfied.

### Rejected requests

A request containing an invalid card or invalid session relationship commits nothing and returns a structured error. Committed operations currently have a rejected count of zero; that field is reserved for a future explicitly designed partial-acceptance contract.

## Consequences

- Multi-card schedule actions are all-or-nothing.
- One batch replaces repeated IPC calls and repeated SQLite transactions.
- One-card callers continue to use the same contract.
- Existing review history and `.studydeck` files are unchanged.
- Analytics and future maintenance tooling can group card-level events by one durable operation ID.
