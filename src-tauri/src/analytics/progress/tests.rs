//! Progress endpoints over a small library read in the synthetic 2026
//! Pacific zone with the day starting at 4 a.m.

use chrono::{DateTime, Duration};
use rusqlite::{params, Connection};

use super::{
    activity, forecast, load_overview, review_log, strength, subjects, ProgressQueryDto,
    ReviewLogCursorDto,
};
use crate::{
    analytics::scope::AnalyticsContext, database::Database, error::CommandError,
    study_calendar::StudyTimeContext,
};

/// Tuesday 3 November 2026, 12:00 Pacific standard time.
const NOW: &str = "2026-11-03T12:00:00-08:00";
const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

fn millis(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .expect("test instant should parse")
        .timestamp_millis()
}

fn id(kind: u64, index: usize) -> String {
    format!("{kind:08x}{index:024x}")
}

fn time() -> StudyTimeContext {
    StudyTimeContext::pacific_2026_at(millis(NOW), 4).expect("study time should resolve")
}

fn context(range: &str, subject_ids: Vec<String>) -> AnalyticsContext {
    ProgressQueryDto {
        range: range.to_string(),
        subject_ids,
        deck_ids: Vec::new(),
        tag_ids: Vec::new(),
        custom_start_local_date: None,
        custom_end_local_date: None,
    }
    .context(time())
    .expect("progress scope should resolve")
}

struct Library {
    subjects: [String; 2],
    decks: [String; 3],
    next_card: usize,
    next_review: usize,
}

impl Library {
    /// Two subjects with a deck each, plus a deck filed under no subject.
    fn new(connection: &Connection) -> Self {
        let created = millis("2026-01-01T12:00:00-08:00");
        let subjects = [id(7, 0), id(7, 1)];
        for (index, (subject, name)) in subjects.iter().zip(["Biology", "Chemistry"]).enumerate() {
            connection
                .execute(
                    "INSERT INTO document_collections(id, name, sort_order, created_at,
                         updated_at, revision)
                     VALUES (?1, ?2, ?3, ?4, ?4, 1)",
                    params![subject, name, i64::try_from(index).unwrap(), created],
                )
                .unwrap();
        }
        let decks = [id(1, 0), id(1, 1), id(1, 2)];
        for (index, deck) in decks.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO decks (id, name, description, sort_order, created_at,
                         updated_at, revision, subject_id)
                     VALUES (?1, ?2, '', ?3, ?4, ?4, 1, ?5)",
                    params![
                        deck,
                        format!("Deck {index}"),
                        i64::try_from(index).unwrap(),
                        created,
                        subjects.get(index),
                    ],
                )
                .unwrap();
        }
        Self {
            subjects,
            decks,
            next_card: 0,
            next_review: 0,
        }
    }

    fn card(
        &mut self,
        connection: &Connection,
        deck: usize,
        state: i64,
        due_at: i64,
        stability: f64,
    ) -> String {
        let index = self.next_card;
        self.next_card += 1;
        let created = millis("2026-01-01T12:00:00-08:00");
        let (card, note) = (id(3, index), id(4, index));
        connection
            .execute(
                "INSERT INTO notes (id, deck_id, note_type, content_format,
                     content_schema_version, front_doc, back_doc, front_text, back_text,
                     created_at, updated_at, revision, duplicate_hash)
                 VALUES (?1, ?2, 'basic', 'tiptap-json', 1, '{}', '{}', ?3, 'Answer',
                     ?4, ?4, 1, ?1)",
                params![note, self.decks[deck], format!("Question {index}"), created],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cards (id, note_id, deck_id, direction, template_ordinal,
                     created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
                params![card, note, self.decks[deck], created],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO card_schedules (card_id, state, due_at, stability, difficulty,
                     elapsed_days, scheduled_days, reps, lapses, last_review_at,
                     scheduler_name, scheduler_version, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 5.0, 0, 0, ?5, 0, ?6, 'fsrs', 'test', ?6)",
                params![
                    card,
                    state,
                    due_at,
                    stability,
                    i64::from(state != 0),
                    millis(NOW) - 2 * DAY
                ],
            )
            .unwrap();
        card
    }

    /// A review of a due Review-state card at a Pacific wall time.
    fn review(
        &mut self,
        connection: &Connection,
        card: &str,
        at: &str,
        remembered: bool,
    ) -> String {
        let index = self.next_review;
        self.next_review += 1;
        let instant = DateTime::parse_from_rfc3339(at).unwrap();
        let reviewed_at = instant.timestamp_millis();
        let review = id(5, index);
        connection
            .execute(
                "INSERT INTO review_events (id, submission_id, session_id, card_id,
                     reviewed_at, rating, answer_correct, confidence, response_time_ms,
                     state_before, state_after, due_before, due_after, stability_before,
                     stability_after, difficulty_before, difficulty_after,
                     scheduled_days_before, scheduled_days_after, scheduler_name,
                     scheduler_version, created_at)
                 VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, 4000, 2, 2, ?4, ?8, 1.0, 2.0,
                     5.0, 5.0, 0, 1, 'fsrs', 'test', ?4)",
                params![
                    review,
                    id(6, index),
                    card,
                    reviewed_at,
                    if remembered { 3 } else { 1 },
                    i64::from(remembered),
                    if remembered {
                        "somewhat_confident"
                    } else {
                        "wrong"
                    },
                    reviewed_at + DAY,
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO review_event_attributions (review_event_id, reviewed_at,
                     review_local_date, time_zone_id, utc_offset_minutes, card_id, note_id,
                     note_revision, owner_deck_id_at_review, captured_deck_name, direction,
                     card_type, card_label, presentation_variant, question_excerpt,
                     question_text_hash, question_was_truncated, attribution_source)
                 SELECT ?1, ?2, ?3, 'America/Los_Angeles', ?4, c.id, c.note_id, 1, c.deck_id,
                     'Deck', 'forward', 'basic', 'Basic', '', 'Question', ?5, 0, 'captured'
                 FROM cards c WHERE c.id = ?6",
                params![
                    review,
                    reviewed_at,
                    instant.date_naive().to_string(),
                    instant.offset().local_minus_utc() / 60,
                    format!("{index:064x}"),
                    card,
                ],
            )
            .unwrap();
        review
    }
}

