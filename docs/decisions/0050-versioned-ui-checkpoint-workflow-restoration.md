# ADR 0050: Versioned UI checkpoint and internal-workflow restoration

- **Status:** Accepted
- **Date:** 2026-08-27
- **Related:** ADR 0032, ADR 0049, `docs/architecture/ui.md`

## Context

The released device-local UI checkpoint uses schema version 1 and stores
`practice` in the same section field as visible application destinations. After
Practice became an internal workflow, that representation could still resume a
real native session, but it could not represent Study Guides or the logical
destination to return to after the session ended. Generic and lifecycle saves
therefore omitted Study Guides and active Practice.

The checkpoint is a presentation recovery hint, not learner-domain authority.
SQLite remains authoritative for practice sessions, card ordering, revealed
state, reviews, and schedules. Runtime route/request identities are consumable
frontend events and cannot be reused after process restoration.

## Decision

ostudy writes `device-state/ui-checkpoint-v2.json`. Schema v2 has two separate
parts:

- one visible destination from Flashcards, Study Guides, Documents, Analytics,
  or Settings, with bounded Flashcards workspace/deck or Analytics state where
  applicable; and
- an optional resumable Practice hint containing session identity, cursor,
  revealed state, and a logical visible return target.

The v2 file never stores route transition IDs, launch request IDs, card content,
review submissions, credentials, or learner history. The frontend allocates new
transition/request identities when it reconstructs a saved deck, Search, or
Analytics route. A missing saved deck falls back to the Flashcards root without
discarding a valid resumable session.

Rust owns strict, unknown-field-rejecting v1 and v2 readers. It reads v2 in
preference to v1. It consults v1 only when v2 is absent; an invalid or
quarantined v2 file therefore cannot resurrect older v1 state. A valid v1 file
is normalized to the canonical v2 DTO, written as v2, and retired. A v1 Practice
checkpoint with a session retains the session ID, position, and revealed hint
and receives a deterministic Flashcards return target. A setup-only v1 Practice
checkpoint becomes Flashcards/Decks with no internal workflow. Legacy Cards,
Templates, and removed Create workspace values retain their established safe
normalization.

Saving v2 retires any remaining v1 file. Clearing the checkpoint removes both
generations. Both files remain outside SQLite, complete-library backup, and
device synchronization.

The application controller owns checkpoint construction. `PracticeScreen`
reports its latest in-memory cursor but does not write a section checkpoint.
Normal and background/suspending/termination lifecycle saves use the same
snapshot. Platform Back retains overlay precedence and the active-session exit
guard: cancel preserves the session; confirmation completes it before returning
to Flashcards.

## Consequences

- Existing released checkpoints remain readable without a database migration.
- Study Guides and every maintained visible destination can be restored.
- Resumable Practice sessions retain logical return behavior across process
  restart while native session state remains authoritative.
- Checkpoint save/reconciliation failure remains a presentation failure and
  cannot turn an already committed review or session mutation into a failure.
- Stage 3 still owns relocation and removal of the retained Practice setup,
  recent-practice, and nested guide composition.
