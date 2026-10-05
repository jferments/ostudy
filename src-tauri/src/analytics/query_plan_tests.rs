//! Plan and equivalence checks for the analytics history queries.
//!
//! History queries pin their loop order with `CROSS JOIN`, and the inventory,
//! deck and tag queries read one per-request learning-signal table. These tests
//! hold both changes to the SQL they replaced: every statement must return the
//! same rows as before, and a dated window must start from the local-date index
//! on a library without planner statistics.

use std::sync::{Mutex, MutexGuard, PoisonError};

use chrono::Duration;
use rusqlite::{
    params,
    trace::{TraceEvent, TraceEventCodes},
    types::Value,
    Connection,
};

use super::{
    load_analytics,
    scope::{HISTORY_BY_DECK_SQL, HISTORY_BY_LOCAL_DATE_SQL},
    signals::{self, RequestSignals},
    AnalyticsQueryDto,
};
use crate::{
    database::Database, error::CommandError, projections::learning_signals as signal_projection,
    study_calendar::StudyTimeContext,
};

/// The history joins every analytics query used before the order was pinned,
/// counting a merged collection copy's reviews under its original.
/// (The session query listed `practice_sessions` first; inner joins commute.)
const PREVIOUS_HISTORY_FROM_SQL: &str = "review_events r
         JOIN review_event_attributions a ON a.review_event_id = r.id
         LEFT JOIN card_merges merged ON merged.retired_card_id = r.card_id
         JOIN cards c ON c.id = COALESCE(merged.surviving_card_id, r.card_id)
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id";

/// History, timeline, decks, tags, recent reviews, sessions, question types.
const HISTORY_STATEMENTS_PER_REQUEST: usize = 7;
/// Current inventory, decks, tags.
const SIGNAL_STATEMENTS_PER_REQUEST: usize = 3;
const DAY_MS: i64 = 86_400_000;

/// The trace callback is a plain function, so captures go through a static;
/// tests that trace hold `CAPTURE_LOCK` for their whole run.
static CAPTURE_LOCK: Mutex<()> = Mutex::new(());
static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A failed test must not hide the others' own failures behind a poisoned lock.
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

/// Runs analytics and returns the read statements it executed, values bound.
fn traced_reads(connection: &mut Connection, query: AnalyticsQueryDto) -> Vec<String> {
    lock(&CAPTURED).clear();
    connection.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(capture_statement));
    let loaded = load_analytics(connection, query);
    connection.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, None);
    loaded.expect("analytics should load");
    std::mem::take(&mut *lock(&CAPTURED))
        .into_iter()
        .filter(|sql| sql.starts_with("SELECT") || sql.starts_with("WITH"))
        .collect()
}

/// The statement as the loaders wrote it before this change: history joins in
/// their original unpinned order, and the learning signal recomputed as a CTE
/// inside every statement that reads it.
fn previous_sql(sql: &str) -> String {
    let mut previous = sql
        .replace(HISTORY_BY_LOCAL_DATE_SQL, PREVIOUS_HISTORY_FROM_SQL)
        .replace(HISTORY_BY_DECK_SQL, PREVIOUS_HISTORY_FROM_SQL)
        .replace("CROSS JOIN", "JOIN");
    if previous.contains(signals::JOIN_SQL) {
        previous = previous.replace(signals::JOIN_SQL, signal_projection::join_sql());
        let ctes = signal_projection::LEARNING_SIGNAL_CTES;
        previous = match previous.strip_prefix("WITH ") {
            Some(rest) => format!("WITH {ctes},\n{rest}"),
            None => format!("WITH {ctes}\n{previous}"),
        };
    }
    previous
}

/// How to compare a statement's output where SQL never fixed an order.
#[derive(Clone, Copy)]
struct Unordered {
    /// The tag statement has no `ORDER BY`; Rust sorts its rows afterwards.
    rows: bool,
    /// A session's deck names come from `json_group_array` in scan order,
    /// which was never specified, so they are compared as a set.
    session_deck_names: bool,
}

