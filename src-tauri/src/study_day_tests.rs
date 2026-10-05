//! Study-day boundary behaviour across analytics, daily limits, search,
//! practice order and scheduling controls.
//!
//! One fixture of reviews around midnight, 4 a.m., both 2026 Pacific
//! daylight-saving transitions and one review saved in Tokyo. Every review is
//! written as its local wall time with the offset in force, which is exactly
//! what a review attribution stores. Everything is read in the synthetic 2026
//! Pacific zone, so no result depends on the machine's time zone.
//!
//! - With the day starting at midnight, every result equals the output of the
//!   code before study days existed, captured from this fixture into
//!   `tests/fixtures/study-day/midnight-boundary-golden.txt`.
//! - For several start hours, an oracle dates every review with SQLite's
//!   `date(reviewed_at / 1000 + utc_offset_minutes * 60 - H * 3600,
//!   'unixepoch')` and analytics, the daily limits and the search filters must
//!   agree with it.
//! - Named scenarios pin the 4 a.m. behaviour the learner sees.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate};
use rusqlite::{params, Connection};

use crate::{
    analytics::{load_analytics_at, AnalyticsDto, AnalyticsQueryDto},
    authoring::{list_matching_cards_at, SearchCardsInput},
    database::Database,
    error::CommandError,
    schedule_controls::{apply_at, RescheduleIntent, ScheduleActionInput},
    settings::load_settings,
    study_calendar::StudyTimeContext,
    workload::{planned_extra_cards_for_scope, workload_overview_for_scope, StudyScope},
};

const MIDNIGHT_GOLDEN: &str =
    include_str!("../tests/fixtures/study-day/midnight-boundary-golden.txt");

/// Tuesday 3 November 2026, 12:00 Pacific standard time.
const NOW: &str = "2026-11-03T12:00:00-08:00";

const DECKS: [&str; 2] = ["Alpha", "Beta"];
const TAGS: [&str; 2] = ["Topic 0", "Topic 1"];

/// (deck, tags) per card. Card 3 is never reviewed.
const CARDS: [(usize, &[usize]); 9] = [
    (0, &[0]),
    (0, &[0, 1]),
    (0, &[1]),
    (0, &[0]),
    (1, &[1]),
    (1, &[0]),
    (1, &[]),
    (1, &[1]),
    (0, &[1]),
];

struct Review {
    card: usize,
    /// Local wall time and the offset in force when the review was saved.
    at: &'static str,
    zone: &'static str,
    rating: i64,
    new: bool,
}

const fn review(card: usize, at: &'static str, rating: i64, new: bool) -> Review {
    Review {
        card,
        at,
        zone: "America/Los_Angeles",
        rating,
        new,
    }
}

const REVIEWS: [Review; 26] = [
    review(5, "2025-12-15T10:00:00-08:00", 3, true),
    review(5, "2026-01-20T23:30:00-08:00", 2, false),
    review(6, "2026-03-07T23:30:00-08:00", 3, true),
    // Spring forward: 02:00 PST becomes 03:00 PDT on 8 March.
    review(6, "2026-03-08T01:30:00-08:00", 1, false),
    review(6, "2026-03-08T03:30:00-07:00", 4, false),
    review(4, "2026-03-08T04:30:00-07:00", 3, true),
    review(4, "2026-03-09T00:30:00-07:00", 3, false),
    review(5, "2026-06-10T12:00:00-07:00", 4, false),
    review(8, "2026-10-04T03:00:00-07:00", 2, true),
    // Card 8's last review: local date 5 October, study date 4 October at 4 a.m.
    review(8, "2026-10-05T03:00:00-07:00", 3, false),
    review(2, "2026-10-05T05:00:00-07:00", 3, true),
    // Saved while travelling: 05:00 in Tokyo is still 19 October in Los Angeles.
    Review {
        card: 2,
        at: "2026-10-20T05:00:00+09:00",
        zone: "Asia/Tokyo",
        rating: 4,
        new: false,
    },
    review(1, "2026-10-28T02:00:00-07:00", 3, true),
    review(1, "2026-10-31T23:30:00-07:00", 2, false),
    // Fall back: 02:00 PDT becomes 01:00 PST on 1 November.
    review(2, "2026-11-01T00:30:00-07:00", 3, false),
    review(1, "2026-11-01T01:30:00-07:00", 1, false),
    review(2, "2026-11-01T01:30:00-08:00", 3, false),
    review(1, "2026-11-01T03:30:00-08:00", 4, false),
    review(2, "2026-11-01T04:30:00-08:00", 3, false),
    review(0, "2026-11-02T03:59:00-08:00", 3, true),
    review(0, "2026-11-02T04:00:00-08:00", 3, false),
    review(0, "2026-11-03T00:30:00-08:00", 2, false),
    review(7, "2026-11-03T00:45:00-08:00", 3, true),
    review(1, "2026-11-03T03:30:00-08:00", 3, false),
    review(4, "2026-11-03T04:00:00-08:00", 4, false),
    review(5, "2026-11-03T11:00:00-08:00", 3, false),
];

