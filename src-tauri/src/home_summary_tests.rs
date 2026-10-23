//! The Home summary's counts, its daily-limit arithmetic, the study day it
//! counts by, the time estimate, and the plans of every statement that reads
//! review history.
//!
//! The library is read at a fixed time in the synthetic 2026 Pacific zone, so
//! no result depends on when or where the tests run.

use std::sync::{Mutex, MutexGuard, PoisonError};

use rusqlite::trace::{TraceEvent, TraceEventCodes};

use super::*;
use crate::{collections::test_support::seed_document, database::Database};

const MINUTE: i64 = 60_000;
/// Noon on Wednesday 10 June 2026, Pacific daylight time. Every review the
/// tests make falls in daylight time too, so each carries the clock's offset.
const NOW: &str = "2026-06-10T12:00:00-07:00";
const RICH_TEXT: &str =
    r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"q"}]}]}"#;

fn millis(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value)
        .expect("test time should parse")
        .timestamp_millis()
}

/// The tests' clock at `now`, with the study day starting at `hour`.
fn study_time(now: &str, hour: u32) -> StudyTimeContext {
    StudyTimeContext::pacific_2026_at(millis(now), hour).expect("study time should resolve")
}

/// Builds a current-schema library row by row.
struct Library<'a> {
    connection: &'a Connection,
    time: StudyTimeContext,
    next: u32,
    documents: u8,
}

impl<'a> Library<'a> {
    /// A library read at [`NOW`], with the study day starting at 4 a.m.
    fn new(connection: &'a Connection) -> Self {
        Self::at(
            connection,
            study_time(NOW, crate::study_calendar::DEFAULT_NEW_DAY_START_HOUR),
        )
    }

    fn at(connection: &'a Connection, time: StudyTimeContext) -> Self {
        Self {
            connection,
            time,
            next: 0,
            documents: 0,
        }
    }

    fn now(&self) -> i64 {
        self.time.now_utc_millis
    }

    fn id(&mut self, kind: u8) -> String {
        self.next += 1;
        format!("{kind:02x}{:030x}", self.next)
    }

    fn exec(&self, sql: &str, values: impl rusqlite::Params) {
        self.connection
            .execute(sql, values)
            .unwrap_or_else(|error| panic!("{error}: {sql}"));
    }

