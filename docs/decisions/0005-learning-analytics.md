# ADR 0005 — Local learning analytics and progress visualization

- **Status:** Accepted for Phase 2 Stage 5B
- **Date:** 2026-08-02
- **Base:** `a72f79f4789f942828f1439e8a9652e2f809d9be`

## Context

ostudy already stores immutable review events, mutable FSRS schedules, practice
session counters, tags, and exact generated-card identities. The analytics stage
must help a learner understand progress and choose what to study without creating
a second scheduling algorithm, duplicating mutable truth, or sending learning
data to a remote service.

## Decision

### Source of truth

Analytics is calculated from the existing local SQLite records:

- `review_events` provides remembered answers, confidence ratings, response time,
  review state, due transitions, and session identity.
- `practice_sessions` provides session mode, selected decks, start/completion
  times, and accepted session counters.
- current cards and schedules provide active-card, due, suspended, buried, and
  Needs more practice state.
- note tags provide topic scope and overlapping topic attribution.

No analytics rollup table or schema migration is introduced. This avoids stale
caches and keeps review events and schedules authoritative. Aggregates can be
added later only if profiling shows that ordinary indexed queries are too slow on
representative large libraries.

### Scheduling boundary

Analytics does not change FSRS stability, difficulty, state, or due dates. Weak
tags and Needs more practice status explain and prioritize existing due work.
The topic action starts an explicit custom session containing active cards that
are already due; it never silently pulls a not-due card forward.

### Scope semantics

The dashboard supports 30-day, 90-day, one-year, and all-time ranges. Deck
selection is a union. Multiple selected tags are an intersection, matching the
existing Search/Filter contract. A completed session can launch analytics with
its decks preselected and its timeline bucket highlighted.

### Timeline definitions

The mixed chart uses local calendar boundaries:

- 30 days: daily buckets;
- 90 days: seven-day buckets;
- one year: seven-day buckets;
- all time: monthly buckets.

Bars show either unique cards practiced in each bucket or focused study minutes.
The line shows remembered-answer rate. Wrong is a miss; Hard, Medium, and Easy
are remembered answers. A bucket with no reviews has no recall value rather than
a misleading zero percent.

Unique cards are deduplicated within a bucket. Review count and confidence use
all accepted review events. Focused study time is the sum of bounded response
times stored with accepted reviews. Session elapsed time is displayed separately
and is not presented as focused study time.

### Topic classification

A tag is classified only after at least five reviews in the selected range:

- **Building data:** fewer than five reviews.
- **Needs attention:** any active card Needs more practice, or remembered rate is
  at least ten percentage points below desired retention.
- **Strong:** remembered rate is at least three points above desired retention and
  no card Needs more practice.
- **On track:** enough evidence without either condition above.

Trend compares the selected range with the immediately preceding equal-length
range and is withheld unless both periods contain at least five reviews. Reviews
can contribute to more than one tag row because tags overlap. Whole-scope totals
count each review once.

### Visualization library

Chart.js and `react-chartjs-2` are pinned exactly. Chart.js supplies the mixed
bar/line chart, responsive Canvas rendering, tree-shakable registration, and
custom tooltips. The React adapter contains no direct runtime dependencies.
No date adapter is installed because Rust supplies already bucketed category
labels.

The chart reads existing CSS theme variables. Canvas receives an accessible role
and generated description. A semantic table exposes the same values and is
available from the keyboard. Reduce motion disables chart animation.

### Privacy and transfer

All calculations remain on-device. Analytics, schedules, review history, daily
limits, and session history are personal learner state and are not added to
`.studydeck` exports.

## Consequences

- The analytics IPC changes from a no-input global snapshot to a typed scoped
  query; all callers must be updated instead of retaining a compatibility shim.
- Practice uses a lightweight recent-review command rather than loading the full
  dashboard for its recent-deck strip.
- Dashboard figures are reproducible from ordinary stored events and current card
  state.
- Overlapping tag totals are intentionally non-additive and the UI states that
  limitation.
- Study time describes recorded answer-response time, not all time the application
  was open.
