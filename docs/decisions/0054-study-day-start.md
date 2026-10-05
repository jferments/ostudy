# ADR 0054: Study days start at a configurable hour

- **Status:** Accepted
- **Date:** 2026-10-04

## Context

[ADR 0010](0010-study-calendar-historical-attribution-and-projections.md) made the study calendar native: one `StudyTimeContext` per command, local dates instead of fixed 24-hour durations, and review attributions that store the calendar date, zone and UTC offset of the moment each review was saved. Every day boundary was local midnight. Daily limits refilled at midnight ([Stage 5 scheduling controls](0004-stage-5-scheduling-controls.md)), Analytics bucketed by local calendar dates ([Learning analytics](0005-learning-analytics.md)), burial ended at midnight, and Search/Filter's "reviewed today" meant the calendar date.

For a learner who studies past midnight, a session at 00:30 started a fresh day: the limits refilled, Today's counts reset, and that work appeared under the next date. The maintainer decided that a study day starts at 4 a.m. local time by default, and that the hour must be configurable in Settings because not everyone studies that late.

## Decision

### One setting

`newDayStartHour` is an ordinary app setting: a whole hour from 0 (midnight) to 23, default 4. Saving validates it and rejects anything else, including non-integers. It is stored as a key/value row beside the daily limits, so no migration runs. Settings → Review/Scheduling shows it as **New day starts at** with an hour select from 12 a.m. to 11 p.m.

### One owner of day boundaries

`StudyTimeContext` carries the hour. `StudyTimeContext::capture(connection)` reads it from the library's settings, so every command that already captured one context reads the hour explicitly, from the library it works on, instead of from process-wide state. Tests build contexts with an explicit hour.

- The **study date** of an instant is the local date of its wall time minus H hours. The context's `local_date` is the current study date, so between midnight and H it is yesterday's date.
- `today_start_utc_millis` and `tomorrow_start_utc_millis` are local wall time H:00 on the current study date and on the next one. When a daylight-saving change skips H:00 the day starts at the first instant after the gap; when it repeats H:00 the day starts at the first occurrence.
- With H = 0 the context keeps the platform's own first-valid-instant rule, so midnight days behave exactly as before.

### Stored history keeps its meaning

`review_event_attributions.review_local_date` remains the calendar date at review time, as ADR 0010 defines it, whatever the hour. A review's study date is derived when it is read, from its stored instant and offset:

```sql
date(a.reviewed_at / 1000 + a.utc_offset_minutes * 60 - H * 3600, 'unixepoch')
```

Each review therefore keeps the offset of the place where it was made: travelling later does not re-date it. Changing the hour re-buckets all history at query time and rewrites nothing.

Dated windows still select rows through `review_event_attributions_local_date_idx`. Study dates S..E occupy calendar dates S..E+1 when H > 0, so the index range is widened by one date and the stored wall-clock time then decides exactly. That comparison is the integer form of the expression above; dates are formatted only for month buckets, the earliest review and the recent reviews that are shown.

### What follows study days

- Daily new and review limits, in practice selection and in Analytics' Today counts.
- Analytics windows, day, week and month timeline buckets, aggregates, deck and tag summaries, and the date shown for recent reviews. The scope reports the current study date so the custom range picker does not use the device clock.
- Search/Filter's reviewed today, past 7, past 30 and earlier filters. Card rows label "Studied today" from the start of the current study day.
- Burial and **Tomorrow** end at the next study-day start, and **In 3/7 local calendar days** at the start of that study day. A date-only reschedule to date D lands at the start of study day D; choosing the current study date still means now.
- Practice ordering groups due cards by the study date they fell due on, computed from the captured context instead of SQLite's `'localtime'`, which was a second time source.

FSRS due instants stay exact; nothing is rounded to H:00. The stored attribution date stays a calendar date because it is evidence about the moment of review, not a reporting choice.

## Consequences

- Late-night study counts toward the day it belongs to, and the boundary is one setting that can be changed back to midnight at any time.
- Statements in ADRs 0004, 0005 and 0010 about local calendar days, local midnight and local calendar boundaries now refer to study days. The attribution schema and its meaning do not change.
- Due-card ordering reads due instants with the offset in force now. Within an hour of the boundary, a due instant on the other side of a daylight-saving change can sort one study date off. Ordering never feeds a count, so no total is affected.
- Where clocks fall back across H:00 by more than an hour (Antarctica/Troll), a review made in the repeated time before H:00 counts toward the previous study day even though the new day has started. One-hour and half-hour changes never repeat H:00 in the middle of the repeated time.
- An older build ignores the setting and keeps midnight days for the same library.

## Verification

`src-tauri/src/study_day_tests.rs` seeds reviews around midnight, 4 a.m., both 2026 Pacific daylight-saving changes and one review saved in Tokyo, read in a synthetic Pacific zone:

- with H = 0, analytics, daily limits and search equal the output captured before study days existed (`src-tauri/tests/fixtures/study-day/midnight-boundary-golden.txt`);
- for H = 0, 1, 4, 12 and 23, every consumer agrees with an oracle that dates each review with SQLite's own expression above;
- named 4 a.m. scenarios cover Today, timeline buckets, limits, search, burial, rescheduling, daylight-saving days and travel.

`study_calendar` unit tests cover skipped and repeated start times and check that every study day of 2026 begins where the previous one ends for all 24 hours. The query-plan tests run with H = 0 and H = 4 and still require dated history to start from the local-date index.