impl Unordered {
    /// Decided from the current statement, whose SQL no longer carries the
    /// learning-signal window function, and applied to both sides.
    fn of(sql: &str) -> Self {
        Self {
            rows: !sql.contains("ORDER BY"),
            session_deck_names: sql.contains("json_group_array(DISTINCT d.name)"),
        }
    }
}

fn rows(connection: &Connection, sql: &str, unordered: Unordered) -> Vec<Vec<Value>> {
    let mut statement = connection.prepare(sql).unwrap();
    let width = statement.column_count();
    let mut rows = statement
        .query_map([], |row| {
            (0..width)
                .map(|index| row.get::<_, Value>(index))
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    if unordered.session_deck_names {
        for row in &mut rows {
            if let Value::Text(names) = &row[2] {
                let mut names: Vec<String> = serde_json::from_str(names).unwrap();
                names.sort();
                row[2] = Value::Text(serde_json::to_string(&names).unwrap());
            }
        }
    }
    if unordered.rows {
        rows.sort_by_key(|row| format!("{row:?}"));
    }
    rows
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

/// Positions of the attribution (`a`) and review (`r`) loops in a plan.
fn history_loops(plan: &[String]) -> (usize, usize) {
    let position = |alias: &str| {
        plan.iter()
            .position(|line| {
                line.starts_with(&format!("SEARCH {alias} "))
                    || line.starts_with(&format!("SCAN {alias} "))
                    || *line == format!("SCAN {alias}")
            })
            .unwrap_or_else(|| panic!("plan has no `{alias}` loop: {plan:#?}"))
    };
    (position("a"), position("r"))
}

/// Whether the plan searches `alias` with an index lookup on `key`.
fn searches(plan: &[String], alias: &str, key: &str) -> bool {
    plan.iter()
        .any(|line| line.starts_with(&format!("SEARCH {alias} ")) && line.contains(key))
}

fn assert_starts_from_local_date_index(plan: &[String]) {
    let (attribution, review) = history_loops(plan);
    assert!(
        attribution < review
            && plan[attribution].contains("INDEX review_event_attributions_local_date_idx")
            && plan[review].contains("sqlite_autoindex_review_events_1 (id=?)")
            && searches(plan, "merged", "(retired_card_id=?)")
            && searches(plan, "c", "sqlite_autoindex_cards_1 (id=?)"),
        "a dated window must scan the local-date index, then look rows up by key: {plan:#?}"
    );
}

fn assert_walks_reviews_by_card(plan: &[String]) {
    let (attribution, review) = history_loops(plan);
    assert!(
        review < attribution
            && plan[review].contains("(card_id=?)")
            && plan[attribution].contains("(review_event_id=?)")
            && searches(plan, "retired", "(retired_card_id=?)")
            && searches(plan, "merged", "(surviving_card_id=?)"),
        "selected decks and all time must walk each card's own and merged reviews: {plan:#?}"
    );
}

fn statistics_present(connection: &Connection) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'sqlite_stat1')",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
        != 0
}

fn signal_table_exists(connection: &Connection) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_temp_master WHERE name = 'analytics_learning_signal'
             )",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
        != 0
}

fn hex_id(kind: u64, index: u64) -> String {
    format!("{kind:08x}{index:024x}")
}

/// Small deterministic generator so the fixture is identical on every run.
struct Sequence(u64);

impl Sequence {
    fn below(&mut self, bound: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % bound
    }
}

struct Fixture {
    decks: Vec<String>,
    tags: Vec<String>,
    historical_tags: Vec<String>,
    sessions: Vec<String>,
}

const DECK_NAMES: [&str; 6] = ["Alpha", "beta", "Gamma", "Delta", "Epsilon", "Zeta"];
const ARCHIVED_DECK: usize = 2;
const DELETED_DECK: usize = 3;
const CARD_TYPES: [&str; 6] = [
    "basic",
    "cloze",
    "single_choice",
    "matching",
    "ordering",
    "image_occlusion",
];

