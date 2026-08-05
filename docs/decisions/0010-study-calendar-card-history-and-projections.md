# ADR 0010: Study calendar, card-owned history, and bounded projections

- **Status:** Accepted
- **Date:** 2026-08-05
- **Scope:** Stages 3.6–3.8

## Context

The application previously reconstructed “due,” local days, weak-learning signals, and historical analytics independently in several commands. Those definitions diverged. Large list paths also hydrated full editable card/template graphs or issued SQL once per returned row.

Review history belongs to the generated card. A card move therefore transfers its complete learning history to its destination deck. A deck rename changes the label shown for all cards it currently owns. Archiving removes cards from current studyability but preserves their history. Deleting a deck removes the cards it still owns and all review/session history for those cards; a card moved out before deletion is unaffected.

## Decision

### Captured command time

Every scheduling, workload, practice, and analytics command captures one `StudyTimeContext` containing:

- the current UTC instant;
- the best available IANA time-zone identifier, or the explicit `unknown` fallback;
- the captured UTC offset;
- the current local date;
- the first valid instant of today and tomorrow.

Calendar-day behavior uses local dates and first-valid local instants. It never assumes that a local day is 86,400,000 milliseconds. FSRS due timestamps remain exact instants and are never rounded to calendar boundaries.

### Studyability vocabulary

- **Current generated card:** card, note, and deck are not deleted, and the deck is not archived.
- **Studyable card:** current generated card, not suspended, and not buried past the captured instant.
- **Schedule due:** schedule timestamp is at or before the captured instant.
- **Studyable due:** studyable and schedule due. This is the user-facing “ready now” concept.
- **Scheduled today:** selected studyable-due cards after daily limits and backlog policy.
- **Extra available:** studyable card available through an explicit extra/custom session even when scheduled later.
- **Needs more practice:** current learning-epoch signal; it does not imply due status.

All commands consume the same SQL/pure-Rust studyability contract.

### Card-owned history and presentation evidence

`review_events.card_id` is the authoritative ownership key. Current deck and tag labels used by aggregate analytics are resolved through the card/note’s current organization. Moving a card transfers its history. Renaming a deck or tag changes its current aggregate label.

Separate immutable attribution rows preserve event-time evidence needed to explain a review:

- presentation variant and card label;
- bounded rendered-question excerpt and full-text SHA-256 hash;
- note revision;
- event-time deck/tag audit labels;
- local review date, time-zone identifier, and captured offset;
- whether attribution was captured or reconstructed during upgrade.

Accepted reviews and their attribution rows commit atomically. Legacy review attribution is an explicitly marked, checksummed, one-time repair. Captured deck/tag values are audit evidence, not aggregate ownership.

### Lifecycle

- **Move:** change card/note ownership; all review history follows the card.
- **Rename:** use current entity names in aggregate analytics.
- **Archive:** suspend the deck’s current cards while preserving prior manual suspension state; retain history.
- **Delete:** permanently remove currently owned cards and all their review/session history. Moved-out cards remain.

### Projection classes

Read commands are classified as:

1. summary projections for lists/selectors;
2. fixed-query detail projections for one editor item;
3. SQL aggregate projections for workload/analytics;
4. validated page/window projections for potentially large ordered lists.

SQL statement count must be fixed or grow only by documented 250-ID chunks, never by one statement per returned card, note, asset, tag, deck, or template.

## Consequences

- Manual rescheduling sends canonical date intent; Rust stores both intent and resolved due instant for retry safety.
- Daily limits and analytics timelines use durable `review_local_date` values.
- Practice sessions persist complete session order but return small card windows.
- Deck-card, Search/Filter, template, review, and session lists are bounded.
- Replaced commands and ambiguous DTO fields are removed after callers migrate.
- Full-library backups preserve attribution tables. Shareable `.studydeck` packages continue excluding schedules, sessions, reviews, and learner attribution.
- Once schema and repair ledgers are complete, ordinary startup writes no application data.