    fn subject(&mut self, name: &str, sort_order: i64) -> String {
        let id = self.id(0xc0);
        self.exec(
            "INSERT INTO document_collections(id, name, sort_order, created_at, updated_at,
                                              revision)
             VALUES (?1, ?2, ?3, 1, 1, 1)",
            params![id, name, sort_order],
        );
        id
    }

    fn deck(&mut self, name: &str, subject: Option<&str>) -> String {
        let id = self.id(0xde);
        self.exec(
            "INSERT INTO decks(id, name, created_at, updated_at, subject_id)
             VALUES (?1, ?2, 1, 1, ?3)",
            params![id, name, subject],
        );
        id
    }

    fn document(&mut self, subject: Option<&str>) -> String {
        let id = self.id(0xd0);
        let hash = char::from(b'a' + self.documents);
        self.documents += 1;
        seed_document(self.connection, &id, 1, hash).unwrap();
        self.exec(
            "UPDATE documents SET subject_id = ?2 WHERE id = ?1",
            params![id, subject],
        );
        id
    }

    fn note(&mut self, deck: &str) -> String {
        let id = self.id(0x40);
        self.exec(
            "INSERT INTO notes(id, deck_id, front_doc, back_doc, front_text, back_text,
                               created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3, 'q', 'a', 1, 1)",
            params![id, deck, RICH_TEXT],
        );
        id
    }

    /// A card of `note` with a schedule in `state` (0 is new), due at `due_at`.
    fn card_of(&mut self, note: &str, deck: &str, state: i64, due_at: i64) -> String {
        let id = self.id(0xca);
        self.exec(
            "INSERT INTO cards(id, note_id, deck_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, 1)",
            params![id, note, deck],
        );
        self.exec(
            "INSERT INTO card_schedules(card_id, state, due_at, last_review_at, scheduler_name,
                                        scheduler_version, updated_at)
             VALUES (?1, ?2, ?3, 1, 'fsrs', 'rs-fsrs-1.2.1', 1)",
            params![id, state, due_at],
        );
        id
    }

    /// A one-card note.
    fn card(&mut self, deck: &str, state: i64, due_at: i64) -> String {
        let note = self.note(deck);
        self.card_of(&note, deck, state, due_at)
    }

    fn due_review(&mut self, deck: &str) -> String {
        let due = self.now() - MINUTE;
        self.card(deck, 2, due)
    }

    fn due_new(&mut self, deck: &str) -> String {
        let due = self.now() - MINUTE;
        self.card(deck, 0, due)
    }

    /// A review of `card` at `at`, with the active time the practice screen
    /// measured. Like a saved review, its attribution keeps the local calendar
    /// date, zone and offset of the moment it was made.
    fn review(&mut self, card: &str, at: i64, state_before: i64, active_time_ms: Option<i64>) {
        let date = self.time.calendar_date_string_for_timestamp(at).unwrap();
        let id = self.id(0xee);
        let submission = self.id(0x5b);
        let (note, deck): (String, String) = self
            .connection
            .query_row(
                "SELECT note_id, deck_id FROM cards WHERE id = ?1",
                [card],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        self.exec(
            "INSERT INTO review_events(id, submission_id, card_id, reviewed_at, rating,
                 answer_correct, confidence, response_time_ms, state_before, state_after,
                 due_before, due_after, stability_before, stability_after, difficulty_before,
                 difficulty_after, scheduled_days_before, scheduled_days_after,
                 scheduler_name, scheduler_version, created_at)
             VALUES (?1, ?2, ?3, ?4, 3, 1, 'somewhat_confident', 1000, ?5, 2, ?4, ?4,
                     0.0, 3.0, 0.0, 5.0, 0, 3, 'fsrs', 'rs-fsrs-1.2.1', ?4)",
            params![id, submission, card, at, state_before],
        );
        self.exec(
            "INSERT INTO review_event_attributions(review_event_id, reviewed_at,
                 review_local_date, time_zone_id, utc_offset_minutes, card_id, note_id,
                 note_revision, owner_deck_id_at_review, captured_deck_name, direction,
                 card_type, card_label, presentation_variant, question_excerpt,
                 question_text_hash, question_was_truncated, attribution_source)
             VALUES (?1, ?2, ?3, ?8, ?9, ?4, ?5, 1, ?6, 'Deck', 'forward', 'basic',
                     'Basic', 'forward', 'q', ?7, 0, 'captured')",
            params![
                id,
                at,
                date,
                card,
                note,
                deck,
                "0".repeat(64),
                self.time.time_zone_id,
                self.time.utc_offset_minutes
            ],
        );
        self.exec(
            "INSERT INTO review_event_scheduler_context(review_event_id, elapsed_days,
                 reps_before, lapses_before, desired_retention, scheduler_parameters_sha256,
                 active_time_ms)
             VALUES (?1, 0, 0, 0, 0.9, ?2, ?3)",
            params![id, "a".repeat(64), active_time_ms],
        );
    }

    /// A timed review made `ago` before now.
    fn timed_review(&mut self, card: &str, ago: i64, active_time_ms: Option<i64>) {
        let at = self.now() - ago;
        self.review(card, at, 2, active_time_ms);
    }

    fn overview_guide(&mut self, subject: &str, state: &str) {
        let target = self.id(0x7e);
        let guide = self.id(0x9d);
        self.exec(
            "INSERT INTO study_targets(id, target_kind, collection_id, created_at, updated_at)
             VALUES (?1, 'collection', ?2, 1, 1)",
            params![target, subject],
        );
        self.exec(
            "INSERT INTO study_target_guides(id, target_id, revision, source_snapshot_id,
                 content_json, content_hash, state, created_at, updated_at, published_at)
             VALUES (?1, ?2, 1, ?3, '{}', ?3, ?4, 1, 1, 1)",
            params![guide, target, "b".repeat(64), state],
        );
    }

    fn set_daily_limits(&self, new_cards: i64, reviews: i64) {
        for (key, value) in [("dailyNewLimit", new_cards), ("dailyReviewLimit", reviews)] {
            self.exec(
                "INSERT INTO app_settings(key, value_json, updated_at) VALUES (?1, ?2, 1)
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                params![key, value.to_string()],
            );
        }
    }
}

fn summary(connection: &mut Connection, time: &StudyTimeContext) -> HomeSummaryDto {
    load_home_summary(connection, time).expect("the summary should load")
}

