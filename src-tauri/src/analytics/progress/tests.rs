//! Progress endpoints over a small library read in the synthetic 2026
//! Pacific zone with the day starting at 4 a.m.

use chrono::{DateTime, Duration};
use rusqlite::{params, Connection};

use super::{
    activity, forecast, hours, load_overview, load_topics, review_log, session, strength, subjects,
    ProgressQueryDto, ReviewLogCursorDto,
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
fn topics_count_only_the_reviews_in_the_chosen_subject() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let tag = id(8, 0);
        connection
            .execute(
                "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                 VALUES (?1, 'Membranes', 'membranes', ?2, ?2)",
                params![tag, now],
            )
            .unwrap();
        let biology = library.card(connection, 0, 2, now - HOUR, 3.0);
        let chemistry = library.card(connection, 1, 2, now - HOUR, 3.0);
        for (card, remembered) in [(&biology, true), (&chemistry, false)] {
            connection
                .execute(
                    "INSERT INTO note_tags (note_id, tag_id)
                     SELECT note_id, ?2 FROM cards WHERE id = ?1",
                    params![card, tag],
                )
                .unwrap();
            let review = library.review(connection, card, "2026-11-02T09:00:00-08:00", remembered);
            connection
                .execute(
                    "INSERT INTO review_event_tag_attributions (review_event_id, tag_id, tag_name)
                     VALUES (?1, ?2, 'Membranes')",
                    params![review, tag],
                )
                .unwrap();
        }

        let everywhere = load_topics(connection, &context("7d", Vec::new()))?;
        assert_eq!(
            everywhere
                .iter()
                .map(|topic| (
                    topic.tag_name.as_str(),
                    topic.review_count,
                    topic.studyable_due_count
                ))
                .collect::<Vec<_>>(),
            [("Membranes", 2, 2)]
        );
        let biology_only = load_topics(
            connection,
            &context("7d", vec![library.subjects[0].clone()]),
        )?;
        assert_eq!(
            biology_only
                .iter()
                .map(|topic| (
                    topic.review_count,
                    topic.correct_count,
                    topic.studyable_due_count
                ))
                .collect::<Vec<_>>(),
            [(1, 1, 1)]
        );
        Ok(())
    });
}

