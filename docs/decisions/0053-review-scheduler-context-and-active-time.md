# ADR 0053: Record the scheduler's view and active study time for each review

- **Status:** Accepted
- **Date:** 2026-10-04
- **Applies from:** `ostudy-release-v1` schema 5

## Context

`review_events` stores each review's state, due time, stability and difficulty before and after FSRS scheduled it (see [the FSRS review ledger](0003-fsrs-review-ledger.md)). It does not store what the scheduler predicted at that moment or the elapsed time it used. Those values cannot be rebuilt later: the stored `elapsed_days` before a review describes the previous review, and the prediction depends on the parameters in use when the review happened. True retention, calibration of estimated recall and a future FSRS optimizer all need them.

Study minutes were also measured as wall-clock time per card, capped at 24 hours, including time while the app was hidden.

## Decision

Schema 5 adds one append-only side table, `review_event_scheduler_context`, with at most one row per review event. A new table, rather than new `review_events` columns, keeps the release upgrade on the existing additive path: the checkpoint preservation proof already accepts a new empty table, while changing an existing table's shape would need a new preservation contract.

The practice write path inserts the row in the same transaction as its review, for ordinary and checked-question reviews alike. A retry that returns an already committed review writes nothing. Each row records:

| Column | Meaning |
| --- | --- |
| `previous_review_at` | The card's previous review time; `NULL` for a New card, whose `last_review_at` holds its creation or reset time. |
| `elapsed_days` | The whole days since the previous review that FSRS used for this review (0 for a New card). |
| `retrievability` | The FSRS recall probability for the pre-review card, read before FSRS schedules it. `NULL` for a New card (the crate reports 0, which is not a prediction) and for a schedule whose stability cannot produce a probability. |
| `reps_before`, `lapses_before` | The review and lapse counts before this review. |
| `desired_retention` | The retention target used. |
| `scheduler_parameters_sha256` | SHA-256 of the canonical JSON of every other FSRS parameter: a format tag `ostudy-fsrs-parameters-v1`, the weights, maximum interval, decay, factor, short-term and fuzz switches. The crate's fuzz seed is excluded because it is recomputed for every review. A change to the weights or other parameters changes the hash. |
| `active_time_ms` | Milliseconds the card was visible before the rating, when the client measured it. |

Reviews recorded before schema 5 keep no row. The upgrade never backfills one, because the values were never known and an invented row would also conflict with rows synchronized from other devices.

### Active study time

The practice screen measures the time the card is visible, from when it is shown until the rating is submitted. The clock pauses while the document is hidden, between the platform's background and foreground lifecycle events, and while the card editor replaces the practice view. It uses the monotonic page clock. A retry resends the original measurement with the original request.

`active_time_ms` is optional in the review command. It is not part of the retry identity, so the `review:v1` fingerprint is unchanged and a retry reporting a different value returns the original review. The native command accepts 0 to 86,400,000 milliseconds. The client does not cap the raw measurement: a value outside that range is sent as unknown (`NULL`) instead. `response_time_ms` keeps its existing meaning, and analytics does not read active time yet.

## Consequences

- The side row is review evidence. Device synchronization merges it with the flashcard graph like `review_event_attributions`, a flashcards-unselected package removes it, a complete backup carries it, a `.studydeck` transfer never includes it, and deleting a review through finalized purge removes it by foreign-key cascade.
- An older schema-4 build cannot open a schema-5 library or a synchronization package from a schema-5 device. Upgrading takes the usual pre-upgrade checkpoint, which can restore the schema-4 library.
- Readers must treat a missing row or a `NULL` value as unknown. Older reviews, reviews from clients that do not measure active time and New-card predictions are all expected to be unknown.