/// A library with archived and deleted decks, deleted notes and cards, moved
/// cards, current and historical-only tags, sessions spanning several decks,
/// resets, checked and self-rated questions, backfilled attributions, ties in
/// review time, reviews spread over two years plus one future local date, and
/// reviewed collection copies merged into their originals.
fn seed(connection: &Connection, time: &StudyTimeContext) -> Fixture {
    let now = time.now_utc_millis;
    let mut sequence = Sequence(0x9E37_79B9_7F4A_7C15);
    let decks: Vec<String> = (0..6).map(|index| hex_id(1, index)).collect();
    for (index, deck_id) in decks.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO decks (
                     id, name, description, sort_order, created_at, updated_at,
                     archived_at, deleted_at, revision
                 ) VALUES (?1, ?2, '', ?3, ?4, ?4, ?5, ?6, 1)",
                params![
                    deck_id,
                    DECK_NAMES[index],
                    i64::try_from(index).unwrap(),
                    now - 900 * DAY_MS,
                    (index == ARCHIVED_DECK).then_some(now - DAY_MS),
                    (index == DELETED_DECK).then_some(now - DAY_MS),
                ],
            )
            .unwrap();
    }
    let tags: Vec<String> = (0..4).map(|index| hex_id(2, index)).collect();
    for (index, tag_id) in tags.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO tags (id, name, normalized_name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![
                    tag_id,
                    format!("Topic {index}"),
                    format!("topic {index}"),
                    now
                ],
            )
            .unwrap();
    }
    let historical_tags: Vec<String> = (0..2).map(|index| hex_id(3, index)).collect();

    // (card id, note id, deck index, direction)
    let mut cards = Vec::<(String, String, usize, &str)>::new();
    for note_index in 0..48_u64 {
        let note_id = hex_id(4, note_index);
        let deck = usize::try_from(note_index % 5).unwrap();
        connection
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, created_at, updated_at,
                     deleted_at, revision, duplicate_hash
                 ) VALUES (
                     ?1, ?2, 'basic', 'tiptap-json', 1, '{}', '{}', ?3, 'Answer',
                     ?4, ?4, ?5, 1, ?1
                 )",
                params![
                    note_id,
                    decks[deck],
                    format!("Question {note_index}"),
                    now - 900 * DAY_MS,
                    (note_index % 11 == 10).then_some(now - DAY_MS),
                ],
            )
            .unwrap();
        for tag in [note_index % 4, (note_index + 1) % 4] {
            if tag == note_index % 4 || note_index % 3 == 0 {
                connection
                    .execute(
                        "INSERT INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                        params![note_id, tags[usize::try_from(tag).unwrap()]],
                    )
                    .unwrap();
            }
        }
        let directions: &[&str] = if note_index % 3 == 0 {
            &["forward", "reverse"]
        } else {
            &["forward"]
        };
        for direction in directions {
            let card_index = u64::try_from(cards.len()).unwrap();
            let card_id = hex_id(5, card_index);
            connection
                .execute(
                    "INSERT INTO cards (
                         id, note_id, deck_id, direction, template_ordinal, created_at,
                         updated_at, suspended_at, deleted_at, buried_until
                     ) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5, ?6, ?7, ?8)",
                    params![
                        card_id,
                        note_id,
                        decks[deck],
                        direction,
                        now - 900 * DAY_MS,
                        (card_index % 7 == 6).then_some(now - DAY_MS),
                        (card_index % 13 == 12).then_some(now - DAY_MS),
                        (card_index % 9 == 8).then_some(now + DAY_MS),
                    ],
                )
                .unwrap();
            let offset = i64::try_from(card_index % 7).unwrap() - 3;
            connection
                .execute(
                    "INSERT INTO card_schedules (
                         card_id, state, due_at, stability, difficulty, elapsed_days,
                         scheduled_days, reps, lapses, last_review_at, scheduler_name,
                         scheduler_version, updated_at
                     ) VALUES (?1, ?2, ?3, 1.0, 5.0, 0, 0, 0, 0, ?4, 'fsrs', 'test', ?4)",
                    params![
                        card_id,
                        i64::try_from(card_index % 4).unwrap(),
                        now + offset * DAY_MS,
                        now - DAY_MS,
                    ],
                )
                .unwrap();
            cards.push((card_id, note_id.clone(), deck, direction));
        }
    }

    let sessions: Vec<String> = (0..12).map(|index| hex_id(6, index)).collect();
    for (index, session_id) in sessions.iter().enumerate() {
        let index = i64::try_from(index).unwrap();
        connection
            .execute(
                "INSERT INTO practice_sessions (
                     id, selected_deck_ids_json, selected_tag_ids_json, shuffle, card_count,
                     mode, started_at, completed_at, rated_count, correct_count
                 ) VALUES (?1, '[]', '[]', 1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    session_id,
                    10 + index,
                    if index % 3 == 0 { "extra" } else { "scheduled" },
                    now - (index * 40 + 1) * DAY_MS,
                    (index % 4 != 0).then_some(now - index * 40 * DAY_MS),
                    5 + index,
                    3 + index % 4,
                ],
            )
            .unwrap();
    }

    // A reset starts a new learning epoch for two cards.
    connection
        .execute(
            "INSERT INTO schedule_action_operations (
                 id, action, requested_count, changed_count, unchanged_count,
                 rejected_count, occurred_at, created_at
             ) VALUES (?1, 'reset', 2, 2, 0, 0, ?2, ?2)",
            params![hex_id(7, 0), now - 100 * DAY_MS],
        )
        .unwrap();
    for (index, (card_id, ..)) in cards.iter().take(2).enumerate() {
        connection
            .execute(
                "INSERT INTO schedule_action_events (
                     id, operation_id, card_id, action, due_before, due_after,
                     state_before, state_after, schedule_revision_before,
                     schedule_revision_after, occurred_at, created_at
                 ) VALUES (?1, ?2, ?3, 'reset', ?4, ?4, 2, 0, 1, 2, ?4, ?4)",
                params![
                    hex_id(8, u64::try_from(index).unwrap()),
                    hex_id(7, 0),
                    card_id,
                    now - 100 * DAY_MS,
                ],
            )
            .unwrap();
    }

    let card_count = u64::try_from(cards.len()).unwrap();
    let mut previous_reviewed_at = now;
    for review_index in 0..900_u64 {
        let review_id = hex_id(9, review_index);
        let (card_id, note_id, deck, direction) =
            &cards[usize::try_from(sequence.below(card_count)).unwrap()];
        let days_ago = match review_index {
            0..=2 => -1,
            _ if review_index % 3 == 0 => i64::try_from(sequence.below(10)).unwrap(),
            _ => i64::try_from(sequence.below(800)).unwrap(),
        };
        let reviewed_at = if review_index % 50 == 49 {
            previous_reviewed_at
        } else {
            now - days_ago * DAY_MS - i64::try_from(sequence.below(80_000_000)).unwrap()
        };
        previous_reviewed_at = reviewed_at;
        let rating = i64::try_from(1 + sequence.below(4)).unwrap();
        let answer_correct = i64::from(rating > 1 && review_index % 17 != 0);
        let confidence = [
            "wrong",
            "not_confident",
            "somewhat_confident",
            "very_confident",
        ][usize::try_from(rating - 1).unwrap()];
        let session = (review_index % 5 != 0)
            .then(|| &sessions[usize::try_from(sequence.below(12)).unwrap()]);
        connection
            .execute(
                "INSERT INTO review_events (
                     id, submission_id, session_id, card_id, reviewed_at, rating,
                     answer_correct, confidence, response_time_ms, state_before,
                     state_after, due_before, due_after, stability_before, stability_after,
                     difficulty_before, difficulty_after, scheduled_days_before,
                     scheduled_days_after, scheduler_name, scheduler_version, created_at
                 ) VALUES (
                     ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12,
                     1.0, 2.0, 5.0, 5.0, 0, 1, 'fsrs', 'test', ?13
                 )",
                params![
                    review_id,
                    hex_id(10, review_index),
                    session,
                    card_id,
                    reviewed_at,
                    rating,
                    answer_correct,
                    confidence,
                    i64::try_from(sequence.below(120_000)).unwrap(),
                    i64::try_from(sequence.below(4)).unwrap(),
                    i64::try_from(sequence.below(4)).unwrap(),
                    reviewed_at + DAY_MS,
                    reviewed_at + i64::try_from(review_index % 3).unwrap(),
                ],
            )
            .unwrap();
        // Some cards moved since review: the captured owner differs.
        let captured_deck = if review_index % 7 == 0 {
            (deck + 1) % 5
        } else {
            *deck
        };
        let card_type = CARD_TYPES[usize::try_from(review_index % 6).unwrap()];
        connection
            .execute(
                "INSERT INTO review_event_attributions (
                     review_event_id, reviewed_at, review_local_date, time_zone_id,
                     utc_offset_minutes, card_id, note_id, note_revision,
                     owner_deck_id_at_review, captured_deck_name, direction, card_type,
                     card_label, presentation_variant, question_excerpt,
                     question_text_hash, question_was_truncated, attribution_source
                 ) VALUES (
                     ?1, ?2, ?3, 'UTC', 0, ?4, ?5, 1, ?6, ?7, ?8, ?9, 'Basic', '', ?10,
                     ?11, 0, ?12
                 )",
                params![
                    review_id,
                    reviewed_at,
                    (time.local_date - Duration::days(days_ago)).to_string(),
                    card_id,
                    note_id,
                    decks[captured_deck],
                    DECK_NAMES[captured_deck],
                    direction,
                    card_type,
                    format!("Question {review_index}"),
                    format!("{review_index:064x}"),
                    if review_index % 11 == 0 {
                        "legacy_current_state_backfill"
                    } else {
                        "captured"
                    },
                ],
            )
            .unwrap();
        for slot in 0..sequence.below(3) {
            let (tag_id, tag_name) = if (review_index + slot) % 5 == 4 {
                let index = usize::try_from(slot % 2).unwrap();
                (&historical_tags[index], format!("Retired {index}"))
            } else {
                let index = usize::try_from((review_index + slot) % 4).unwrap();
                (&tags[index], format!("Topic {index}"))
            };
            connection
                .execute(
                    "INSERT OR IGNORE INTO review_event_tag_attributions (
                         review_event_id, tag_id, tag_name
                     ) VALUES (?1, ?2, ?3)",
                    params![review_id, tag_id, tag_name],
                )
                .unwrap();
        }
        if card_type != "basic" && card_type != "cloze" {
            let revealed = card_type == "image_occlusion";
            let phase = if revealed { "revealed" } else { "checked" };
            let assisted = i64::from(review_index % 4 == 0);
            let verdict = (!revealed).then(|| {
                ["correct", "incorrect", "incomplete"][usize::try_from(review_index % 3).unwrap()]
            });
            let instance_id = hex_id(11, review_index);
            connection
                .execute(
                    "INSERT INTO question_attempts (
                         instance_id, card_id, note_id, question_kind, instance_hash,
                         revision, phase, assisted, checkpoint_json, first_checked_at,
                         verdict, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10, ?9, ?9)",
                    params![
                        instance_id,
                        card_id,
                        note_id,
                        card_type,
                        format!("{review_index:064x}"),
                        phase,
                        assisted,
                        format!(
                            r#"{{"revision":1,"assisted":{assisted},"state":{{"phase":"{phase}"}}}}"#
                        ),
                        reviewed_at,
                        verdict,
                    ],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO question_review_links (review_event_id, instance_id, override_reason)
                     VALUES (?1, ?2, ?3)",
                    params![
                        review_id,
                        instance_id,
                        (review_index % 8 == 0).then_some("Deliberate override"),
                    ],
                )
                .unwrap();
        }
    }
    let fixture = Fixture {
        decks,
        tags,
        historical_tags,
        sessions,
    };
    seed_merged_copies(connection, time, &fixture, &cards, &mut sequence);
    fixture
}

