//! One set of learning-metric definitions for every surface.
//!
//! Analytics, Progress, practice sessions, question types and the deletion
//! purge all count "remembered", "retention", "ready" and "due today" through
//! these helpers, so the same review never means two things on two screens.
//! Card eligibility lives in `studyability`; "Needs more practice" (a
//! struggling card) lives in `projections::learning_signals`.
//!
//! - **Remembered:** the review's stored outcome, `answer_correct`. A checked
//!   question uses its first-check verdict, even when the learner overrode the
//!   scheduling rating; every other card counts any rating above Again.
//! - **Retention review:** a review of a card in the Review state that was
//!   actually due (`due_before <= reviewed_at`). New cards, learning steps,
//!   relearning steps and early extra practice are excluded.
//! - **True retention:** remembered retention reviews over retention reviews.
//!   It is the number comparable with desired retention; the remembered rate
//!   over all reviews is not.
//! - **Ready now:** studyable cards whose due instant has passed, split into
//!   due reviews (any state but New) and new cards.
//! - **Due today:** studyable reviews due before the next study day starts.
//! - **Average confidence:** the mean rating on a 0 (Wrong) to 3 (Easy) scale.
//! - **Card strength:** New, Learning (learning or relearning), Getting there
//!   (in review, stability under 21 days) and Solid (21 days or more).

use rusqlite::Connection;

use crate::error::CommandError;

/// `card_schedules.state` and `review_events.state_*` for a new card.
pub const STATE_NEW: i64 = 0;
/// `card_schedules.state` and `review_events.state_*` for a graduated card.
pub const STATE_REVIEW: i64 = 2;

/// Stability, in days, from which a card in review counts as solid: FSRS
/// expects it to stay above 90 percent recall for about three weeks.
pub const SOLID_STABILITY_DAYS: f64 = 21.0;

/// Retention reviews a topic needs in the selected range before it is
/// classified. Fewer than this reads as "Building data".
pub const MIN_TOPIC_RETENTION_REVIEWS: i64 = 10;
/// A topic needs attention when its true retention is at least this far below
/// desired retention.
pub const NEEDS_ATTENTION_MARGIN: f64 = 0.10;
/// A topic is strong when its true retention is at least this far above
/// desired retention.
pub const STRONG_MARGIN: f64 = 0.03;

/// Remembered reviews among the rows aliased `review`.
pub fn remembered_count_sql(review: &str) -> String {
    format!("COALESCE(SUM({review}.answer_correct), 0)")
}

/// Remembered reviews among the rows matching `predicate`.
pub fn remembered_count_where_sql(review: &str, predicate: &str) -> String {
    format!("COALESCE(SUM(CASE WHEN {predicate} THEN {review}.answer_correct ELSE 0 END), 0)")
}

/// Reviews of new cards (a card's first review) among the rows aliased `review`.
pub fn new_card_review_count_sql(review: &str) -> String {
    format!("COALESCE(SUM(CASE WHEN {review}.state_before = {STATE_NEW} THEN 1 ELSE 0 END), 0)")
}

/// Whether one review aliased `review` counts toward true retention.
pub fn retention_review_sql(review: &str) -> String {
    format!(
        "({review}.state_before = {STATE_REVIEW} AND {review}.due_before <= {review}.reviewed_at)"
    )
}

/// Retention reviews among the rows matching `predicate` (`1` for all rows).
pub fn retention_review_count_sql(review: &str, predicate: &str) -> String {
    let retention = retention_review_sql(review);
    format!("COALESCE(SUM(CASE WHEN {predicate} AND {retention} THEN 1 ELSE 0 END), 0)")
}

/// Remembered retention reviews among the rows matching `predicate`.
pub fn retention_remembered_count_sql(review: &str, predicate: &str) -> String {
    let retention = retention_review_sql(review);
    format!(
        "COALESCE(SUM(CASE WHEN {predicate} AND {retention} THEN {review}.answer_correct ELSE 0 END), 0)"
    )
}

/// Two columns: retention reviews, then the remembered ones among them.
pub fn retention_counts_sql(review: &str) -> String {
    format!(
        "{}, {}",
        retention_review_count_sql(review, "1"),
        retention_remembered_count_sql(review, "1")
    )
}

/// Mean rating on the 0 (Wrong) to 3 (Easy) scale, shown as "Average confidence".
pub fn average_confidence_sql(review: &str) -> String {
    format!("AVG(CAST({review}.rating - 1 AS REAL))")
}