const RANGES: [&str; 5] = ["7d", "30d", "1y", "all", "custom"];
const CUSTOM_RANGE: (&str, &str) = ("2026-03-07", "2026-03-10");
const REVIEWED_FILTERS: [&str; 5] = [
    "today",
    "past_7_local_dates",
    "past_30_local_dates",
    "before_past_30_local_dates",
    "never",
];

fn instant(value: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(value).expect("fixture time should parse")
}

fn millis(value: &str) -> i64 {
    instant(value).timestamp_millis()
}

fn date(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("fixture date should parse")
}

fn id(kind: u64, index: usize) -> String {
    format!("{kind:08x}{index:024x}")
}

fn card_label(card_id: &str) -> String {
    (0..CARDS.len())
        .find(|index| id(3, *index) == card_id)
        .map_or_else(|| card_id.to_string(), |index| format!("c{index}"))
}

fn review_index(review_id: &str) -> Option<usize> {
    (0..REVIEWS.len()).find(|index| id(5, *index) == review_id)
}

fn review_label(review_id: &str) -> String {
    review_index(review_id).map_or_else(|| review_id.to_string(), |index| format!("r{index:02}"))
}

/// The fixture read at `now` in the synthetic Pacific zone.
fn study_time(now: &str, hour: u32) -> StudyTimeContext {
    StudyTimeContext::pacific_2026_at(millis(now), hour).expect("study time should resolve")
}

fn insert_card(
    connection: &Connection,
    index: usize,
    state: i64,
    due_at: i64,
) -> Result<(), CommandError> {
    let created = millis("2025-11-15T12:00:00-08:00");
    let (deck, tags) = CARDS[index];
    let card_id = id(3, index);
    let note_id = id(4, index);
    connection
        .execute(
            "INSERT INTO notes (
                 id, deck_id, note_type, content_format, content_schema_version,
                 front_doc, back_doc, front_text, back_text, created_at, updated_at,
                 revision, duplicate_hash
             ) VALUES (?1, ?2, 'basic', 'tiptap-json', 1, '{}', '{}', ?3, 'Answer',
                       ?4, ?4, 1, ?1)",
            params![note_id, id(1, deck), format!("Question {index}"), created],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO cards (
                 id, note_id, deck_id, direction, template_ordinal, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 'forward', 0, ?4, ?4)",
            params![card_id, note_id, id(1, deck), created],
        )
        .map_err(CommandError::database)?;
    connection
        .execute(
            "INSERT INTO card_schedules (
                 card_id, state, due_at, stability, difficulty, elapsed_days,
                 scheduled_days, reps, lapses, last_review_at, scheduler_name,
                 scheduler_version, updated_at
             ) VALUES (?1, ?2, ?3, 1.0, 5.0, 0, 0, ?4, 0, ?5, 'fsrs', 'test', ?5)",
            params![card_id, state, due_at, i64::from(state != 0), created],
        )
        .map_err(CommandError::database)?;
    for tag in tags {
        connection
            .execute(
                "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, id(2, *tag)],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn insert_decks_and_tags(connection: &Connection) -> Result<(), CommandError> {
    let created = millis("2025-11-15T12:00:00-08:00");
    for (index, name) in DECKS.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO decks (id, name, description, sort_order, created_at, updated_at, revision)
                 VALUES (?1, ?2, '', ?3, ?4, ?4, 1)",
                params![id(1, index), name, i64::try_from(index).unwrap(), created],
            )
            .map_err(CommandError::database)?;
    }
    for (index, name) in TAGS.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                 VALUES (?1, ?2, lower(?2), ?3, ?3)",
                params![id(2, index), name, created],
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