/// Collection copies that release schema 6 retired into six of the cards,
/// each with reviews filed under the copy in the otherwise unused last deck.
/// The last copy is live again, which the schema allows; its reviews still
/// count only under its original.
fn seed_merged_copies(
    connection: &Connection,
    time: &StudyTimeContext,
    fixture: &Fixture,
    cards: &[(String, String, usize, &str)],
    sequence: &mut Sequence,
) {
    let now = time.now_utc_millis;
    let copy_deck = DECK_NAMES.len() - 1;
    for (copy_index, (card_id, note_id, _, direction)) in
        cards.iter().step_by(5).take(6).enumerate()
    {
        let copy = u64::try_from(copy_index).unwrap();
        let copy_note_id = hex_id(12, copy);
        let copy_card_id = hex_id(13, copy);
        let retired_at = (copy_index < 5).then_some(now - 200 * DAY_MS);
        connection
            .execute(
                "INSERT INTO notes (
                     id, deck_id, note_type, content_format, content_schema_version,
                     front_doc, back_doc, front_text, back_text, created_at, updated_at,
                     deleted_at, revision, duplicate_hash
                 ) VALUES (
                     ?1, ?2, 'basic', 'tiptap-json', 1, '{}', '{}', ?3, 'Answer',
                     ?4, ?4, ?5, 1, ?1
                 )",
                params![
                    copy_note_id,
                    fixture.decks[copy_deck],
                    format!("Copy {copy}"),
                    now - 600 * DAY_MS,
                    retired_at,
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO cards (
                     id, note_id, deck_id, direction, template_ordinal, created_at,
                     updated_at, deleted_at
                 ) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5, ?6)",
                params![
                    copy_card_id,
                    copy_note_id,
                    fixture.decks[copy_deck],
                    direction,
                    now - 600 * DAY_MS,
                    retired_at,
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO card_merges (
                     retired_card_id, surviving_card_id, retired_note_id,
                     surviving_note_id, reason, copied_at
                 ) VALUES (?1, ?2, ?3, ?4, 'collection_copy', ?5)",
                params![
                    copy_card_id,
                    card_id,
                    copy_note_id,
                    note_id,
                    now - 600 * DAY_MS
                ],
            )
            .unwrap();
        for (slot, days_ago) in [0_i64, 4, 45, 300].into_iter().enumerate() {
            let review = copy * 4 + u64::try_from(slot).unwrap();
            let review_id = hex_id(14, review);
            let days_ago = days_ago + i64::try_from(copy).unwrap();
            let reviewed_at =
                now - days_ago * DAY_MS - i64::try_from(sequence.below(80_000_000)).unwrap();
            let rating = i64::try_from(1 + review % 4).unwrap();
            let confidence = [
                "wrong",
                "not_confident",
                "somewhat_confident",
                "very_confident",
            ][usize::try_from(rating - 1).unwrap()];
            let session =
                (review % 2 == 0).then(|| &fixture.sessions[usize::try_from(review % 12).unwrap()]);
            connection
                .execute(
                    "INSERT INTO review_events (
                         id, submission_id, session_id, card_id, reviewed_at, rating,
                         answer_correct, confidence, response_time_ms, state_before,
                         state_after, due_before, due_after, stability_before, stability_after,
                         difficulty_before, difficulty_after, scheduled_days_before,
                         scheduled_days_after, scheduler_name, scheduler_version, created_at
                     ) VALUES (
                         ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 2, 2, ?10, ?10,
                         1.0, 2.0, 5.0, 5.0, 0, 1, 'fsrs', 'test', ?5
                     )",
                    params![
                        review_id,
                        hex_id(15, review),
                        session,
                        copy_card_id,
                        reviewed_at,
                        rating,
                        i64::from(rating > 1),
                        confidence,
                        i64::try_from(sequence.below(120_000)).unwrap(),
                        reviewed_at + DAY_MS,
                    ],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO review_event_attributions (
                         review_event_id, reviewed_at, review_local_date, time_zone_id,
                         utc_offset_minutes, card_id, note_id, note_revision,
                         owner_deck_id_at_review, captured_deck_name, direction, card_type,
                         card_label, presentation_variant, question_excerpt,
                         question_text_hash, question_was_truncated, attribution_source
                     ) VALUES (
                         ?1, ?2, ?3, 'UTC', 0, ?4, ?5, 1, ?6, ?7, ?8, 'basic', 'Basic', '',
                         ?9, ?10, 0, 'captured'
                     )",
                    params![
                        review_id,
                        reviewed_at,
                        (time.local_date - Duration::days(days_ago)).to_string(),
                        copy_card_id,
                        copy_note_id,
                        fixture.decks[copy_deck],
                        DECK_NAMES[copy_deck],
                        direction,
                        format!("Copy {copy}"),
                        format!("{:064x}", 10_000 + review),
                    ],
                )
                .unwrap();
            if review % 3 == 0 {
                connection
                    .execute(
                        "INSERT INTO review_event_tag_attributions (
                             review_event_id, tag_id, tag_name
                         ) VALUES (?1, ?2, 'Topic 1')",
                        params![review_id, fixture.tags[1]],
                    )
                    .unwrap();
            }
        }
    }
}