fn entry<'a>(summary: &'a HomeSummaryDto, subject: &str) -> &'a HomeSubjectSummaryDto {
    summary
        .subjects
        .iter()
        .find(|entry| entry.subject_id.as_deref() == Some(subject))
        .unwrap_or_else(|| panic!("no summary for {subject}: {summary:#?}"))
}

fn sorted(ids: &[&String]) -> Vec<String> {
    let mut ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
    ids.sort();
    ids
}

#[test]
fn counts_current_and_due_cards_per_subject_and_under_no_subject() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library::new(connection);
            let now = library.now();
            let biology = library.subject("Biology", 1000);
            let chemistry = library.subject("Chemistry", 2000);
            let history = library.subject("History", 3000);

            // Biology: two due reviews, a new card, a review due tomorrow, a
            // suspended and a buried due card, a deleted card and a card of a
            // deleted note.
            let cells = library.deck("Cells", Some(&biology));
            library.due_review(&cells);
            library.due_review(&cells);
            library.due_new(&cells);
            library.card(&cells, 2, now + 86_400_000);
            let suspended = library.due_review(&cells);
            library.exec(
                "UPDATE cards SET suspended_at = 1 WHERE id = ?1",
                [&suspended],
            );
            let buried = library.due_review(&cells);
            library.exec(
                "UPDATE cards SET buried_until = ?2 WHERE id = ?1",
                params![buried, now + MINUTE],
            );
            let deleted = library.due_review(&cells);
            library.exec("UPDATE cards SET deleted_at = 1 WHERE id = ?1", [&deleted]);
            let deleted_note = library.note(&cells);
            library.card_of(&deleted_note, &cells, 2, now - MINUTE);
            library.exec(
                "UPDATE notes SET deleted_at = 1 WHERE id = ?1",
                [&deleted_note],
            );

            // A collection copy merged into its original, as release
            // migration 6 leaves it: the copy is retired (deleted) and moved
            // into the original's deck, and its schedule may still be due.
            let original_note = library.note(&cells);
            let original = library.card_of(&original_note, &cells, 2, now - MINUTE);
            let copy_note = library.note(&cells);
            let copy = library.card_of(&copy_note, &cells, 2, now - MINUTE);
            library.exec("UPDATE cards SET deleted_at = 1 WHERE id = ?1", [&copy]);
            library.exec(
                "UPDATE notes SET deleted_at = 1 WHERE id = ?1",
                [&copy_note],
            );
            library.exec(
                "INSERT INTO card_merges(retired_card_id, surviving_card_id, retired_note_id,
                                         surviving_note_id, reason, copied_at)
                 VALUES (?1, ?2, ?3, ?4, 'collection_copy', 1)",
                params![copy, original, copy_note, original_note],
            );

            // Archived and deleted decks count for nothing, due cards included.
            let archived = library.deck("Archived", Some(&biology));
            library.due_review(&archived);
            library.exec(
                "UPDATE decks SET archived_at = 1 WHERE id = ?1",
                [&archived],
            );
            let gone = library.deck("Gone", Some(&biology));
            library.due_review(&gone);
            library.exec("UPDATE decks SET deleted_at = 1 WHERE id = ?1", [&gone]);
            library.document(Some(&biology));
            library.overview_guide(&biology, "active");

            // Chemistry: a due new card in one deck, an empty second deck, two
            // documents and only a superseded overview guide.
            let acids = library.deck("Acids", Some(&chemistry));
            library.due_new(&acids);
            library.deck("Bases", Some(&chemistry));
            library.document(Some(&chemistry));
            library.document(Some(&chemistry));
            library.overview_guide(&chemistry, "superseded");

            // No subject: one deck with a due review and one document.
            let loose = library.deck("Loose", None);
            library.due_review(&loose);
            library.document(None);
            let time = library.time.clone();

            let summary = summary(connection, &time);
            assert_eq!(summary.study_date, time.local_date.to_string());
            assert_eq!(
                summary
                    .subjects
                    .iter()
                    .map(|entry| entry.subject_id.clone().unwrap())
                    .collect::<Vec<_>>(),
                vec![biology.clone(), chemistry.clone(), history.clone()],
                "subjects keep the learner's order"
            );

            let bio = entry(&summary, &biology);
            // Two due reviews, the new card, the future card, the suspended
            // and buried cards, and the original that absorbed its copy.
            assert_eq!(bio.card_count, 7);
            assert_eq!(bio.due_reviews, 3);
            assert_eq!(bio.due_new, 1);
            assert_eq!((bio.scheduled_reviews, bio.scheduled_new), (3, 1));
            assert_eq!(bio.deck_count, 1);
            assert_eq!(bio.source_count, 1);
            assert!(bio.has_overview_guide);
            assert_eq!(bio.due_deck_ids, vec![cells.clone()]);

            let chem = entry(&summary, &chemistry);
            assert_eq!(chem.card_count, 1);
            assert_eq!((chem.due_reviews, chem.due_new), (0, 1));
            assert_eq!(chem.deck_count, 2);
            assert_eq!(chem.source_count, 2);
            assert!(!chem.has_overview_guide);
            assert_eq!(chem.due_deck_ids, vec![acids.clone()]);

            assert_eq!(
                entry(&summary, &history),
                &HomeSubjectSummaryDto {
                    subject_id: Some(history.clone()),
                    ..HomeSubjectSummaryDto::default()
                }
            );

            let unfiled = &summary.no_subject;
            assert_eq!(unfiled.subject_id, None);
            assert_eq!(unfiled.card_count, 1);
            assert_eq!((unfiled.due_reviews, unfiled.due_new), (1, 0));
            assert_eq!(unfiled.deck_count, 1);
            assert_eq!(unfiled.source_count, 1);
            assert!(!unfiled.has_overview_guide);
            assert_eq!(unfiled.due_deck_ids, vec![loose.clone()]);

            let today = &summary.today;
            assert_eq!((today.due_reviews, today.due_new), (4, 2));
            assert_eq!((today.scheduled_reviews, today.scheduled_new), (4, 2));
            assert_eq!((today.reviews_remaining, today.new_remaining), (200, 20));
            assert_eq!(today.subject_count, 3, "Biology, Chemistry and No subject");
            assert_eq!(today.deck_ids, sorted(&[&cells, &acids, &loose]));
            // Six cards at the default twelve seconds: 72 seconds, about 1 minute.
            assert_eq!(today.estimate_ms_per_card, DEFAULT_MS_PER_CARD);
            assert_eq!(today.estimate_review_count, 0);
            assert_eq!(today.estimated_minutes, 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn an_empty_library_has_nothing_due_and_no_estimate() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let time = study_time(NOW, 4);
            let summary = summary(connection, &time);
            assert!(summary.subjects.is_empty());
            assert_eq!(summary.no_subject, empty_group(None));
            assert_eq!(
                summary.today.scheduled_reviews + summary.today.scheduled_new,
                0
            );
            assert_eq!(summary.today.subject_count, 0);
            assert!(summary.today.deck_ids.is_empty());
            assert_eq!(summary.today.estimated_minutes, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn rows_filed_in_a_missing_subject_count_under_no_subject() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library::new(connection);
            let stray = library.deck("Stray", None);
            library.due_review(&stray);
            let document = library.document(None);
            library
                .connection
                .execute_batch("PRAGMA foreign_keys = OFF")
                .unwrap();
            let missing = "f".repeat(32);
            library.exec(
                "UPDATE decks SET subject_id = ?2 WHERE id = ?1",
                params![stray, missing],
            );
            library.exec(
                "UPDATE documents SET subject_id = ?2 WHERE id = ?1",
                params![document, missing],
            );
            let time = library.time.clone();

            let summary = summary(connection, &time);
            assert!(summary.subjects.is_empty());
            assert_eq!(summary.no_subject.deck_count, 1);
            assert_eq!(summary.no_subject.source_count, 1);
            assert_eq!(summary.no_subject.due_reviews, 1);
            assert_eq!(summary.no_subject.due_deck_ids, vec![stray]);
            Ok(())
        })
        .unwrap();
}

