# Scheduling, Analytics, and projection testing

Stages 3.6–3.8 use one local-calendar context and separate current inventory from historical review evidence.

## Invariants

- **Studyable due** means the card, note, and deck are current; the deck is not archived; the card is not suspended; burial has expired; and `due_at <= now`.
- FSRS due values remain exact instants. Calendar dates are used only for burial, date-only rescheduling, daily limits, recency, and Analytics buckets.
- Archived decks have zero current studyable inventory but retain historical Analytics.
- Deck and tag IDs remain historical dimensions; retained rows provide their current names.
- Soft-deleted decks are immediately excluded from every Analytics query. When Undo expires, their review events, attribution rows, and practice-session card rows are removed.
- List commands return bounded summaries. Full documents/assets are loaded only for an opened card/template or a bounded practice window.

## Fast gate

```bash
bash ./scripts/check-fast.sh
```

This checks the source architecture, React behavior, strict TypeScript, Rust formatting, and all-target compilation.

## Full gate

```bash
./scripts/check.sh
```

The full gate adds Clippy with warnings denied, every Rust/migration/backup test, the standalone Tauri build, dependency verification, and native restart persistence.

## Isolated local profile

Schema 14 is forward-only. Exercise migration and manual behavior in a disposable profile before opening a regular library:

```bash
TEST_ROOT="$(mktemp -d)"
mkdir -p "$TEST_ROOT"/{data,config,cache,state}

XDG_DATA_HOME="$TEST_ROOT/data" \
XDG_CONFIG_HOME="$TEST_ROOT/config" \
XDG_CACHE_HOME="$TEST_ROOT/cache" \
XDG_STATE_HOME="$TEST_ROOT/state" \
./run-app.sh
```

Keep a copy of the normal application-data directory shown under **Settings → Storage** until the pull request has been accepted.

## Manual acceptance

1. A buried or suspended overdue card is not counted as Ready now in Decks, Search/Filter, Practice, or Analytics.
2. Bury until tomorrow becomes studyable at the next local date boundary.
3. Reschedule presets and custom dates resolve in Rust; selecting today means due now.
4. Archive a deck. Every previously active card becomes suspended, it leaves current workload, and its historical Analytics remains.
5. Unarchive it. Cards suspended by archive resume; cards manually suspended beforehand stay suspended.
6. Rename the archived deck and a historical tag. Analytics displays the new names without moving review counts.
7. Delete a reviewed deck. It disappears from Analytics immediately. Let Undo expire, restart, and confirm its review/session history is gone.
8. Search a large deck and confirm only one bounded page is transferred. Open a card to load its full rich document.
9. Start a session with more than 50 cards and proceed past the first window.
10. Back up and restore the library; attribution and archive state remain. Exporting `.studydeck` still excludes learner history.