/// Average confidence among the rows matching `predicate`.
pub fn average_confidence_where_sql(review: &str, predicate: &str) -> String {
    format!("AVG(CASE WHEN {predicate} THEN CAST({review}.rating - 1 AS REAL) END)")
}

/// A ready review: a schedule aliased `schedule`, not new, due by `now`.
pub fn ready_review_sql(schedule: &str, now: &str) -> String {
    format!("({schedule}.due_at <= {now} AND {schedule}.state <> {STATE_NEW})")
}

/// A ready new card: a schedule aliased `schedule`, new, available by `now`.
pub fn ready_new_sql(schedule: &str, now: &str) -> String {
    format!("({schedule}.due_at <= {now} AND {schedule}.state = {STATE_NEW})")
}

/// A review due today: not new and due before the next study day starts.
pub fn due_today_review_sql(schedule: &str, tomorrow_start: &str) -> String {
    format!("({schedule}.due_at < {tomorrow_start} AND {schedule}.state <> {STATE_NEW})")
}

/// A session's remembered cards, for `practice_sessions.correct_count`.
///
/// Every review records `answer_correct` from the same rule, but session
/// counters are refreshed before the review row is written, so the rule is
/// restated over the session's cards: a checked question counts its verdict,
/// an image occlusion or a plain card counts any rating above Again.
pub fn session_remembered_count_sql(session_id: &str) -> String {
    format!(
        "(SELECT COUNT(*) FROM practice_session_cards p
          LEFT JOIN question_attempts qa
            ON qa.session_id = p.session_id AND qa.card_id = p.card_id
          WHERE p.session_id = {session_id} AND p.rated_at IS NOT NULL
            AND ((qa.instance_id IS NULL AND p.rating BETWEEN 2 AND 4)
                 OR (qa.question_kind = 'image_occlusion'
                     AND qa.phase = 'revealed' AND p.rating BETWEEN 2 AND 4)
                 OR qa.verdict = 'correct'))"
    )
}

/// Recount one session's stored counters from its remaining cards.
pub fn refresh_session_counts(
    connection: &Connection,
    session_id: &str,
) -> Result<(), CommandError> {
    connection
        .execute(
            &format!(
                "UPDATE practice_sessions
                 SET card_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = ?1
                     ),
                     viewed_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = ?1 AND viewed_at IS NOT NULL
                     ),
                     revealed_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = ?1 AND revealed_at IS NOT NULL
                     ),
                     rated_count = (
                         SELECT COUNT(*) FROM practice_session_cards
                         WHERE session_id = ?1 AND rated_at IS NOT NULL
                     ),
                     correct_count = {remembered}
                 WHERE id = ?1",
                remembered = session_remembered_count_sql("?1"),
            ),
            [session_id],
        )
        .map(|_| ())
        .map_err(CommandError::database)
}

/// `numerator / denominator`, or `None` without evidence.
pub fn rate(numerator: i64, denominator: i64) -> Option<f64> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}

/// How a topic is doing, from true retention against desired retention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopicStatus {
    BuildingData,
    NeedsAttention,
    OnTrack,
    Strong,
}

impl TopicStatus {
    /// Classify from retention reviews only. One struggling card does not make a
    /// topic need attention; its share of misses on due reviews does.
    pub fn classify(
        retention_reviews: i64,
        retention_remembered: i64,
        desired_retention: f64,
    ) -> Self {
        if retention_reviews < MIN_TOPIC_RETENTION_REVIEWS {
            return Self::BuildingData;
        }
        let retention = retention_remembered as f64 / retention_reviews as f64;
        if retention <= desired_retention - NEEDS_ATTENTION_MARGIN {
            Self::NeedsAttention
        } else if retention >= desired_retention + STRONG_MARGIN {
            Self::Strong
        } else {
            Self::OnTrack
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BuildingData => "building_data",
            Self::NeedsAttention => "needs_attention",
            Self::OnTrack => "on_track",
            Self::Strong => "strong",
        }
    }

    /// Sort order for topic lists: the ones to act on first.
    pub const fn rank(self) -> i64 {
        match self {
            Self::NeedsAttention => 0,
            Self::BuildingData => 1,
            Self::OnTrack => 2,
            Self::Strong => 3,
        }
    }
}

/// How well a card is learned, from its schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CardStrength {
    New,
    Learning,
    GettingThere,
    Solid,
}