fn with_library<T>(
    test: impl FnOnce(&mut Connection, &mut Library) -> Result<T, CommandError>,
) -> T {
    Database::open_in_memory()
        .expect("database should open")
        .with_connection(|connection| {
            let mut library = Library::new(connection);
            test(connection, &mut library)
        })
        .expect("progress test should run")
}

#[test]
fn the_forecast_places_each_review_on_its_study_day() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let today = time();
        library.card(connection, 0, 2, now - 2 * DAY, 5.0); // overdue
        library.card(connection, 0, 2, today.tomorrow_start_utc_millis - 1, 5.0); // 3:59 a.m.
        library.card(connection, 1, 2, today.tomorrow_start_utc_millis, 5.0); // tomorrow
        library.card(
            connection,
            2,
            3,
            today.tomorrow_start_utc_millis + 12 * DAY,
            5.0,
        );
        library.card(
            connection,
            2,
            2,
            today.tomorrow_start_utc_millis + 13 * DAY,
            5.0,
        ); // day 15
        library.card(connection, 0, 0, now, 0.0); // new
        let suspended = library.card(connection, 0, 2, now, 5.0);
        connection
            .execute(
                "UPDATE cards SET suspended_at = ?1 WHERE id = ?2",
                params![now, suspended],
            )
            .unwrap();

        let result = forecast::load(connection, &context("30d", Vec::new()))?;
        let counts: Vec<i64> = result.days.iter().map(|day| day.due_count).collect();
        assert_eq!(result.days.len(), 14);
        assert_eq!(result.days[0].local_date, "2026-11-03");
        assert_eq!(counts[0], 2);
        assert_eq!(counts[1], 1);
        assert_eq!(counts[13], 1);
        assert_eq!(counts.iter().sum::<i64>(), 4);
        assert_eq!(result.overdue_count, 1);
        assert_eq!(result.new_available_count, 1);

        let biology = forecast::load(
            connection,
            &context("30d", vec![library.subjects[0].clone()]),
        )?;
        assert_eq!(biology.days[0].due_count, 2);
        assert_eq!(biology.days[1].due_count, 0);
        Ok(())
    });
}

