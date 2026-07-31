# ADR 0005: Use direct ratings, bounded daily queues, and reversible reviews

- **Status:** Accepted
- **Date:** 2026-07-31
- **Applies from:** database schema 4 / Phase 2

## Context

The original practice flow asked whether an answer was Right or Wrong and then
asked a second confidence question for correct answers. That made every review a
two-step interaction even though the scheduler ultimately needs one of four
outcomes. Phase 2 also introduces daily limits, generated sibling cards, and an
Undo command. These features must preserve deterministic FSRS state and the
append-only review ledger.

## Decision

### One direct learner rating

After revealing an answer, the learner chooses exactly one score:

| Public score | Label | Canonical FSRS rating | Compatibility confidence |
| --- | --- | --- | --- |
| 0 | Wrong | Again (1) | `wrong` |
| 1 | Hard | Hard (2) | `not_confident` |
| 2 | OK | Good (3) | `somewhat_confident` |
| 3 | Easy | Easy (4) | `very_confident` |

The compatibility confidence column remains in `review_events` because schema 3
is already released. It is an internal mapping only; the product does not ask a
second question. Analytics, recent-deck summaries, keyboard shortcuts, and
interval previews use the public 0–3 vocabulary.

### Daily limits and generated siblings

The application stores separate daily limits for New cards and cards that have
already entered review. Session construction counts active, non-reversed review
events within the learner's local day and applies the remaining allowances to a
globally ordered due queue.

When related-card burying is enabled, session construction admits at most one
card per note. After an accepted review, active siblings are buried until the
next local day. Reverse, Fill in the blank, and template-generated siblings
therefore do not appear back-to-back in one session.

### Undo without rewriting history

Every accepted Phase 2 review stores the exact before/after schedule and the
relevant card-state snapshots. Undo:

1. finds the latest active review in the current session;
2. refuses if that card has a later active review, using the same
   `reviewed_at`, `created_at`, and ID ordering used to select reviews;
3. restores the exact pre-review schedule, Struggling/suspension state, and
   sibling burial values;
4. clears the session-card rating and recalculates session counters; and
5. appends a `review_event_reversals` row instead of deleting or editing the
   original event.

Active analytics, daily-limit counts, recent-deck summaries, and Struggling-card
calculations exclude reversed events. The immutable event remains available for
audit and repair.

## Consequences

- A review is one reveal followed by one rating decision.
- Existing schema-3 databases upgrade without rewriting historical events.
- Product labels are ordinary language while FSRS receives its canonical four
  ratings.
- New/review limits are deterministic and based on the learner's local-day
  boundary supplied by the desktop client.
- Related cards are less likely to reveal one another's answers during the same
  session.
- Undo requires additional snapshot tables and transaction logic, but it does
  not compromise the append-only ledger.
- Undo is intentionally limited to the latest active review in the current
  session and cannot cross a later review of the same card.