#[test]
fn todays_reviews_use_up_the_daily_limits_start_review_applies() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library::new(connection);
            library.set_daily_limits(2, 4);
            let languages = library.subject("Languages", 1000);
            let music = library.subject("Music", 2000);

            // Languages: two due reviews and three new cards.
            let verbs = library.deck("Verbs", Some(&languages));
            for _ in 0..2 {
                library.due_review(&verbs);
            }
            for _ in 0..3 {
                library.due_new(&verbs);
            }
            // Music: one due review.
            let scales = library.deck("Scales", Some(&music));
            library.due_review(&scales);

            // Today one review and one new card were studied; yesterday's
            // reviews do not count against today's limits.
            let studied = library.card(&scales, 2, library.now() + 86_400_000);
            let at = library.now() - MINUTE;
            library.review(&studied, at, 2, Some(9_000));
            library.review(&studied, at, 0, Some(9_000));
            for _ in 0..3 {
                library.review(&studied, at - DAY_MS, 2, None);
            }
            let time = library.time.clone();

            let summary = summary(connection, &time);
            let today = &summary.today;
            assert_eq!((today.reviews_remaining, today.new_remaining), (3, 1));
            // Every due review fits, so the remaining new card joins them.
            assert_eq!((today.due_reviews, today.due_new), (3, 3));
            assert_eq!((today.scheduled_reviews, today.scheduled_new), (3, 1));
            assert_eq!(today.subject_count, 2);
            // Each subject's own Start applies the same remaining limits.
            let languages = entry(&summary, &languages);
            assert_eq!(
                (languages.scheduled_reviews, languages.scheduled_new),
                (2, 1)
            );
            let music = entry(&summary, &music);
            assert_eq!((music.scheduled_reviews, music.scheduled_new), (1, 0));
            // The card studied today is not due, so its deck is due for the
            // Scales review only and still listed once.
            assert_eq!(today.deck_ids, sorted(&[&verbs, &scales]));
            Ok(())
        })
        .unwrap();
}