#[test]
fn a_session_summary_reports_its_misses_new_solid_cards_and_what_is_still_due() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let session = id(9, 0);
        connection
            .execute(
                "INSERT INTO practice_sessions (id, selected_deck_ids_json, shuffle, card_count,
                     started_at, completed_at, rated_count, correct_count, mode)
                 VALUES (?1, ?2, 0, 2, ?3, ?3, 2, 1, 'scheduled')",
                params![session, format!("[\"{}\"]", library.decks[0]), now - HOUR],
            )
            .unwrap();
        let tag = id(8, 0);
        connection
            .execute(
                "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                 VALUES (?1, 'Muscarinic antagonists', 'muscarinic antagonists', ?2, ?2)",
                params![tag, now],
            )
            .unwrap();
        let solid = library.card(connection, 0, 2, now + 3 * DAY, 30.0);
        let missed = library.card(connection, 0, 2, now + 3 * DAY, 2.0);
        let still_due = library.card(connection, 0, 2, now - HOUR, 3.0);
        connection
            .execute(
                "INSERT INTO note_tags (note_id, tag_id) SELECT note_id, ?2 FROM cards WHERE id = ?1",
                params![missed, tag],
            )
            .unwrap();
        // Yesterday's review keeps a two-day streak going.
        library.review(connection, &still_due, "2026-11-02T09:00:00-08:00", true);
        let made_solid = library.review(connection, &solid, "2026-11-03T10:00:00-08:00", true);
        let forgotten = library.review(connection, &missed, "2026-11-03T10:01:00-08:00", false);
        connection
            .execute(
                "UPDATE review_events SET session_id = ?1 WHERE id IN (?2, ?3)",
                params![session, made_solid, forgotten],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE review_events SET stability_before = 10.0, stability_after = 30.0
                 WHERE id = ?1",
                params![made_solid],
            )
            .unwrap();

        let summary = session::load(connection, time(), &session)?;
        assert_eq!(
            (
                summary.rated_count,
                summary.remembered_count,
                summary.study_time_ms
            ),
            (2, 1, 8_000)
        );
        assert_eq!(summary.now_solid_count, 1);
        assert_eq!(summary.missed_card_ids, [missed]);
        let topic = summary
            .missed_topic
            .expect("the missed card's tag should be named");
        assert_eq!(
            (topic.name.as_str(), topic.card_count),
            ("Muscarinic antagonists", 1)
        );
        assert_eq!(summary.due_today_count, Some(1));
        assert_eq!(
            (
                summary.today.local_date.as_str(),
                summary.today.review_count
            ),
            ("2026-11-03", 2)
        );
        assert_eq!(summary.current_streak.map(|streak| streak.length), Some(2));
        assert!(summary.last_as_long_streak.is_none());

        // A session filtered to that tag names no topic: every card has it.
        connection
            .execute(
                "UPDATE practice_sessions SET selected_tag_ids_json = ?2 WHERE id = ?1",
                params![session, format!("[\"{tag}\"]")],
            )
            .unwrap();
        assert!(session::load(connection, time(), &session)?
            .missed_topic
            .is_none());
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

        let overview = load_overview(connection, &context("7d", Vec::new()))?;
        assert_eq!(overview.summary.review_count, 3);
        assert_eq!(overview.study_day_count, 2);
        let previous = overview
            .previous_summary
            .expect("a 7-day range has a previous one");
        assert_eq!((previous.review_count, previous.remembered_count), (2, 1));
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
        let overview = load_overview(connection, &context("90d", Vec::new()))?;
        let last = overview.timeline.last().expect("90 days has buckets");
        assert_eq!(
            (last.start_local_date.as_str(), last.end_local_date.as_str()),
            ("2026-10-28", "2026-11-03")
        );
        assert_eq!(last.totals.review_count, 2);
        let first = overview.timeline.first().unwrap();
        assert_eq!(
            first.start_local_date,
            (time().local_date - Duration::days(89)).to_string()
        );
        assert_eq!(overview.timeline.len(), 13);
        Ok(())
    });
}

static CAPTURED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn capture_statement(event: rusqlite::trace::TraceEvent<'_>) {
    if let rusqlite::trace::TraceEvent::Stmt(statement, _) = event {
        if let Some(sql) = statement.expanded_sql() {
            CAPTURED
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(sql);
        }
    }
}

/// The plans of the reads `run` executes, values bound. Tests that call this
/// run one at a time through `CAPTURED`'s lock order: each takes the reads it
/// made on its own connection only.
fn traced_plans(
    connection: &mut Connection,
    run: impl FnOnce(&mut Connection) -> Result<(), CommandError>,
) -> Vec<(String, Vec<String>)> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    CAPTURED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    connection.trace_v2(
        rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT,
        Some(capture_statement),
    );
    let result = run(connection);
    connection.trace_v2(rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT, None);
    result.expect("traced reads should run");
    let reads = std::mem::take(
        &mut *CAPTURED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    reads
        .into_iter()
        .filter(|sql| sql.starts_with("SELECT") || sql.starts_with("WITH"))
        .map(|sql| {
            let mut statement = connection
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap();
            let plan = statement
                .query_map([], |row| row.get::<_, String>(3))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            (sql, plan)
        })
        .collect()
}

/// The outermost table loop of a plan.
fn first_loop(plan: &[String]) -> &str {
    plan.iter()
        .find(|step| step.starts_with("SEARCH ") || step.starts_with("SCAN "))
        .map_or("", String::as_str)
}

