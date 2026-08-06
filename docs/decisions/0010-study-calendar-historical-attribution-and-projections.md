# ADR 0010: Study calendar, card-owned history, and bounded projections

- **Status:** Accepted
- **Date:** 2026-08-06
- **Scope:** Stages 3.6, 3.7, and 3.8

## Context

Scheduling, workload, Search/Filter, practice, and Analytics previously reproduced overlapping definitions of “due,” “available,” and “studied recently.” Some paths interpreted local dates as fixed 24-hour durations, some hydrated complete card graphs for list screens, and historical Analytics depended too heavily on mutable current rows.

These problems interact. A bounded projection is only correct when it uses the same studyability and calendar rules as the command it replaces, and historical aggregation is only stable when review-time facts are stored independently from mutable presentation data.

## Decision

### One study-time context per command

Every command that evaluates studyability, a local date, burial, workload, rescheduling, or Analytics captures one `StudyTimeContext` at entry and passes it through subordinate calculations.

The context contains:

- one UTC timestamp;
- the system time-zone identifier when it can be determined;
- the captured UTC offset;
- the current local date;
- the first valid instant of today and tomorrow.

Local-calendar operations add calendar dates and then resolve the first valid local instant. They do not add fixed 24-hour durations. FSRS due timestamps remain exact instants and are not rounded to local midnight.

### Canonical card-state predicates

The native `studyability` module owns the definitions of:

- current generated card;
- studyable card;
- schedule-due card;
- studyable-due card.

A studyable card belongs to a non-deleted, non-archived deck; has non-deleted note and card rows; is not manually suspended; is not archive-suspended; and is not buried past the captured command time. A studyable-due card additionally has `due_at <= now`.

Deck summaries, workload, scheduled selection, Search/Filter, Analytics current inventory, and tag-scoped practice use this same predicate family.

### Archive suspension is separate from manual suspension

Archiving a deck sets `decks.archived_at` and `cards.archive_suspended_at`. It does not overwrite `cards.suspended_at`. Restoring a deck clears only `archive_suspended_at`, so a card that was manually suspended before archival remains manually suspended afterward.

Archived decks are excluded from active inventory and study selection. Their review history remains visible in historical Analytics.

### Review history belongs to the card

The stable generated card ID is the owner of scheduling and review history.

Moving a source note and its generated cards to another deck transfers current ownership without rewriting or deleting review events. Historical deck aggregation joins each review through its stable card to the card’s current deck. Therefore:

- moving a card from Deck A to Deck B moves all of its historical deck attribution to B;
- renaming B changes the displayed historical deck name;
- deleting A does not affect the moved card or its review history;
- permanently deleting B later purges the card and all history owned by it.

Captured deck names remain audit facts about the original review session. They are not the normal grouping key.

### Immutable review-time presentation and tag facts

Schema 14 adds snapshot and attribution tables for:

- practice-session card presentation;
- practice-session card tags;
- review-event presentation and calendar attribution;
- review-event tags.

A review attribution records the review instant, durable local study date, time-zone metadata, stable card and note IDs, note revision, captured owner/name, direction, card type and label, presentation variant, bounded question excerpt, full-question SHA-256 hash, truncation flag, and attribution source.

Editing card content, retagging, changing cloze rotation, or changing Reverse configuration cannot alter these captured review-time facts. Current tag names may be used for display when the same stable tag still exists; captured names remain available as fallback evidence.

Legacy reviews are repaired exactly once after schema 14 and are marked `legacy_current_state_backfill`. The repair does not claim to reconstruct unavailable historical presentation perfectly.

### Permanent deletion is an explicit exception to append-only history

Review events are append-only during normal study and authoring operations. A finalized destructive deletion is the explicit exception.

After the Undo window expires, the purge process:

1. resolves the cards still owned by the deleted deck at purge time;
2. removes their review events and attribution rows;
3. removes their scheduling events and session membership/snapshots;
4. recomputes or removes affected sessions;
5. removes the owned notes, cards, schedules, and deck;
6. removes the completed deletion operation.

Resolving current ownership at purge time is what protects cards moved out of a deleted deck during the Undo period.

### Summary, detail, aggregate, and window projections are separate contracts

List screens no longer use one full graph DTO.

- Decks use grouped deck summaries.
- A deck’s cards use bounded source-note summary pages.
- Editing loads one full organized-card detail.
- Templates use summary rows and load one complete template graph on demand.
- Practice creation validates the full selected ID plan in batches, stores durable ordering and snapshots, and returns only the first bounded card window.
- Analytics uses fixed aggregate/timeline/recent projections rather than per-card hydration.

The removed unbounded `list_cards` command is not retained as a compatibility shim. The active deck-list projection command is `list_deck_card_summaries`.

### Mutation validation remains all-or-nothing

Chunking bounds SQL statement size; it does not split a logical mutation into independently committed chunks. Multi-card scheduling, practice creation, deletion, and transfer validate the complete plan before committing one transaction.

## Consequences

### Positive

- All study surfaces agree at due, burial, suspension, archive, and deletion boundaries.
- Daily accounting remains stable across daylight-saving transitions and later travel.
- Card moves and renames have predictable Analytics behavior.
- Historical presentation and tag evidence cannot drift after edits.
- Deleted card-owned history is actually removed.
- Large libraries no longer require unbounded list hydration or N+1 card reads.
- Frontend screens can discard stale asynchronous responses using request identities.

### Costs

- Schema 14 and repair version 3 add durable attribution data.
- Historical aggregation joins immutable event facts with current card ownership.
- List and detail DTOs must evolve independently.
- Query-count and migration fixtures are required to prevent accidental regression.

## Verification

The required qualification sequence is:

```bash
./scripts/check-fast.sh
./scripts/check.sh
./run-app.sh
```

Manual acceptance must cover archive/unarchive suspension preservation, DST/local-date rescheduling, moved-card ownership, deck/tag renames, deletion after Undo, immutable recent-review presentation, scoped practice, bounded card/template pages, and Analytics filters/ranges.