fn query(
    range: &str,
    deck_ids: &[&String],
    tag_ids: &[&String],
    highlighted_session_id: Option<&String>,
    time: &StudyTimeContext,
) -> AnalyticsQueryDto {
    let custom = range == "custom";
    AnalyticsQueryDto {
        range: range.to_string(),
        deck_ids: deck_ids.iter().map(|id| (*id).clone()).collect(),
        tag_ids: tag_ids.iter().map(|id| (*id).clone()).collect(),
        custom_start_local_date: custom
            .then(|| (time.local_date - Duration::days(400)).to_string()),
        custom_end_local_date: custom.then(|| (time.local_date - Duration::days(3)).to_string()),
        highlighted_session_id: highlighted_session_id.cloned(),
    }
}

#[test]
fn dated_history_queries_start_from_the_local_date_index_without_statistics() {
    let _serial = lock(&CAPTURE_LOCK);
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let time = StudyTimeContext::capture()?;
            let fixture = seed(connection, &time);
            assert!(!statistics_present(connection));
            let mut checked = 0;
            for range in ["7d", "30d", "90d", "1y", "custom"] {
                for tag_ids in [vec![], vec![&fixture.tags[1]]] {
                    let reads = traced_reads(
                        connection,
                        query(range, &[], &tag_ids, Some(&fixture.sessions[2]), &time),
                    );
                    // The deck and tag statements join the request table.
                    let signals = RequestSignals::materialize(connection)?;
                    let dated: Vec<&String> = reads
                        .iter()
                        .filter(|sql| sql.contains(HISTORY_BY_LOCAL_DATE_SQL))
                        .collect();
                    assert_eq!(dated.len(), HISTORY_STATEMENTS_PER_REQUEST, "{range}");
                    for sql in dated {
                        assert_starts_from_local_date_index(&plan(connection, sql));
                        checked += 1;
                    }
                    drop(signals);
                }
            }
            assert_eq!(checked, 5 * 2 * HISTORY_STATEMENTS_PER_REQUEST);

            // Selected decks and all time keep walking each card's reviews.
            for (range, deck_ids) in [
                ("7d", vec![&fixture.decks[0]]),
                ("all", vec![]),
                (
                    "all",
                    vec![&fixture.decks[1], &fixture.decks[ARCHIVED_DECK]],
                ),
            ] {
                let reads = traced_reads(connection, query(range, &deck_ids, &[], None, &time));
                let signals = RequestSignals::materialize(connection)?;
                let walked: Vec<&String> = reads
                    .iter()
                    .filter(|sql| sql.contains(HISTORY_BY_DECK_SQL))
                    .collect();
                assert_eq!(walked.len(), HISTORY_STATEMENTS_PER_REQUEST, "{range}");
                for sql in walked {
                    assert_walks_reviews_by_card(&plan(connection, sql));
                }
                drop(signals);
            }

            // The pinned order survives planner statistics as well.
            connection
                .execute_batch("ANALYZE")
                .map_err(CommandError::database)?;
            assert!(statistics_present(connection));
            let reads = traced_reads(connection, query("7d", &[], &[], None, &time));
            let signals = RequestSignals::materialize(connection)?;
            for sql in reads
                .iter()
                .filter(|sql| sql.contains(HISTORY_BY_LOCAL_DATE_SQL))
            {
                assert_starts_from_local_date_index(&plan(connection, sql));
            }
            drop(signals);
            Ok(())
        })
        .unwrap();
}

