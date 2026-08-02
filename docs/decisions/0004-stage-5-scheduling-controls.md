# ADR 0004 — Stage 5 scheduling controls and bounded workloads

- **Status:** Accepted for Phase 2 Stage 5 implementation
- **Date:** 2026-08-02
- **Base:** `56ef4f6864ffc6a752004bd7a540d12b4bb9d955`

## Context

ostudy already stores one mutable FSRS schedule per generated card and an
append-only review ledger. Stage 5 adds user scheduling controls and bounded
daily workloads without replacing FSRS, rewriting review history, or exposing a
large set of controls on every practice screen.

This first Stage 5 delivery intentionally excludes undo and the analytics-page
redesign. Those changes will be implemented separately after scheduling controls
are tested and merged.

## Decision

### Scheduler authority

`src-tauri/src/scheduler.rs` remains the only adapter around `rs-fsrs`. Graded
reviews continue to determine stability, difficulty, state, and normal due dates.
Workload policy only determines which eligible cards enter a bounded session and
the order in which they appear.

### Card-level controls

Scheduling actions operate on exact generated-card IDs:

- **Suspend** excludes a card until the learner resumes it.
- **Bury** excludes a card until the next local calendar day.
- **Reset** replaces the mutable schedule with a fresh New schedule due now while
  preserving the append-only review ledger.
- **Reschedule** changes only the next due time; it is not recorded as a review.

Forward, reverse, and cloze presentations therefore retain independent control
where they have independent card IDs.

### Durable auditability and retry safety

Manual schedule changes are recorded in an append-only `schedule_action_events`
table. Each request has a unique mutation ID so repeated clicks or retries cannot
apply the same action twice. A schedule revision increments after every graded
review or manual schedule mutation, allowing stale previews and mutations to fail
visibly.

### Daily workload limits

Global settings default to 20 new cards and 200 review cards per local calendar
day. Accepted review events consume the limits according to their pre-review
state:

- New consumes the new-card limit.
- Learning, Review, and Relearning consume the review limit.

Normal scheduled sessions enforce remaining limits without changing omitted
cards' due dates. Explicit **Extra** and **Custom** sessions may exceed limits,
but their accepted reviews still count toward the same day.

When due review demand exceeds remaining review capacity, normal new-card
introduction pauses. This protects retention instead of hiding backlog by moving
due dates.

### Needs more practice

A card is derived as **Needs more practice** when, since its most recent reset:

- its last two reviews were both Wrong; or
- at least three of its last five reviews were Wrong and it has not subsequently
  produced two remembered answers in a row.

It clears after two consecutive remembered answers. Hard, Medium, and Easy are
remembered answers. The state is derived from history rather than stored as a
mutable flag.

Needs-more-practice cards receive priority only among cards due on the same local
calendar day. An older overdue card remains ahead of a newer weak card.

### Interval previews

After answer reveal, Rust calculates all four possible FSRS outcomes from the
same current schedule, schedule revision, desired-retention setting, and preview
time. The frontend displays approximate intervals inside the existing rating
buttons. Submission remains authoritative and rejects a stale schedule revision.

### UI disclosure

Common workload information remains visible. Less-common controls live in the
existing Search/Filter selection action menu, compact per-card menus, and one
Practice **More** menu. Destructive reset requires confirmation. No permanent row
of scheduling buttons is added.

## Consequences

- A forward-only schema migration is required.
- Existing schedules and review history remain valid.
- `.studydeck` exports remain unchanged because scheduling controls, settings,
  and personal review history are local learner state.
- Analytics can later derive manual actions and workload behavior from ordinary
  stored data without changing this scheduling contract.
- Undo remains a separate future change because exact reversal requires a
  complete review-snapshot and reversal design.