#[test]
fn due_reviews_beyond_the_review_limit_keep_new_cards_out_of_start_review() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library::new(connection);
            library.set_daily_limits(5, 2);
            let reading = library.subject("Reading", 1000);
            let writing = library.subject("Writing", 2000);
            let novels = library.deck("Novels", Some(&reading));
            for _ in 0..3 {
                library.due_review(&novels);
            }
            let essays = library.deck("Essays", Some(&writing));
            library.due_new(&essays);
            let time = library.time.clone();

            let summary = summary(connection, &time);
            let today = &summary.today;
            assert_eq!((today.scheduled_reviews, today.scheduled_new), (2, 0));
            // Writing's new card fits its own session, but not Start review.
            assert_eq!(today.subject_count, 1);
            let writing = entry(&summary, &writing);
            assert_eq!((writing.scheduled_reviews, writing.scheduled_new), (0, 1));
            Ok(())
        })
        .unwrap();
}

/// Home, the Review badge and Start review count by the study day, which
/// starts at the learner's "New day starts at" hour: reviews made after
/// midnight count toward the day before until that hour, when the daily
/// limits start over. What Home counts is what the scheduled session over
/// Start review's decks plans, because both read the same workload and study
/// calendar.
#[test]
fn todays_counts_follow_the_study_day_and_match_the_session_start_review_plans() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library::at(connection, study_time("2026-06-10T03:00:00-07:00", 4));
            library.set_daily_limits(2, 6);
            let deck = library.deck("Deck", None);
            for _ in 0..5 {
                library.due_review(&deck);
            }
            for _ in 0..2 {
                library.due_new(&deck);
            }
            let studied = library.card(&deck, 2, library.now() + DAY_MS);
            // With the day starting at 4 a.m., 03:00 on 9 June belongs to
            // 8 June, and 22:00 on 9 June and the new card at 00:30 and the
            // review at 02:30 on 10 June belong to 9 June.
            library.review(&studied, millis("2026-06-09T03:00:00-07:00"), 2, None);
            library.review(&studied, millis("2026-06-09T22:00:00-07:00"), 2, None);
            library.review(&studied, millis("2026-06-10T00:30:00-07:00"), 0, None);
            library.review(&studied, millis("2026-06-10T02:30:00-07:00"), 2, None);
            let settings = load_settings(connection)?;

            // (now, new day hour, study date, (reviews, new) left today,
            // (reviews, new) Start review studies)
            let cases = [
                // At 03:59 it is still 9 June: its two reviews leave four, so
                // the fifth due review keeps new cards out.
                ("2026-06-10T03:59:00-07:00", 4, "2026-06-09", (4, 1), (4, 0)),
                // At 4 a.m. a new study day starts with the whole allowance.
                ("2026-06-10T04:00:00-07:00", 4, "2026-06-10", (6, 2), (5, 2)),
                // With days starting at midnight, 10 June began at 00:00 and
                // holds the new card at 00:30 and the review at 02:30.
                ("2026-06-10T03:59:00-07:00", 0, "2026-06-10", (5, 1), (5, 1)),
            ];
            for (now, hour, study_date, remaining, scheduled) in cases {
                let case = format!("{now} with the day starting at {hour}");
                let time = study_time(now, hour);
                let summary = summary(connection, &time);
                let today = &summary.today;
                assert_eq!(summary.study_date, study_date, "{case}");
                assert_eq!(
                    (today.reviews_remaining, today.new_remaining),
                    remaining,
                    "{case}"
                );
                assert_eq!(
                    (today.scheduled_reviews, today.scheduled_new),
                    scheduled,
                    "{case}"
                );

                // Start review starts a scheduled session over these decks.
                let scope = workload::StudyScope {
                    deck_ids: &today.deck_ids,
                    tag_ids: &[],
                };
                let (planned, overview) = workload::planned_scheduled_cards_for_scope(
                    connection,
                    scope,
                    &settings,
                    &time,
                    "start-review",
                )?;
                assert_eq!(
                    (overview.reviews_remaining, overview.new_remaining),
                    remaining,
                    "{case}"
                );
                let planned_new = planned.iter().filter(|card| card.state == 0).count();
                let planned = (
                    i64::try_from(planned.len() - planned_new).unwrap(),
                    i64::try_from(planned_new).unwrap(),
                );
                assert_eq!(planned, scheduled, "{case}");
            }
            Ok(())
        })
        .unwrap();
}

