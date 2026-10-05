# ADR 0056: One set of learning-metric definitions

- **Status:** Accepted
- **Date:** 2026-10-05

## Context

Analytics, practice sessions, question types and the deletion purge each counted "remembered" their own way. The purge recounted a session as `rating BETWEEN 2 AND 4` and ignored checked-question verdicts, so a session's correct rate could change days later when an unrelated card was deleted. Topics were "Needs attention" when any one card needed more practice, or when the remembered rate over all reviews fell below desired retention. That rate mixes first sightings of new cards, learning steps and early extra practice with real reviews, so a learner adding many new cards scored low by construction and most active topics read "Needs attention". "Ready now" counted every unstudied new card, and the workload sentence compared it with the review allowance, which excludes new cards. One number, the mean rating, was labeled "Average confidence" on a tile and "Average difficulty" in the chart.

The Progress page that replaces Analytics needs these numbers to agree everywhere.

## Decision

`src-tauri/src/learning_metrics.rs` owns the definitions. Every surface builds its SQL from its helpers:

- **Remembered** is a review's stored `answer_correct`: a checked question's first-check verdict, even when the learner overrode the scheduling rating; otherwise any rating above Again. Session counters, including the purge recount, restate the same rule over the session's cards because they are refreshed before the review row is written.
- **A retention review** is a review of a card in the Review state that was due (`state_before = 2 AND due_before <= reviewed_at`). New cards, learning and relearning steps, and early extra practice are excluded.
- **True retention** is remembered retention reviews over retention reviews. It is the number comparable with desired retention.
- **Ready now** is split into due reviews (any state but New, due by now) and available new cards.
- **Due today** counts reviews due before the next study day starts ([ADR 0054](0054-study-day-start.md)).
- **Average confidence** is the mean rating on a 0 (Wrong) to 3 (Easy) scale; it has that one name.
- **Needs more practice** stays the learning signal in `projections/learning_signals.rs`.

Topic status replaces the classification in [Learning analytics](0005-learning-analytics.md):

- **Building data:** fewer than 10 retention reviews in the selected range.
- **Needs attention:** true retention at least 10 points below desired retention.
- **Strong:** true retention at least 3 points above desired retention.
- **On track:** otherwise.

A struggling card no longer flags its topic; the topic's "Needs more practice" count still shows it. Trend still compares remembered rates between equal periods with at least five reviews each.

## Consequences

- No migration. True retention uses `state_before` and `due_before`, which every review already stores.
- Topics need more evidence before they are classified, so small libraries show "Building data" for longer.
- Analytics responses gain retention counts on the summary, timeline buckets, decks and topics; the ready split and due-today count on the current inventory; and a remembered count on question types.