fn seed(connection: &Connection) -> Result<(), CommandError> {
    insert_decks_and_tags(connection)?;
    let now = millis(NOW);
    for index in 0..CARDS.len() {
        // Every other card is due: even cards an hour ago, odd ones tomorrow.
        let due_at = if index % 2 == 0 {
            now - 3_600_000
        } else {
            now + 86_400_000
        };
        insert_card(connection, index, if index == 3 { 0 } else { 2 }, due_at)?;
    }
    for (index, review) in REVIEWS.iter().enumerate() {
        let at = instant(review.at);
        let reviewed_at = at.timestamp_millis();
        let review_id = id(5, index);
        let (deck, tags) = CARDS[review.card];
        connection
            .execute(
                "INSERT INTO review_events (
                     id, submission_id, session_id, card_id, reviewed_at, rating,
                     answer_correct, confidence, response_time_ms, state_before,
                     state_after, due_before, due_after, stability_before, stability_after,
                     difficulty_before, difficulty_after, scheduled_days_before,
                     scheduled_days_after, scheduler_name, scheduler_version, created_at
                 ) VALUES (
                     ?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, 4000, ?8, 2, ?4, ?9,
                     1.0, 2.0, 5.0, 5.0, 0, 1, 'fsrs', 'test', ?4
                 )",
                params![
                    review_id,
                    id(6, index),
                    id(3, review.card),
                    reviewed_at,
                    review.rating,
                    i64::from(review.rating > 1),
                    [
                        "wrong",
                        "not_confident",
                        "somewhat_confident",
                        "very_confident"
                    ][usize::try_from(review.rating - 1).unwrap()],
                    if review.new { 0 } else { 2 },
                    reviewed_at + 86_400_000,
                ],
            )
            .map_err(CommandError::database)?;
        connection
            .execute(
                "INSERT INTO review_event_attributions (
                     review_event_id, reviewed_at, review_local_date, time_zone_id,
                     utc_offset_minutes, card_id, note_id, note_revision,
                     owner_deck_id_at_review, captured_deck_name, direction, card_type,
                     card_label, presentation_variant, question_excerpt,
                     question_text_hash, question_was_truncated, attribution_source
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9, 'forward', 'basic', 'Basic', '',
                     ?10, ?11, 0, 'captured'
                 )",
                params![
                    review_id,
                    reviewed_at,
                    at.date_naive().to_string(),
                    review.zone,
                    at.offset().local_minus_utc() / 60,
                    id(3, review.card),
                    id(4, review.card),
                    id(1, deck),
                    DECKS[deck],
                    format!("Question {}", review.card),
                    format!("{index:064x}"),
                ],
            )
            .map_err(CommandError::database)?;
        for tag in tags {
            connection
                .execute(
                    "INSERT INTO review_event_tag_attributions (review_event_id, tag_id, tag_name)
                     VALUES (?1, ?2, ?3)",
                    params![review_id, id(2, *tag), TAGS[*tag]],
                )
                .map_err(CommandError::database)?;
        }
    }
    Ok(())
}

fn analytics_query(range: &str) -> AnalyticsQueryDto {
    let custom = range == "custom";
    AnalyticsQueryDto {
        range: range.to_string(),
        deck_ids: Vec::new(),
        tag_ids: Vec::new(),
        custom_start_local_date: custom.then(|| CUSTOM_RANGE.0.to_string()),
        custom_end_local_date: custom.then(|| CUSTOM_RANGE.1.to_string()),
        highlighted_session_id: None,
    }
}

fn search_input(status_filter: &str, reviewed_filter: &str) -> SearchCardsInput {
    serde_json::from_value(serde_json::json!({
        "statusFilter": status_filter,
        "reviewedFilter": reviewed_filter,
    }))
    .expect("search input should parse")
}