/// Today's summary for ten due reviews after timed reviews of `active_times`
/// (one a minute, newest first), plus an untimed review, one older than the
/// window and one in the future, none of which may count.
fn estimate_after(active_times: &[i64]) -> HomeTodayDto {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library::new(connection);
            let deck = library.deck("Deck", None);
            for _ in 0..10 {
                library.due_review(&deck);
            }
            let card = library.card(&deck, 2, library.now() + 86_400_000);
            for (minutes, active_time) in (1..).zip(active_times) {
                library.timed_review(&card, minutes * MINUTE, Some(*active_time));
            }
            library.timed_review(&card, 2 * MINUTE, None);
            library.timed_review(&card, (ESTIMATE_WINDOW_DAYS + 1) * DAY_MS, Some(1_000));
            library.timed_review(&card, -MINUTE, Some(1_000));
            let time = library.time.clone();
            Ok(summary(connection, &time).today)
        })
        .unwrap()
}

#[test]
fn the_estimate_is_the_median_active_time_of_recent_timed_reviews() {
    // Nineteen timed reviews are too few, so the default applies.
    let too_few = estimate_after(&[6_000; 19]);
    assert_eq!(
        (too_few.estimate_ms_per_card, too_few.estimate_review_count),
        (DEFAULT_MS_PER_CARD, 0)
    );
    // Ten cards at twelve seconds: two minutes.
    assert_eq!(too_few.estimated_minutes, 2);

    // Twenty are enough. One card left open for ten minutes does not move
    // the median.
    let mut times = vec![6_000; 19];
    times.push(600_000);
    let enough = estimate_after(&times);
    assert_eq!(
        (enough.estimate_ms_per_card, enough.estimate_review_count),
        (6_000, 20)
    );
    // Ten cards at six seconds: one minute.
    assert_eq!(enough.estimated_minutes, 1);
}

#[test]
fn the_estimate_reads_only_the_most_recent_sample() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library::new(connection);
            // The sample's reviews were all made today; leave room for more.
            library.set_daily_limits(20, 1_000);
            let deck = library.deck("Deck", None);
            for _ in 0..30 {
                library.due_review(&deck);
            }
            let card = library.card(&deck, 2, library.now() + 86_400_000);
            let sample = ESTIMATE_SAMPLE_REVIEWS;
            for index in 0..sample {
                library.timed_review(&card, (index + 1) * MINUTE, Some(20_000));
            }
            // Older reviews inside the window, past the sample, are not read.
            for index in 0..sample {
                library.timed_review(&card, (sample + index + 1) * MINUTE, Some(1_000));
            }
            let time = library.time.clone();
            let today = summary(connection, &time).today;
            assert_eq!(
                (today.estimate_ms_per_card, today.estimate_review_count),
                (20_000, sample)
            );
            // Thirty cards at twenty seconds: ten minutes.
            assert_eq!(today.estimated_minutes, 10);
            Ok(())
        })
        .unwrap();
}