#[test]
fn review_reads_start_from_an_index_on_review_time_or_study_date() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let card = library.card(connection, 0, 2, now, 5.0);
        for day in 0..30 {
            library.review(
                connection,
                &card,
                &DateTime::from_timestamp_millis(now - day * DAY - HOUR)
                    .unwrap()
                    .with_timezone(&chrono::FixedOffset::west_opt(8 * 3600).unwrap())
                    .to_rfc3339(),
                true,
            );
        }
        // Every page of the log, for every range, walks reviews newest first
        // by review time and stops after the page; nothing sorts all reviews.
        for range in ["7d", "all"] {
            let page = review_log::load_page(connection, &context(range, Vec::new()), None)?;
            let cursor = page.next.clone();
            for before in [None, cursor] {
                let plans = traced_plans(connection, |connection| {
                    review_log::load_page(connection, &context(range, Vec::new()), before.as_ref())
                        .map(|_| ())
                });
                let (_, plan) = plans
                    .iter()
                    .find(|(sql, _)| sql.contains(review_log::REVIEW_LOG_FROM_SQL))
                    .expect("the log reads reviews");
                assert!(
                    first_loop(plan).contains("r USING INDEX review_events_reviewed_idx")
                        && !plan
                            .iter()
                            .any(|step| step == "USE TEMP B-TREE FOR ORDER BY"),
                    "{range}: {plan:#?}"
                );
            }
        }
        // The activity calendar reads its 26 weeks through the local-date index.
        let plans = traced_plans(connection, |connection| {
            activity::load(connection, &time()).map(|_| ())
        });
        let (_, plan) = plans
            .iter()
            .find(|(sql, _)| sql.contains("GROUP BY 1"))
            .expect("the calendar groups reviews by study date");
        assert!(
            first_loop(plan).contains("a USING INDEX review_event_attributions_local_date_idx"),
            "{plan:#?}"
        );
        Ok(())
    });
}

fn context_at(range: &str, hour: u32, tag_ids: Vec<String>) -> AnalyticsContext {
    ProgressQueryDto {
        range: range.to_string(),
        subject_ids: Vec::new(),
        deck_ids: Vec::new(),
        tag_ids,
        custom_start_local_date: None,
        custom_end_local_date: None,
    }
    .context(StudyTimeContext::pacific_2026_at(millis(NOW), hour).unwrap())
    .unwrap()
}

/// The rollup rows and the history rows for the same scope, sorted.
fn both_sources(
    connection: &mut Connection,
    context: &AnalyticsContext,
    by_subject: bool,
) -> (Vec<hours::HourRow>, Vec<hours::HourRow>) {
    crate::study_activity::refresh_rollups(connection).unwrap();
    let first = context
        .previous_window()
        .or(context.window.current.as_ref())
        .map(|window| window.start_local_date);
    let end = context.window.end_local_date;
    let mut rollups = hours::rollup_rows(connection, context, by_subject, first, end).unwrap();
    let mut history = hours::history_rows(connection, context, by_subject).unwrap();
    // The rollup read spans whole wall dates; keep only what the period's
    // study dates hold, as `hours::load` does.
    let hour = context.time.new_day_start_hour();
    let in_period = |row: &hours::HourRow| {
        let date = crate::study_activity::study_date(
            crate::study_activity::parse_date(&row.1).unwrap(),
            row.2,
            hour,
        );
        first.is_none_or(|first| date >= first) && date <= end
    };
    rollups.retain(in_period);
    history.retain(in_period);
    rollups.sort_by(|left, right| (&left.0, &left.1, left.2).cmp(&(&right.0, &right.1, right.2)));
    history.sort_by(|left, right| (&left.0, &left.1, left.2).cmp(&(&right.0, &right.1, right.2)));
    (rollups, history)
}

fn assert_rollups_match_history(connection: &mut Connection, step: &str) {
    for range in ["all", "30d", "7d"] {
        for hour in [0, 4, 23] {
            for by_subject in [false, true] {
                let context = context_at(range, hour, Vec::new());
                let (rollups, history) = both_sources(connection, &context, by_subject);
                assert_eq!(rollups, history, "{step}: {range}, {hour}:00, {by_subject}");
            }
        }
    }
}

