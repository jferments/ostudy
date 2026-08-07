# Scheduling, Analytics, and Query Testing

This guide covers the Stage 3.6–3.8 invariants that must be preserved when changing studyability, calendar behavior, historical attribution, or list hydration.

## Required automated gates

Run the fast gate while iterating:

```bash
./scripts/check-fast.sh
```

Before merge, run the complete gate:

```bash
./scripts/check.sh
```

The complete gate must pass frontend tests and type checking, Rust formatting and Clippy, Rust tests, the production frontend build, the native Tauri build, schema/migration verification, and native smoke coverage.

Do not report a branch as qualified merely because an earlier commit passed. The exact proposed head must pass.

## Canonical studyability matrix

Construct cards that cover every boundary below and verify identical results in Decks, Practice workload, scheduled practice selection, Search/Filter, Analytics current inventory, and tag-scoped practice.

| State | Current inventory | Studyable | Studyable due |
| --- | ---: | ---: | ---: |
| Active, due before now | yes | yes | yes |
| Active, due exactly now | yes | yes | yes |
| Active, due after now | yes | yes | no |
| Manually suspended | yes | no | no |
| Archive-suspended | no | no | no |
| Buried until before now | yes | yes | depends on due |
| Buried until exactly now | yes | yes | depends on due |
| Buried until after now | yes | no | no |
| Soft-deleted card, note, or deck | no | no | no |

The command must capture time once. Do not call the clock separately while selecting, validating, and assembling the response.

## Local-calendar tests

Calendar tests assert local dates and local boundaries, not elapsed multiples of 24 hours.

Cover:

- spring daylight-saving transition;
- autumn daylight-saving transition;
- local midnight;
- month and year boundaries;
- leap-day one-year windows;
- a custom reschedule date equal to today;
- rejection of past dates;
- retry of the same reschedule mutation after a date boundary.

FSRS due instants remain exact. Only calendar operations such as “tomorrow,” daily limits, burial expiry, and Analytics windows use local-date resolution.

## Historical attribution tests

A review test fixture should contain:

- one stable card ID;
- review event and attribution row;
- captured question excerpt and hash;
- captured deck name;
- captured tags;
- durable local review date and time-zone metadata.

Then independently mutate current state:

- edit the note;
- move it to another deck;
- rename the destination deck;
- retag it;
- rename a retained tag;
- alter Reverse or cloze presentation state;
- archive and restore its deck.

Expected behavior:

- question excerpt, card label, presentation variant, and captured tag membership remain immutable;
- historical deck aggregation follows the card’s current owner;
- the current deck name is displayed after rename;
- current tag names are displayed for retained stable tag IDs;
- archived-deck history remains visible.

Legacy repaired rows must be visibly distinguishable through `attribution_source = legacy_current_state_backfill`.

## Ownership and deletion tests

The critical regression scenario is:

1. Review a card in Deck A.
2. Move its note and all generated cards to Deck B.
3. Rename Deck B.
4. Delete Deck A and allow Undo to expire.
5. Verify the card and all of its history remain under B.
6. Delete Deck B and allow Undo to expire.
7. Verify the card, schedules, review events, attribution, session membership, and Analytics rows are gone.

Deletion cleanup must resolve the cards currently owned by the deleted deck at purge time. It must not trust stale card IDs captured before a move.

Mixed sessions require counter recomputation after some cards are purged. Empty sessions should be removed.

## Bounded projection tests

### Deck cards

- reject page sizes above the native maximum;
- verify returned rows never exceed the requested page size;
- verify `totalCount`, `offset`, and `hasMore`;
- verify summaries do not contain full rich documents or asset graphs;
- load one full card detail only when editing.

### Templates

- list summaries without field/rule/asset graphs;
- load one complete template detail on selection;
- discard a late response for a previously selected template.

### Practice

- create a session larger than one card window;
- verify only the first window is returned at creation;
- fetch later windows by position;
- preserve durable database ordering;
- prevent stale prefetch responses from reviving an ended/replaced session;
- retry the same operation ID without creating a second session.

### Analytics

- query the 7-day, 30-day, 90-day, one-year, and all-time ranges;
- verify buckets use durable local review dates;
- verify current inventory and historical evidence remain separate;
- verify deck/tag filters do not change captured presentation facts;
- verify an old request cannot overwrite a newer filter/range response.

## SQL statement-count discipline

Query-count tests should measure SQL statement count, not wall-clock duration. Timing thresholds are nondeterministic across developer machines and CI runners.

When SQLite tracing is enabled for a test, count application statements while excluding transaction-control and schema-inspection noise. Assert a fixed upper bound as the number of cards grows.

Expected properties:

- deck summary count is constant with deck size;
- a deck-card page uses a bounded fixed number of statements;
- practice validation grows by ID chunk count, not by card count;
- asset and tag hydration is batched;
- Analytics uses a fixed group of aggregate queries, not one query per card/tag/deck;
- template summary listing does not hydrate each template graph.

For performance-sensitive SQL, also inspect plans:

```sql
EXPLAIN QUERY PLAN <query>;
```

Confirm the Stage 3.8 indexes are selected for studyability, review-local-date, card-review chronology, deck paging, and tag attribution paths. An index existing in the schema is insufficient if the planner does not use it for the tested predicate/order.

## Backup and transfer boundaries

A full library backup must preserve review attribution and reproduce Analytics after restore.

A shareable `.studydeck` package must not contain another learner’s review events, session snapshots, attribution rows, or schedules. Imported cards receive fresh local schedules.

## Manual application smoke test

After automated gates pass:

```bash
./run-app.sh
```

Verify:

- opening and paging a large deck;
- archive and restore;
- manual reschedule presets and custom local date;
- deck/tag scoped practice;
- moving reviewed cards between decks;
- Analytics rename and deletion behavior;
- complete backup/restore;
- `.studydeck` export/import isolation.