fn matching_cards(
    connection: &Connection,
    status_filter: &str,
    reviewed_filter: &str,
    time: &StudyTimeContext,
) -> Result<BTreeSet<String>, CommandError> {
    Ok(list_matching_cards_at(
        connection,
        search_input(status_filter, reviewed_filter),
        time,
    )?
    .into_iter()
    .map(|card| card_label(&card.card_id))
    .collect())
}

fn load(
    connection: &mut Connection,
    range: &str,
    time: &StudyTimeContext,
) -> Result<AnalyticsDto, CommandError> {
    load_analytics_at(connection, analytics_query(range), time.clone())
}

fn workload_completed(
    connection: &mut Connection,
    time: &StudyTimeContext,
) -> Result<(i64, i64), CommandError> {
    let settings = load_settings(connection)?;
    let workload = workload_overview_for_scope(
        connection,
        StudyScope {
            deck_ids: &[],
            tag_ids: &[],
        },
        &settings,
        time,
    )?;
    Ok((workload.new_completed, workload.reviews_completed))
}

/// Every boundary-dependent output, one fact per line, in the format the
/// pre-change golden file was captured in.
fn summary(connection: &mut Connection, time: &StudyTimeContext) -> Result<String, CommandError> {
    let mut out = String::new();
    for range in RANGES {
        let analytics = load(connection, range, time)?;
        let scope = &analytics.scope;
        let history = &analytics.history;
        let daily = &analytics.daily_limits;
        writeln!(
            out,
            "{range}: {}..{} at {:?}..{} reviews={} cards={} correct={} today new={} review={}",
            scope.start_local_date.as_deref().unwrap_or("-"),
            scope.end_local_date,
            scope.start_at,
            scope.end_at,
            history.review_count,
            history.cards_practiced,
            history.correct_count,
            daily.new_completed,
            daily.reviews_completed,
        )
        .unwrap();
        for bucket in &analytics.timeline {
            writeln!(
                out,
                "  bucket {}..{} at {}..{} reviews={} cards={} new={}",
                bucket.start_local_date,
                bucket.end_local_date,
                bucket.start_at,
                bucket.end_at,
                bucket.review_count,
                bucket.cards_practiced,
                bucket.new_cards,
            )
            .unwrap();
        }
        for deck in &analytics.deck_stats {
            writeln!(
                out,
                "  deck {} reviews={} cards={} last={:?}",
                deck.deck_name, deck.review_count, deck.cards_practiced, deck.last_practiced_at,
            )
            .unwrap();
        }
        for tag in &analytics.tag_stats {
            writeln!(
                out,
                "  tag {} reviews={} correct={} trend={:?} status={}",
                tag.tag_name, tag.review_count, tag.correct_count, tag.trend_delta, tag.status,
            )
            .unwrap();
        }
        let recent = analytics
            .recent_reviews
            .iter()
            .map(|review| format!("{}@{}", review_label(&review.id), review.review_local_date))
            .collect::<Vec<_>>();
        writeln!(out, "  recent {}", recent.join(" ")).unwrap();
        for kind in &analytics.question_types {
            writeln!(
                out,
                "  type {} reviews={}",
                kind.question_type, kind.review_count
            )
            .unwrap();
        }
    }
    let settings = load_settings(connection)?;
    let workload = workload_overview_for_scope(
        connection,
        StudyScope {
            deck_ids: &[],
            tag_ids: &[],
        },
        &settings,
        time,
    )?;
    writeln!(
        out,
        "workload new={} review={} due_reviews={} due_new={}",
        workload.new_completed, workload.reviews_completed, workload.due_reviews, workload.due_new,
    )
    .unwrap();
    for filter in REVIEWED_FILTERS {
        let cards = matching_cards(connection, "all", filter, time)?;
        writeln!(
            out,
            "search {filter}: {}",
            cards.into_iter().collect::<Vec<_>>().join(" ")
        )
        .unwrap();
    }
    Ok(out)
}

fn with_fixture<T>(test: impl FnOnce(&mut Connection) -> Result<T, CommandError>) -> T {
    Database::open_in_memory()
        .expect("database should open")
        .with_connection(|connection| {
            seed(connection)?;
            test(connection)
        })
        .expect("study-day fixture test should run")
}