#[test]
fn rollups_follow_every_change_to_the_history_they_summarize() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let cards: Vec<String> = (0..6)
            .map(|index| library.card(connection, index % 3, 2, now, 5.0))
            .collect();
        let mut at = millis("2026-09-01T00:30:00-07:00");
        for (index, card) in cards.iter().cycle().take(60).enumerate() {
            let instant = DateTime::from_timestamp_millis(at)
                .unwrap()
                .with_timezone(&chrono::FixedOffset::west_opt(7 * 3600).unwrap());
            library.review(connection, card, &instant.to_rfc3339(), index % 4 != 0);
            at += 25 * HOUR + 17 * 60_000;
        }
        // Built from scratch, then kept up to date one review at a time.
        assert_rollups_match_history(connection, "first build");
        library.review(connection, &cards[0], "2026-11-03T03:30:00-08:00", true);
        library.review(connection, &cards[4], "2026-11-03T09:00:00-08:00", false);
        let built: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM review_day_rollup_built_decks",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(built, 3, "a new review keeps its deck built");
        assert_rollups_match_history(connection, "new reviews");

        let step = |connection: &mut Connection, step: &str, sql: &str, card: &str| {
            connection.execute(sql, params![card, now]).unwrap();
            assert_rollups_match_history(connection, step);
        };
        step(
            connection,
            "move a card",
            "UPDATE cards SET deck_id = (SELECT id FROM decks WHERE id <> cards.deck_id
                 ORDER BY id LIMIT 1), updated_at = ?2 WHERE id = ?1",
            &cards[1],
        );
        step(
            connection,
            "delete a card",
            "UPDATE cards SET deleted_at = ?2 WHERE id = ?1",
            &cards[2],
        );
        step(
            connection,
            "restore it",
            "UPDATE cards SET deleted_at = NULL WHERE id = ?1 AND ?2 > 0",
            &cards[2],
        );
        step(
            connection,
            "delete a note",
            "UPDATE notes SET deleted_at = ?2 WHERE id = (SELECT note_id FROM cards
                 WHERE id = ?1)",
            &cards[3],
        );
        // A retired copy's reviews count under its surviving card.
        connection
            .execute(
                "INSERT INTO card_merges(retired_card_id, surviving_card_id, retired_note_id,
                     surviving_note_id, reason, copied_at)
                 SELECT r.id, s.id, r.note_id, s.note_id, 'collection_copy', ?3
                 FROM cards r, cards s WHERE r.id = ?1 AND s.id = ?2",
                params![cards[5], cards[0], now],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE cards SET deleted_at = ?2 WHERE id = ?1",
                params![cards[5], now],
            )
            .unwrap();
        assert_rollups_match_history(connection, "retire a copy");
        step(
            connection,
            "delete a review",
            "DELETE FROM review_events WHERE id = (SELECT review_event_id
                 FROM review_event_attributions WHERE card_id = ?1 AND ?2 > 0
                 ORDER BY reviewed_at LIMIT 1)",
            &cards[4],
        );
        step(
            connection,
            "delete a deck",
            "UPDATE decks SET deleted_at = ?2 WHERE id = (SELECT deck_id FROM cards
                 WHERE id = ?1)",
            &cards[4],
        );
        Ok(())
    });
}

#[test]
fn the_overview_reads_the_same_totals_with_or_without_a_tag_filter() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let card = library.card(connection, 0, 2, now, 5.0);
        let other = library.card(connection, 1, 2, now, 5.0);
        library.review(connection, &card, "2026-10-21T09:00:00-07:00", false);
        library.review(connection, &card, "2026-10-30T09:00:00-07:00", true);
        library.review(connection, &other, "2026-11-02T02:00:00-08:00", true);
        library.review(connection, &card, "2026-11-03T09:00:00-08:00", false);
        let tag = id(8, 0);
        connection
            .execute(
                "INSERT INTO tags(id, name, normalized_name, created_at, updated_at)
                 VALUES (?1, 'all', 'all', ?2, ?2)",
                params![tag, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO review_event_tag_attributions(review_event_id, tag_id, tag_name)
                 SELECT review_event_id, ?1, 'all' FROM review_event_attributions",
                [&tag],
            )
            .unwrap();
        for range in ["7d", "30d", "90d", "1y", "all"] {
            let plain = load_overview(connection, &context_at(range, 4, Vec::new()))?;
            let tagged = load_overview(connection, &context_at(range, 4, vec![tag.clone()]))?;
            assert_eq!(plain.summary, tagged.summary, "{range}");
            assert_eq!(plain.previous_summary, tagged.previous_summary, "{range}");
            assert_eq!(plain.timeline, tagged.timeline, "{range}");
            assert_eq!(plain.study_day_count, tagged.study_day_count, "{range}");
        }
        let all = load_overview(connection, &context_at("all", 4, Vec::new()))?;
        assert_eq!(all.summary.review_count, 4);
        assert_eq!(all.scope.start_local_date.as_deref(), Some("2026-10-21"));
        let months: Vec<&str> = all
            .timeline
            .iter()
            .map(|bucket| bucket.start_local_date.as_str())
            .collect();
        assert_eq!(months, ["2026-10-01", "2026-11-01"]);
        // The 2 a.m. review on 2 November belongs to 1 November's study day.
        assert_eq!(all.study_day_count, 4);
        Ok(())
    });
}