#[test]
fn card_strength_buckets_and_estimates_recall() {
    with_library(|connection, library| {
        let now = millis(NOW);
        library.card(connection, 0, 0, now, 0.0);
        library.card(connection, 0, 1, now, 0.5);
        library.card(connection, 0, 3, now, 2.0);
        library.card(connection, 0, 2, now, 20.0);
        library.card(connection, 0, 2, now, 21.0);
        library.card(connection, 0, 2, now, 300.0);

        let result = strength::load(connection, &context("30d", Vec::new()))?;
        assert_eq!(
            (
                result.new_count,
                result.learning_count,
                result.getting_there_count,
                result.solid_count
            ),
            (1, 2, 1, 2)
        );
        // Every studied card was last reviewed two days ago.
        assert_eq!(result.estimated_recall_card_count, 5);
        let recall = result
            .estimated_recall
            .expect("studied cards have a prediction");
        assert!((0.5..1.0).contains(&recall), "{recall}");
        Ok(())
    });
}

#[test]
fn streaks_count_study_days_even_for_deleted_cards() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let card = library.card(connection, 0, 2, now, 5.0);
        let gone = library.card(connection, 1, 2, now, 5.0);
        // A three-day run in October, then 1 to 2 November and today. The
        // 3:30 a.m. review on 2 November belongs to 1 November's study day.
        for at in [
            "2026-10-10T09:00:00-07:00",
            "2026-10-11T09:00:00-07:00",
            "2026-10-12T09:00:00-07:00",
            "2026-11-01T20:00:00-08:00",
            "2026-11-02T03:30:00-08:00",
            "2026-11-03T09:00:00-08:00",
        ] {
            library.review(connection, &card, at, true);
        }
        library.review(connection, &gone, "2026-11-02T20:00:00-08:00", true);
        connection
            .execute(
                "UPDATE cards SET deleted_at = ?1 WHERE id = ?2",
                params![now, gone],
            )
            .unwrap();

        let result = activity::load(connection, &time())?;
        let current = result.current_streak.expect("today has a review");
        assert_eq!(
            (
                current.length,
                current.start_local_date.as_str(),
                current.end_local_date.as_str()
            ),
            (3, "2026-11-01", "2026-11-03")
        );
        let longest = result.longest_streak.expect("there are reviews");
        assert_eq!(
            (longest.length, longest.start_local_date.as_str()),
            (3, "2026-10-10")
        );
        let dates: Vec<&str> = result
            .days
            .iter()
            .map(|day| day.local_date.as_str())
            .collect();
        assert_eq!(
            dates,
            [
                "2026-10-10",
                "2026-10-11",
                "2026-10-12",
                "2026-11-01",
                "2026-11-02",
                "2026-11-03"
            ]
        );
        assert_eq!(result.days[3].review_count, 2);
        // Saturday 10 October to Monday 12 October, then Sunday 1 November
        // with two reviews: Sunday is busiest.
        assert_eq!(result.busiest_weekday, Some(6));
        Ok(())
    });
}

#[test]
fn subjects_report_cards_now_and_reviews_in_the_range() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let today = time();
        let biology = library.card(connection, 0, 2, now - HOUR, 30.0);
        library.card(connection, 0, 2, today.tomorrow_start_utc_millis - 1, 3.0);
        let chemistry = library.card(connection, 1, 2, now + 5 * DAY, 3.0);
        library.card(connection, 2, 0, now, 0.0);
        library.review(connection, &biology, "2026-11-02T09:00:00-08:00", true);
        library.review(connection, &chemistry, "2026-11-02T10:00:00-08:00", false);
        library.review(connection, &chemistry, "2026-11-01T10:00:00-08:00", true);

        let rows = subjects::load(connection, &context("7d", Vec::new()))?;
        let names: Vec<Option<&str>> = rows.iter().map(|row| row.name.as_deref()).collect();
        // Most due today first, then the library's subject order; the decks
        // under no subject come last.
        assert_eq!(names, [Some("Biology"), Some("Chemistry"), None]);
        assert_eq!(
            (
                rows[0].card_count,
                rows[0].solid_count,
                rows[0].due_today_count
            ),
            (2, 1, 2)
        );
        assert_eq!((rows[0].review_count, rows[0].remembered_count), (1, 1));
        assert_eq!((rows[1].review_count, rows[1].remembered_count), (2, 1));
        assert_eq!(rows[1].true_retention, Some(0.5));
        assert_eq!(
            (rows[2].subject_id.as_deref(), rows[2].card_count),
            (None, 1)
        );
        Ok(())
    });
}