#[test]
fn a_midnight_boundary_reproduces_the_results_from_before_study_days() {
    let actual = with_fixture(|connection| summary(connection, &study_time(NOW, 0)));
    for (line, (actual, expected)) in actual.lines().zip(MIDNIGHT_GOLDEN.lines()).enumerate() {
        assert_eq!(actual, expected, "golden line {}", line + 1);
    }
    assert_eq!(
        actual.lines().count(),
        MIDNIGHT_GOLDEN.lines().count(),
        "golden line count"
    );
}

/// One review as the oracle sees it: only stored facts and SQLite's own
/// date arithmetic, nothing from the application's study calendar.
struct DatedReview {
    index: usize,
    card: usize,
    reviewed_at: i64,
    study_date: NaiveDate,
}

fn dated_reviews(connection: &Connection, hour: u32) -> Vec<DatedReview> {
    let mut statement = connection
        .prepare(
            "SELECT a.review_event_id, a.reviewed_at,
                    date(a.reviewed_at / 1000 + a.utc_offset_minutes * 60 - ?1 * 3600, 'unixepoch')
             FROM review_event_attributions a",
        )
        .unwrap();
    statement
        .query_map([hour], |row| {
            let review_id: String = row.get(0)?;
            let study_date: String = row.get(2)?;
            let index = review_index(&review_id).expect("fixture review");
            Ok(DatedReview {
                index,
                card: REVIEWS[index].card,
                reviewed_at: row.get(1)?,
                study_date: date(&study_date),
            })
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

/// The study date of `NOW`, from its own wall time and offset.
fn oracle_today(hour: u32) -> NaiveDate {
    (instant(NOW).naive_local() - Duration::hours(i64::from(hour))).date()
}

fn oracle_window(range: &str, today: NaiveDate) -> (Option<NaiveDate>, NaiveDate) {
    match range {
        "7d" => (Some(today - Duration::days(6)), today),
        "30d" => (Some(today - Duration::days(29)), today),
        "1y" => (today.with_year(today.year() - 1), today),
        "all" => (None, today),
        "custom" => (Some(date(CUSTOM_RANGE.0)), date(CUSTOM_RANGE.1)),
        _ => unreachable!("fixture range"),
    }
}

fn counts<'a>(reviews: impl Iterator<Item = &'a DatedReview>) -> (i64, i64, i64) {
    let reviews = reviews.collect::<Vec<_>>();
    let cards = reviews
        .iter()
        .map(|review| review.card)
        .collect::<BTreeSet<_>>();
    let new = reviews
        .iter()
        .filter(|review| REVIEWS[review.index].new)
        .count();
    (
        i64::try_from(reviews.len()).unwrap(),
        i64::try_from(cards.len()).unwrap(),
        i64::try_from(new).unwrap(),
    )
}

fn labels(cards: impl Iterator<Item = usize>) -> BTreeSet<String> {
    cards.map(|card| format!("c{card}")).collect()
}

#[test]
fn every_consumer_dates_reviews_like_the_stored_offset_oracle() {
    with_fixture(|connection| {
        for hour in [0, 1, 4, 12, 23] {
            let time = study_time(NOW, hour);
            let reviews = dated_reviews(connection, hour);
            let today = oracle_today(hour);
            assert_eq!(time.local_date, today, "{hour}: today");
            let earliest = reviews
                .iter()
                .map(|review| review.study_date)
                .min()
                .unwrap();

            let on_today = reviews
                .iter()
                .filter(|review| review.study_date == today)
                .collect::<Vec<_>>();
            let (today_count, _, today_new) = counts(on_today.iter().copied());
            let expected_today = (today_new, today_count - today_new);
            assert_eq!(
                workload_completed(connection, &time)?,
                expected_today,
                "{hour}"
            );

            for range in RANGES {
                let context = format!("{hour}:00 {range}");
                let analytics = load(connection, range, &time)?;
                let (start, end) = oracle_window(range, today);
                let in_window = |review: &&DatedReview| {
                    start.is_none_or(|start| review.study_date >= start) && review.study_date <= end
                };
                assert_eq!(
                    analytics.scope.start_local_date,
                    Some(start.unwrap_or(earliest).to_string()),
                    "{context}"
                );
                assert_eq!(analytics.scope.end_local_date, end.to_string(), "{context}");
                assert_eq!(
                    analytics.scope.today_local_date,
                    today.to_string(),
                    "{context}"
                );

                let (review_count, cards, _) = counts(reviews.iter().filter(in_window));
                assert_eq!(analytics.history.review_count, review_count, "{context}");
                assert_eq!(analytics.history.cards_practiced, cards, "{context}");
                assert_eq!(
                    (
                        analytics.daily_limits.new_completed,
                        analytics.daily_limits.reviews_completed
                    ),
                    expected_today,
                    "{context}"
                );

                // Buckets tile the window, and each holds its study dates' reviews.
                let first = analytics.timeline.first().expect("buckets");
                let first_date = date(&first.start_local_date);
                match start {
                    Some(start) => assert_eq!(first_date, start, "{context}"),
                    None => assert_eq!(first_date, earliest.with_day(1).unwrap(), "{context}"),
                }
                let mut next = first_date;
                for bucket in &analytics.timeline {
                    let (bucket_start, bucket_end) =
                        (date(&bucket.start_local_date), date(&bucket.end_local_date));
                    assert_eq!(bucket_start, next, "{context}: contiguous");
                    next = bucket_end + Duration::days(1);
                    let (reviews_in, cards_in, new_in) =
                        counts(reviews.iter().filter(|review| {
                            (bucket_start..=bucket_end).contains(&review.study_date)
                        }));
                    assert_eq!(
                        (
                            bucket.review_count,
                            bucket.cards_practiced,
                            bucket.new_cards
                        ),
                        (reviews_in, cards_in, new_in),
                        "{context}: bucket {bucket_start}"
                    );
                }
                assert_eq!(next, end + Duration::days(1), "{context}: last bucket");

                let deck_reviews = analytics
                    .deck_stats
                    .iter()
                    .map(|deck| (deck.deck_name.clone(), deck.review_count))
                    .collect::<BTreeMap<_, _>>();
                let tag_reviews = analytics
                    .tag_stats
                    .iter()
                    .map(|tag| (tag.tag_name.clone(), tag.review_count))
                    .collect::<BTreeMap<_, _>>();
                for (deck, name) in DECKS.iter().enumerate() {
                    let expected = reviews
                        .iter()
                        .filter(in_window)
                        .filter(|review| CARDS[review.card].0 == deck)
                        .count();
                    assert_eq!(
                        deck_reviews.get(*name).copied().unwrap_or_default(),
                        i64::try_from(expected).unwrap(),
                        "{context}: deck {name}"
                    );
                }
                for (tag, name) in TAGS.iter().enumerate() {
                    let expected = reviews
                        .iter()
                        .filter(in_window)
                        .filter(|review| CARDS[review.card].1.contains(&tag))
                        .count();
                    assert_eq!(
                        tag_reviews.get(*name).copied().unwrap_or_default(),
                        i64::try_from(expected).unwrap(),
                        "{context}: tag {name}"
                    );
                }

                let mut recent = reviews.iter().filter(in_window).collect::<Vec<_>>();
                recent.sort_by_key(|review| std::cmp::Reverse(review.reviewed_at));
                let expected_recent = recent
                    .iter()
                    .take(20)
                    .map(|review| (id(5, review.index), review.study_date.to_string()))
                    .collect::<Vec<_>>();
                let actual_recent = analytics
                    .recent_reviews
                    .iter()
                    .map(|review| (review.id.clone(), review.review_local_date.clone()))
                    .collect::<Vec<_>>();
                assert_eq!(actual_recent, expected_recent, "{context}: recent");
            }

            let reviewed_since = |first: NaiveDate| {
                labels(
                    reviews
                        .iter()
                        .filter(|review| review.study_date >= first)
                        .map(|review| review.card),
                )
            };
            let all_reviewed = labels(reviews.iter().map(|review| review.card));
            let past_30 = reviewed_since(today - Duration::days(29));
            let expected = [
                ("today", labels(on_today.iter().map(|review| review.card))),
                (
                    "past_7_local_dates",
                    reviewed_since(today - Duration::days(6)),
                ),
                ("past_30_local_dates", past_30.clone()),
                (
                    "before_past_30_local_dates",
                    all_reviewed.difference(&past_30).cloned().collect(),
                ),
                ("never", labels(std::iter::once(3))),
            ];
            for (filter, cards) in expected {
                assert_eq!(
                    matching_cards(connection, "all", filter, &time)?,
                    cards,
                    "{hour}:00 search {filter}"
                );
            }
        }
        Ok(())
    });
}

#[test]
fn reviews_before_four_count_toward_the_previous_study_day() {
    with_fixture(|connection| {
        // 12:00 on 3 November: the 00:30, 00:45 and 03:30 reviews belong to
        // 2 November; the 04:00 and 11:00 ones are today's.
        let noon = study_time(NOW, 4);
        let analytics = load(connection, "7d", &noon)?;
        assert_eq!(analytics.daily_limits.new_completed, 0);
        assert_eq!(analytics.daily_limits.reviews_completed, 2);
        assert_eq!(workload_completed(connection, &noon)?, (0, 2));
        let buckets = analytics
            .timeline
            .iter()
            .map(|bucket| {
                (
                    bucket.start_local_date.as_str(),
                    (
                        bucket.review_count,
                        bucket.cards_practiced,
                        bucket.new_cards,
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        // 04:00 on 2 November, then the three after midnight.
        assert_eq!(buckets["2026-11-02"], (4, 3, 1));
        assert_eq!(buckets["2026-11-03"], (2, 2, 0));
        // 04:30 on 1 November and 03:59 on 2 November.
        assert_eq!(buckets["2026-11-01"], (2, 2, 1));
        assert_eq!(
            matching_cards(connection, "all", "today", &noon)?,
            labels([4, 5].into_iter())
        );

        // At 03:59 it is still 2 November.
        let late = study_time("2026-11-03T03:59:00-08:00", 4);
        assert_eq!(late.local_date, date("2026-11-02"));
        let analytics = load(connection, "7d", &late)?;
        assert_eq!(
            (
                analytics.daily_limits.new_completed,
                analytics.daily_limits.reviews_completed
            ),
            (1, 3)
        );
        assert_eq!(workload_completed(connection, &late)?, (1, 3));
        assert_eq!(
            matching_cards(connection, "all", "today", &late)?,
            labels([0, 1, 7].into_iter())
        );

        // At 04:00 the new day has begun: of the fixture's reviews, only the
        // 04:00 and 11:00 ones belong to it.
        let four = study_time("2026-11-03T04:00:00-08:00", 4);
        assert_eq!(four.local_date, date("2026-11-03"));
        assert_eq!(four.today_start_utc_millis, four.now_utc_millis);
        assert_eq!(workload_completed(connection, &four)?, (0, 2));
        assert_eq!(
            matching_cards(connection, "all", "today", &four)?,
            labels([4, 5].into_iter())
        );
        Ok(())
    });
}

#[test]
fn study_days_follow_daylight_saving_and_each_review_keeps_its_own_offset() {
    with_fixture(|connection| {
        let time = study_time(NOW, 4);
        let bucket = |analytics: &AnalyticsDto, day: &str| {
            analytics
                .timeline
                .iter()
                .find(|bucket| bucket.start_local_date == day)
                .map(|bucket| (bucket.start_at, bucket.end_at, bucket.review_count))
                .expect("bucket")
        };

        // Spring forward: 7 March runs from 04:00 PST to 04:00 PDT, 23 hours,
        // and holds the 23:30, 01:30 PST and 03:30 PDT reviews.
        let spring = load(connection, "custom", &time)?;
        assert_eq!(
            bucket(&spring, "2026-03-07"),
            (
                millis("2026-03-07T04:00:00-08:00"),
                millis("2026-03-08T04:00:00-07:00"),
                3
            )
        );
        // 04:30 on 8 March and 00:30 on 9 March.
        assert_eq!(bucket(&spring, "2026-03-08").2, 2);

        // Fall back: 31 October runs from 04:00 PDT to 04:00 PST, 25 hours,
        // and holds 23:30, 00:30 PDT and both 01:30s and 03:30 PST.
        let autumn = load(connection, "30d", &time)?;
        assert_eq!(
            bucket(&autumn, "2026-10-31"),
            (
                millis("2026-10-31T04:00:00-07:00"),
                millis("2026-11-01T04:00:00-08:00"),
                5
            )
        );

        // 05:00 in Tokyo on 20 October was 13:00 on the 19th in Los Angeles;
        // the review keeps its own offset and its Tokyo date.
        assert_eq!(bucket(&autumn, "2026-10-20").2, 1);
        assert_eq!(bucket(&autumn, "2026-10-19").2, 0);
        Ok(())
    });
}

#[test]
fn burial_at_two_resurfaces_at_four_the_same_calendar_day() {
    with_fixture(|connection| {
        let two = study_time("2026-11-03T02:00:00-08:00", 4);
        let action =
            |mutation: usize, action: &str, intent: Option<RescheduleIntent>| ScheduleActionInput {
                mutation_id: id(7, mutation),
                card_ids: vec![id(3, 0)],
                action: action.to_string(),
                reschedule_intent: intent,
                session_id: None,
            };
        apply_at(connection, action(0, "bury", None), &two)?;
        let buried_until: i64 = connection
            .query_row(
                "SELECT buried_until FROM cards WHERE id = ?1",
                [id(3, 0)],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        assert_eq!(buried_until, millis("2026-11-03T04:00:00-08:00"));
        assert_eq!(
            matching_cards(
                connection,
                "buried",
                "all",
                &study_time("2026-11-03T03:59:00-08:00", 4)
            )?,
            labels([0].into_iter())
        );
        assert!(matching_cards(
            connection,
            "buried",
            "all",
            &study_time("2026-11-03T04:00:00-08:00", 4)
        )?
        .is_empty());

        // "Tomorrow" is the same 04:00, and a date lands at its 04:00.
        let tomorrow = apply_at(
            connection,
            action(1, "reschedule", Some(RescheduleIntent::Tomorrow)),
            &two,
        )?;
        assert_eq!(
            tomorrow.resolved_due_at,
            Some(millis("2026-11-03T04:00:00-08:00"))
        );
        let dated = apply_at(
            connection,
            action(
                2,
                "reschedule",
                Some(RescheduleIntent::CalendarDate {
                    date: "2026-11-05".to_string(),
                }),
            ),
            &two,
        )?;
        assert_eq!(
            dated.resolved_due_at,
            Some(millis("2026-11-05T04:00:00-08:00"))
        );

        // With midnight days, the same burial lasts until the next midnight.
        let midnight = study_time("2026-11-03T02:00:00-08:00", 0);
        apply_at(connection, action(3, "bury", None), &midnight)?;
        let buried_until: i64 = connection
            .query_row(
                "SELECT buried_until FROM cards WHERE id = ?1",
                [id(3, 0)],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        assert_eq!(buried_until, millis("2026-11-04T00:00:00-08:00"));
        Ok(())
    });
}

#[test]
fn practice_order_groups_due_cards_by_study_date() {
    Database::open_in_memory()
        .expect("database should open")
        .with_connection(|connection| {
            insert_decks_and_tags(connection)?;
            // Card 0 is new and fell due late on 2 November; card 1 is a
            // review that fell due at 00:30 on 3 November.
            insert_card(connection, 0, 0, millis("2026-11-02T22:00:00-08:00"))?;
            insert_card(connection, 1, 2, millis("2026-11-03T00:30:00-08:00"))?;
            let order = |hour| -> Result<Vec<String>, CommandError> {
                Ok(planned_extra_cards_for_scope(
                    connection,
                    StudyScope {
                        deck_ids: &[],
                        tag_ids: &[],
                    },
                    &study_time(NOW, hour),
                    "study-day-order",
                )?
                .into_iter()
                .map(|card| card_label(&card.card_id))
                .collect())
            };
            // Midnight: different due dates, the earlier one first.
            assert_eq!(order(0)?, ["c0", "c1"]);
            // 4 a.m.: both fell due on 2 November, so reviews precede new cards.
            assert_eq!(order(4)?, ["c1", "c0"]);
            Ok(())
        })
        .expect("practice order should load");
}