#[test]
fn medians_and_minutes_round_the_documented_way() {
    assert_eq!(median_ms_per_card(&mut []), (DEFAULT_MS_PER_CARD, 0));
    let mut odd: Vec<i64> = (1..=21).map(|value| value * 1_000).collect();
    odd.reverse();
    assert_eq!(median_ms_per_card(&mut odd), (11_000, 21));
    let mut even: Vec<i64> = (1..=20).map(|value| value * 1_000).collect();
    assert_eq!(median_ms_per_card(&mut even), (10_500, 20));

    assert_eq!(estimated_minutes(0, 12_000), 0);
    assert_eq!(estimated_minutes(-3, 12_000), 0);
    // Anything to study is at least a minute.
    assert_eq!(estimated_minutes(1, 12_000), 1);
    assert_eq!(estimated_minutes(1, 0), 1);
    // 84 seconds round down, 96 seconds round up.
    assert_eq!(estimated_minutes(7, 12_000), 1);
    assert_eq!(estimated_minutes(8, 12_000), 2);
    assert_eq!(estimated_minutes(200, 12_000), 40);
    assert_eq!(estimated_minutes(i64::MAX, i64::MAX), i64::MAX / 60_000);
}

/// The trace callback is a plain function, so captures go through a static;
/// the test that traces holds `CAPTURE_LOCK` for its whole run.
static CAPTURE_LOCK: Mutex<()> = Mutex::new(());
static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn lock<T>(mutex: &'static Mutex<T>) -> MutexGuard<'static, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn capture_statement(event: TraceEvent<'_>) {
    if let TraceEvent::Stmt(statement, _) = event {
        if let Some(sql) = statement.expanded_sql() {
            lock(&CAPTURED).push(sql);
        }
    }
}

/// Loads the summary and returns every statement it ran, values bound.
fn traced_summary(connection: &mut Connection, time: &StudyTimeContext) -> Vec<String> {
    lock(&CAPTURED).clear();
    connection.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(capture_statement));
    let loaded = load_home_summary(connection, time);
    connection.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
    loaded.expect("the summary should load");
    std::mem::take(&mut *lock(&CAPTURED))
}

fn plan(connection: &Connection, sql: &str) -> Vec<String> {
    let mut statement = connection
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap();
    statement
        .query_map([], |row| row.get::<_, String>(3))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn assert_reads_history_through_indexes(connection: &Connection, statements: &[String]) {
    assert!(
        statements
            .iter()
            .all(|sql| !sql.contains("learning_reviews")),
        "the summary must not build the learning-signal projection: {statements:#?}"
    );
    let history: Vec<&String> = statements
        .iter()
        .filter(|sql| sql.contains("review_events"))
        .collect();
    assert_eq!(history.len(), 2, "{statements:#?}");
    for sql in &history {
        let plan = plan(connection, sql);
        assert!(
            plan.iter().all(|line| !line.starts_with("SCAN")),
            "review history must be read through an index: {sql}\n{plan:#?}"
        );
    }
    let completed = history
        .iter()
        .find(|sql| sql.contains("review_event_attributions"))
        .unwrap();
    let completed = plan(connection, completed);
    // The study date's calendar dates bound the search, through the local-date
    // index or, with statistics showing few decks, a skip-scan of the deck
    // index; either way the read follows the reviews of those dates.
    assert!(
        completed[0].starts_with("SEARCH a ")
            && completed[0].contains("review_local_date>? AND review_local_date<?"),
        "today's reviews start from the reviews of the study date: {completed:#?}"
    );
    let estimate = history
        .iter()
        .find(|sql| sql.contains("review_event_scheduler_context"))
        .unwrap();
    let estimate = plan(connection, estimate);
    assert!(
        estimate[0].starts_with("SEARCH r ")
            && estimate[0].contains("review_events_reviewed_idx")
            && estimate
                .iter()
                .any(|line| line.starts_with("SEARCH context ") && line.contains("=?")),
        "recent study time walks the review-time index, newest first: {estimate:#?}"
    );
    assert!(
        estimate.iter().all(|line| !line.contains("TEMP B-TREE")),
        "the newest reviews come in index order without a sort: {estimate:#?}"
    );
}

#[test]
fn review_history_is_read_through_indexes_with_and_without_statistics() {
    let _serial = lock(&CAPTURE_LOCK);
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let mut library = Library::new(connection);
            let subject = library.subject("Biology", 1000);
            let deck = library.deck("Cells", Some(&subject));
            let loose = library.deck("Loose", None);
            let mut cards = Vec::new();
            for index in 0..40 {
                let deck = if index % 4 == 0 { &loose } else { &deck };
                cards.push(library.due_review(deck));
            }
            for (index, card) in cards.iter().enumerate() {
                let ago = i64::try_from(index).unwrap() * 37 * MINUTE;
                library.timed_review(card, ago + MINUTE, Some(5_000 + ago % 7_000));
                let at = library.now() - DAY_MS - ago;
                library.review(card, at, 2, Some(4_000));
            }
            let time = library.time.clone();

            let statements = traced_summary(connection, &time);
            assert_reads_history_through_indexes(connection, &statements);

            connection
                .execute_batch("ANALYZE")
                .map_err(CommandError::database)?;
            let statements = traced_summary(connection, &time);
            assert_reads_history_through_indexes(connection, &statements);
            Ok(())
        })
        .unwrap();
}