impl CardStrength {
    pub fn of(state: i64, stability: f64) -> Self {
        match state {
            STATE_NEW => Self::New,
            STATE_REVIEW if stability >= SOLID_STABILITY_DAYS => Self::Solid,
            STATE_REVIEW => Self::GettingThere,
            _ => Self::Learning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_need_enough_retention_reviews_before_classification() {
        assert_eq!(TopicStatus::classify(9, 0, 0.9), TopicStatus::BuildingData);
        assert_eq!(
            TopicStatus::classify(10, 8, 0.9),
            TopicStatus::NeedsAttention
        );
        assert_eq!(TopicStatus::classify(10, 9, 0.9), TopicStatus::OnTrack);
        assert_eq!(TopicStatus::classify(100, 93, 0.9), TopicStatus::Strong);
    }

    #[test]
    fn card_strength_follows_state_then_stability() {
        assert_eq!(CardStrength::of(0, 0.0), CardStrength::New);
        assert_eq!(CardStrength::of(1, 40.0), CardStrength::Learning);
        assert_eq!(CardStrength::of(3, 40.0), CardStrength::Learning);
        assert_eq!(CardStrength::of(2, 20.9), CardStrength::GettingThere);
        assert_eq!(CardStrength::of(2, 21.0), CardStrength::Solid);
    }

    #[test]
    fn retention_reviews_exclude_new_learning_and_early_reviews() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE r(state_before INTEGER, due_before INTEGER,
                                reviewed_at INTEGER, answer_correct INTEGER);
                 INSERT INTO r VALUES
                     (0, 0, 10, 1),   -- new card
                     (1, 0, 10, 0),   -- learning step
                     (3, 0, 10, 0),   -- relearning step
                     (2, 20, 10, 1),  -- review practiced early
                     (2, 10, 10, 1),  -- due review, remembered
                     (2, 5, 10, 0);   -- due review, forgotten",
            )
            .unwrap();
        let (reviews, remembered, new_reviews, all_remembered): (i64, i64, i64, i64) = connection
            .query_row(
                &format!(
                    "SELECT {retention}, {new_reviews}, {remembered} FROM r",
                    retention = retention_counts_sql("r"),
                    new_reviews = new_card_review_count_sql("r"),
                    remembered = remembered_count_sql("r"),
                ),
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!((reviews, remembered), (2, 1));
        assert_eq!(new_reviews, 1);
        assert_eq!(all_remembered, 3);
    }

    #[test]
    fn session_recount_keeps_checked_verdicts_over_overridden_ratings() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE practice_sessions(id TEXT PRIMARY KEY, card_count INTEGER,
                     viewed_count INTEGER, revealed_count INTEGER, rated_count INTEGER,
                     correct_count INTEGER);
                 CREATE TABLE practice_session_cards(session_id TEXT, card_id TEXT,
                     viewed_at INTEGER, revealed_at INTEGER, rated_at INTEGER, rating INTEGER);
                 CREATE TABLE question_attempts(instance_id TEXT, session_id TEXT, card_id TEXT,
                     question_kind TEXT, phase TEXT, verdict TEXT);
                 INSERT INTO practice_sessions VALUES ('s', 9, 0, 0, 0, 9);
                 INSERT INTO practice_session_cards VALUES
                     ('s', 'plain-good', 1, 1, 1, 3),
                     ('s', 'plain-again', 1, 1, 1, 1),
                     ('s', 'checked-overridden', 1, 1, 1, 3),
                     ('s', 'checked-correct', 1, 1, 1, 2),
                     ('s', 'occlusion-good', 1, 1, 1, 4),
                     ('s', 'unrated', 1, NULL, NULL, NULL);
                 INSERT INTO question_attempts VALUES
                     ('q1', 's', 'checked-overridden', 'single_choice', 'checked', 'incorrect'),
                     ('q2', 's', 'checked-correct', 'single_choice', 'checked', 'correct'),
                     ('q3', 's', 'occlusion-good', 'image_occlusion', 'revealed', NULL);",
            )
            .unwrap();
        refresh_session_counts(&connection, "s").unwrap();
        let counts: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT card_count, viewed_count, rated_count, correct_count
                 FROM practice_sessions WHERE id = 's'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        // The overridden check stays not remembered, as its review row records.
        assert_eq!(counts, (6, 6, 5, 3));
    }
}