#[test]
fn pinned_history_and_shared_signals_return_the_previous_rows() {
    let _serial = lock(&CAPTURE_LOCK);
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let time = StudyTimeContext::capture()?;
            let fixture = seed(connection, &time);
            let filters: [(Vec<&String>, Vec<&String>, Option<&String>); 7] = [
                (vec![], vec![], None),
                (vec![], vec![], Some(&fixture.sessions[2])),
                (vec![&fixture.decks[0]], vec![], Some(&fixture.sessions[5])),
                (
                    vec![&fixture.decks[1], &fixture.decks[ARCHIVED_DECK]],
                    vec![],
                    None,
                ),
                (vec![&fixture.decks[DELETED_DECK]], vec![], None),
                (vec![], vec![&fixture.tags[1]], None),
                (
                    vec![&fixture.decks[4]],
                    vec![&fixture.tags[2], &fixture.historical_tags[0]],
                    Some(&fixture.sessions[7]),
                ),
            ];
            let mut compared_rows = 0;
            for range in ["7d", "30d", "90d", "1y", "all", "custom"] {
                for (deck_ids, tag_ids, highlighted) in &filters {
                    let reads = traced_reads(
                        connection,
                        query(range, deck_ids, tag_ids, *highlighted, &time),
                    );
                    let pinned = reads
                        .iter()
                        .filter(|sql| {
                            sql.contains(HISTORY_BY_LOCAL_DATE_SQL)
                                || sql.contains(HISTORY_BY_DECK_SQL)
                        })
                        .count();
                    let shared = reads
                        .iter()
                        .filter(|sql| sql.contains(signals::JOIN_SQL))
                        .count();
                    assert_eq!(pinned, HISTORY_STATEMENTS_PER_REQUEST, "{range}");
                    assert_eq!(shared, SIGNAL_STATEMENTS_PER_REQUEST, "{range}");

                    let signals = RequestSignals::materialize(connection)?;
                    for sql in &reads {
                        let unordered = Unordered::of(sql);
                        let current = rows(connection, sql, unordered);
                        let previous = rows(connection, &previous_sql(sql), unordered);
                        assert_eq!(current, previous, "{range}: {sql}");
                        compared_rows += current.len();
                    }
                    drop(signals);
                }
            }
            // The comparison must not pass vacuously.
            assert!(compared_rows > 2_000, "only {compared_rows} rows compared");
            Ok(())
        })
        .unwrap();
}

#[test]
fn the_request_signal_table_is_dropped_after_success_and_failure() {
    Database::open_in_memory()
        .unwrap()
        .with_connection(|connection| {
            let time = StudyTimeContext::capture()?;
            let fixture = seed(connection, &time);
            let loaded = load_analytics(connection, query("30d", &[], &[], None, &time))?;
            assert!(loaded.history.review_count > 0);
            assert!(loaded.current_inventory.needs_more_practice_count > 0);
            assert!(!signal_table_exists(connection));

            // A leftover from a lost guard is replaced, not an error.
            std::mem::forget(RequestSignals::materialize(connection)?);
            assert!(signal_table_exists(connection));
            load_analytics(
                connection,
                query("all", &[&fixture.decks[0]], &[], None, &time),
            )?;
            assert!(!signal_table_exists(connection));

            // Fail mid-request, while the signal table exists: the tag query
            // reads the table dropped here.
            connection
                .execute_batch("DROP TABLE review_event_tag_attributions")
                .map_err(CommandError::database)?;
            assert!(load_analytics(connection, query("7d", &[], &[], None, &time)).is_err());
            assert!(!signal_table_exists(connection));
            Ok(())
        })
        .unwrap();
}