#[test]
fn purged_history_keeps_its_days_streaks_and_study_time() {
    with_library(|connection, library| {
        let now = millis(NOW);
        let kept = library.card(connection, 0, 2, now, 5.0);
        let gone = library.card(connection, 1, 2, now, 5.0);
        library.review(connection, &kept, "2026-11-03T09:00:00-08:00", true);
        for at in [
            "2026-10-31T09:00:00-07:00",
            "2026-11-01T09:00:00-08:00",
            "2026-11-02T03:30:00-08:00",
            "2026-11-02T09:00:00-08:00",
        ] {
            library.review(connection, &gone, at, true);
        }
        let before = activity::load(connection, &time())?;
        assert_eq!(before.current_streak.as_ref().unwrap().length, 4);

        let note: String = connection
            .query_row("SELECT note_id FROM cards WHERE id = ?1", [&gone], |row| {
                row.get(0)
            })
            .unwrap();
        let deleted_at = now - HOUR;
        connection
            .execute(
                "UPDATE notes SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![note, deleted_at],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE cards SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![gone, deleted_at],
            )
            .unwrap();
        let operation = id(9, 0);
        connection
            .execute(
                "INSERT INTO deletion_operations(id, entity_kind, subject_label, deck_id,
                     note_ids_json, card_ids_json, deleted_at, expires_at, finalized_at,
                     created_at)
                 VALUES (?1, 'cards', 'one card', NULL, ?2, ?3, ?4, ?4, ?4, ?4)",
                params![
                    operation,
                    serde_json::to_string(&[&note]).unwrap(),
                    serde_json::to_string(&[&gone]).unwrap(),
                    deleted_at
                ],
            )
            .unwrap();
        crate::deletion_purge::purge_finalized_in_connection(connection)?;
        let left: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM review_events WHERE card_id = ?1",
                [&gone],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(left, 0, "the purge removed the card's history");

        let after = activity::load(connection, &time())?;
        assert_eq!(after.current_streak, before.current_streak);
        assert_eq!(after.longest_streak, before.longest_streak);
        let totals = |result: &activity::ProgressActivityDto| {
            result
                .days
                .iter()
                .map(|day| (day.local_date.clone(), day.review_count, day.study_time_ms))
                .collect::<Vec<_>>()
        };
        assert_eq!(totals(&after), totals(&before));
        // Moving the new-day hour regroups kept history like stored history:
        // at midnight the 3:30 a.m. review belongs to 2 November.
        let midnight = StudyTimeContext::pacific_2026_at(now, 0).unwrap();
        let regrouped = activity::load(connection, &midnight)?;
        let dates: Vec<(String, i64)> = regrouped
            .days
            .iter()
            .map(|day| (day.local_date.clone(), day.review_count))
            .collect();
        assert_eq!(
            dates,
            [
                ("2026-10-31".to_string(), 1),
                ("2026-11-01".to_string(), 1),
                ("2026-11-02".to_string(), 2),
                ("2026-11-03".to_string(), 1),
            ]
        );
        // The overview counts only cards that still exist.
        let overview = load_overview(connection, &context("7d", Vec::new()))?;
        assert_eq!(overview.summary.review_count, 1);
        Ok(())
    });
}