#[test]
fn the_review_log_pages_through_every_review_once_including_ties() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let card = library.card(connection, 0, 2, now, 5.0);
        let mut expected = Vec::new();
        for index in 0..120_i64 {
            // Pairs of reviews share a review time.
            let at = DateTime::from_timestamp_millis(now - (index / 2 + 1) * HOUR)
                .unwrap()
                .with_timezone(&chrono::FixedOffset::west_opt(8 * 3600).unwrap())
                .to_rfc3339();
            expected.push(library.review(connection, &card, &at, index % 3 != 0));
        }
        for range in ["all", "30d", "7d"] {
            let context = context(range, Vec::new());
            let mut seen = Vec::new();
            let mut cursor: Option<ReviewLogCursorDto> = None;
            let mut pages = 0;
            loop {
                let page = review_log::load_page(connection, &context, cursor.as_ref())?;
                pages += 1;
                assert!(page.reviews.len() <= 50);
                seen.extend(page.reviews.iter().map(|review| review.id.clone()));
                match page.next {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            }
            assert_eq!(pages, 3, "{range}");
            let mut sorted_seen = seen.clone();
            sorted_seen.sort();
            let mut sorted_expected = expected.clone();
            sorted_expected.sort();
            assert_eq!(sorted_seen, sorted_expected, "{range}");
        }
        Ok(())
    });
}

#[test]
fn the_overview_compares_with_the_previous_period_and_counts_study_days() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let card = library.card(connection, 0, 2, now, 5.0);
        // Previous 7-day period: 20 to 26 October. Current: 27 October to today.
        library.review(connection, &card, "2026-10-21T09:00:00-07:00", false);
        library.review(connection, &card, "2026-10-22T09:00:00-07:00", true);
        library.review(connection, &card, "2026-10-30T09:00:00-07:00", true);
        library.review(connection, &card, "2026-10-30T10:00:00-07:00", true);
        library.review(connection, &card, "2026-11-03T09:00:00-08:00", false);

        let overview = load_overview(connection, context("7d", Vec::new()))?;
        assert_eq!(overview.summary.review_count, 3);
        assert_eq!(overview.study_day_count, 2);
        let previous = overview
            .previous_summary
            .expect("a 7-day range has a previous one");
        assert_eq!((previous.review_count, previous.correct_count), (2, 1));
        assert_eq!(overview.timeline.len(), 7);
        Ok(())
    });
}

#[test]
fn weekly_buckets_end_with_a_whole_week_ending_today() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let card = library.card(connection, 0, 2, now, 5.0);
        library.review(connection, &card, "2026-11-03T09:00:00-08:00", true);
        library.review(connection, &card, "2026-10-28T09:00:00-07:00", true);
        library.review(connection, &card, "2026-10-27T09:00:00-07:00", true);
        let overview = load_overview(connection, context("90d", Vec::new()))?;
        let last = overview.timeline.last().expect("90 days has buckets");
        assert_eq!(
            (last.start_local_date.as_str(), last.end_local_date.as_str()),
            ("2026-10-28", "2026-11-03")
        );
        assert_eq!(last.review_count, 2);
        let first = overview.timeline.first().unwrap();
        assert_eq!(
            first.start_local_date,
            (time().local_date - Duration::days(89)).to_string()
        );
        assert_eq!(overview.timeline.len(), 13);
        Ok(())
    });
}