#[test]
fn counts_each_subjects_materials_and_picks_its_tile_figure() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let time = study_time(NOW, 4);
            let (bio, chem, second) = {
                let mut library = Library::at(connection, study_time(NOW, 4));
                let bio = library.subject("Biology", 0);
                let chem = library.subject("Chemistry", 1);
                let first = library.document(Some(&bio));
                let second = library.document(Some(&bio));
                let unfiled = library.document(None);
                for (index, title) in ["Cells", "Tissues"].iter().enumerate() {
                    library.exec(
                        "INSERT INTO lessons(id, subject_id, sort_order, title, revision,
                         created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 1, 1, 1)",
                        params![format!("{:032x}", 0xa0 + index), bio, index as i64, title],
                    );
                }
                library.exec(
                    "INSERT INTO podcasts(id, subject_id, title, target_minutes, model_id,
                     first_voice, second_voice, script_json, provenance_json, created_at)
                 VALUES (?1, ?2, 'Cells', 15, 'model', 'a', 'b', '{}', '{}', 1)",
                    params!["b".repeat(32), bio],
                );
                library.overview_guide(&bio, "active");
                // A small figure, a large unread one, and a smaller one the reading
                // marked worth studying, which wins.
                let figure = |document: &str, id: &str, ordinal: i64, width: f64, height: f64| {
                    library.exec(
                        "INSERT INTO document_figures(document_id, figure_id, ordinal,
                         page_number, page_width, page_height, kind, box_left, box_top,
                         box_width, box_height, hidden, created_at, updated_at)
                     VALUES (?1, ?2, ?3, 2, 612, 792, 'image', 50, 60, ?4, ?5, 0, 1, 1)",
                        params![document, id, ordinal, width, height],
                    );
                };
                figure(&first, "p00002-01", 0, 20.0, 20.0);
                figure(&first, "p00002-02", 1, 500.0, 600.0);
                figure(&second, "p00002-01", 0, 300.0, 200.0);
                figure(&unfiled, "p00002-01", 0, 300.0, 200.0);
                library.exec(
                    "INSERT INTO document_figure_readings(document_id, figure_id, title,
                     shows, concepts_json, explained_json, importance,
                     worth_studying, provider, model, read_at)
                 VALUES (?1, 'p00002-01', 'Cell', 'A cell.', '[]', '[]', 4, 1, 'test', 'test', 1)",
                    params![second],
                );
                (bio, chem, second)
            };

            let summary = summary(connection, &time);
            let bio_entry = entry(&summary, &bio);
            assert_eq!(bio_entry.lesson_count, 2);
            assert_eq!(bio_entry.podcast_count, 1);
            assert_eq!(bio_entry.guide_count, 1);
            let cover = bio_entry.cover.as_ref().expect("Biology has a figure");
            assert_eq!(cover.document_id, second);
            assert_eq!(cover.page_number, 2);
            assert_eq!(
                (cover.left, cover.top, cover.width, cover.height),
                (50.0, 60.0, 300.0, 200.0)
            );

            let chem_entry = entry(&summary, &chem);
            assert_eq!(
                (
                    chem_entry.lesson_count,
                    chem_entry.podcast_count,
                    chem_entry.guide_count
                ),
                (0, 0, 0)
            );
            assert!(chem_entry.cover.is_none());
            // No subject has no tile figure.
            assert!(summary.no_subject.cover.is_none());
            Ok(())
        })
        .unwrap();
}
