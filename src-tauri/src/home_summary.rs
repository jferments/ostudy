//! The Home summary: what is due today, per subject and in total, and about
//! how long reviewing it takes.
//!
//! One read-only command serves Home's Today strip and subject tiles and the
//! Review destination. It reuses the study rules instead of restating them:
//! "studyable" and "due" come from [`studyability`], today's completed reviews
//! from [`workload::completed_today`], the study date from
//! [`StudyTimeContext`], and the daily limits from [`DailyAllowance`], the same
//! rule a scheduled practice session applies when it starts.
//!
//! Home is the landing page and every native command waits in one queue, so the
//! summary avoids the learning-signal projection (which reads every review) and
//! reads review history only through indexes: today's completed reviews through
//! the local-date index, and the recent study time through the review-time
//! index with a bounded window.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::{
    database::Database,
    error::CommandError,
    settings::load_settings,
    study_calendar::StudyTimeContext,
    studyability, subjects,
    workload::{self, DailyAllowance},
};

/// Reviews older than this do not inform the time estimate.
pub(crate) const ESTIMATE_WINDOW_DAYS: i64 = 30;
/// The most recent timed reviews the estimate reads.
pub(crate) const ESTIMATE_SAMPLE_REVIEWS: i64 = 200;
/// Fewer timed reviews than this fall back to [`DEFAULT_MS_PER_CARD`].
pub(crate) const ESTIMATE_MIN_REVIEWS: usize = 20;
/// Twelve seconds a card until the learner's own reviews say otherwise.
pub(crate) const DEFAULT_MS_PER_CARD: i64 = 12_000;
const DAY_MS: i64 = 86_400_000;

/// What is due today in one subject, or in No subject.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeSubjectSummaryDto {
    /// `None` is No subject: decks and documents filed in no subject.
    pub subject_id: Option<String>,
    /// Studyable review cards due now in the subject's live, unarchived decks.
    pub due_reviews: i64,
    /// Studyable new cards available now in those decks.
    pub due_new: i64,
    /// The reviews this subject's own scheduled session would study today,
    /// under the daily limits that are left.
    pub scheduled_reviews: i64,
    /// The new cards that session would study.
    pub scheduled_new: i64,
    /// Current cards in the subject's live, unarchived decks, suspended
    /// cards included. A collection copy merged into its original is retired
    /// and counts once, as its original.
    pub card_count: i64,
    /// Live, unarchived decks filed in the subject.
    pub deck_count: i64,
    /// Documents filed in the subject.
    pub source_count: i64,
    /// Whether the subject has a published overview guide (always false for
    /// No subject).
    pub has_overview_guide: bool,
    /// The decks holding the subject's due cards, sorted: the scope its
    /// scheduled session starts with.
    pub due_deck_ids: Vec<String>,
}

/// What Start review would study today across the whole library.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeTodayDto {
    pub due_reviews: i64,
    pub due_new: i64,
    /// The reviews Start review schedules under the daily review limit.
    pub scheduled_reviews: i64,
    /// The new cards Start review schedules under the daily new-card limit.
    pub scheduled_new: i64,
    pub reviews_remaining: i64,
    pub new_remaining: i64,
    /// Subjects holding due cards of a kind Start review studies today (due
    /// reviews when it schedules reviews, new cards when it schedules new
    /// ones); No subject counts as one. When a daily limit cuts the session
    /// short, this can include a subject whose cards rank after the cut.
    pub subject_count: i64,
    /// The decks holding every due card, sorted: Start review's scope.
    pub deck_ids: Vec<String>,
    /// About how many minutes the scheduled cards take, rounded, at least 1
    /// whenever anything is scheduled.
    pub estimated_minutes: i64,
    /// The time per card the estimate used.
    pub estimate_ms_per_card: i64,
    /// The timed reviews behind the estimate; 0 when it used the default.
    pub estimate_review_count: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeSummaryDto {
    /// The study date the counts belong to, as `YYYY-MM-DD`.
    pub study_date: String,
    pub today: HomeTodayDto,
    /// Every subject, in the learner's order.
    pub subjects: Vec<HomeSubjectSummaryDto>,
    pub no_subject: HomeSubjectSummaryDto,
}

#[tauri::command]
pub fn get_home_summary(database: State<'_, Database>) -> Result<HomeSummaryDto, CommandError> {
    database.with_connection(|connection| {
        let time = StudyTimeContext::capture()?;
        load_home_summary(connection, &time)
    })
}

pub(crate) fn load_home_summary(
    connection: &mut Connection,
    time: &StudyTimeContext,
) -> Result<HomeSummaryDto, CommandError> {
    let settings = load_settings(connection)?;
    let (new_completed, reviews_completed) = workload::completed_today(connection, time)?;
    let allowance = DailyAllowance::new(&settings, new_completed, reviews_completed);

    let mut groups: BTreeMap<Option<String>, HomeSubjectSummaryDto> = BTreeMap::new();
    let ordered_subjects: Vec<String> = subjects::list_subjects(connection)?
        .into_iter()
        .map(|subject| subject.id)
        .collect();
    for subject_id in &ordered_subjects {
        groups.insert(
            Some(subject_id.clone()),
            empty_group(Some(subject_id.clone())),
        );
    }
    groups.insert(None, empty_group(None));

    for (subject_id, deck_count) in grouped_counts(
        connection,
        "SELECT subject_id, COUNT(*) FROM decks
         WHERE deleted_at IS NULL AND archived_at IS NULL
         GROUP BY subject_id",
    )? {
        group(&mut groups, subject_id).deck_count = deck_count;
    }
    for (subject_id, source_count) in grouped_counts(
        connection,
        "SELECT subject_id, COUNT(*) FROM documents GROUP BY subject_id",
    )? {
        group(&mut groups, subject_id).source_count = source_count;
    }
    for subject_id in subjects_with_overview_guides(connection)? {
        if let Some(entry) = groups.get_mut(&Some(subject_id)) {
            entry.has_overview_guide = true;
        }
    }

    let mut due_decks = BTreeSet::new();
    for deck in deck_card_counts(connection, time)? {
        let entry = group(&mut groups, deck.subject_id);
        entry.card_count += deck.card_count;
        entry.due_reviews += deck.due_reviews;
        entry.due_new += deck.due_new;
        if deck.due_reviews + deck.due_new > 0 {
            entry.due_deck_ids.push(deck.deck_id.clone());
            due_decks.insert(deck.deck_id);
        }
    }

    let mut today = HomeTodayDto {
        reviews_remaining: allowance.reviews_remaining,
        new_remaining: allowance.new_remaining,
        deck_ids: due_decks.into_iter().collect(),
        ..HomeTodayDto::default()
    };
    for entry in groups.values_mut() {
        entry.due_deck_ids.sort();
        (entry.scheduled_reviews, entry.scheduled_new) =
            allowance.schedule(entry.due_reviews, entry.due_new);
        today.due_reviews += entry.due_reviews;
        today.due_new += entry.due_new;
    }
    (today.scheduled_reviews, today.scheduled_new) =
        allowance.schedule(today.due_reviews, today.due_new);
    today.subject_count = groups
        .values()
        .filter(|entry| {
            (entry.due_reviews > 0 && today.scheduled_reviews > 0)
                || (entry.due_new > 0 && today.scheduled_new > 0)
        })
        .count()
        .try_into()
        .unwrap_or(i64::MAX);

    let (ms_per_card, review_count) = recent_ms_per_card(connection, time)?;
    today.estimate_ms_per_card = ms_per_card;
    today.estimate_review_count = review_count;
    today.estimated_minutes =
        estimated_minutes(today.scheduled_reviews + today.scheduled_new, ms_per_card);

    let no_subject = groups.remove(&None).unwrap_or_else(|| empty_group(None));
    let subjects = ordered_subjects
        .into_iter()
        .filter_map(|subject_id| groups.remove(&Some(subject_id)))
        .collect();
    Ok(HomeSummaryDto {
        study_date: time.local_date.to_string(),
        today,
        subjects,
        no_subject,
    })
}

fn empty_group(subject_id: Option<String>) -> HomeSubjectSummaryDto {
    HomeSubjectSummaryDto {
        subject_id,
        ..HomeSubjectSummaryDto::default()
    }
}

/// The entry for `subject_id`. A row filed in a subject the list did not
/// return (it was deleted between the two reads) counts under No subject, as
/// deleting a subject files its decks and documents there.
fn group(
    groups: &mut BTreeMap<Option<String>, HomeSubjectSummaryDto>,
    subject_id: Option<String>,
) -> &mut HomeSubjectSummaryDto {
    let key = if groups.contains_key(&subject_id) {
        subject_id
    } else {
        None
    };
    groups.entry(key).or_insert_with(|| empty_group(None))
}

fn grouped_counts(
    connection: &Connection,
    sql: &str,
) -> Result<Vec<(Option<String>, i64)>, CommandError> {
    let mut statement = connection.prepare(sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    rows.map_err(CommandError::database)
}

fn subjects_with_overview_guides(connection: &Connection) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT target.collection_id
             FROM study_targets target
             JOIN study_target_guides guide
               ON guide.target_id = target.id AND guide.state = 'active'
             WHERE target.target_kind = 'collection'",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| row.get(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    rows.map_err(CommandError::database)
}

struct DeckCounts {
    deck_id: String,
    subject_id: Option<String>,
    card_count: i64,
    due_reviews: i64,
    due_new: i64,
}

/// Current cards and studyable due cards per live, unarchived deck, in one
/// grouped pass over the current cards. A retired collection copy is a deleted
/// card, so it is neither counted nor due.
fn deck_card_counts(
    connection: &Connection,
    time: &StudyTimeContext,
) -> Result<Vec<DeckCounts>, CommandError> {
    let due = studyability::studyable_due_sql("?1");
    let sql = format!(
        "SELECT c.deck_id,
                d.subject_id,
                COUNT(*),
                COALESCE(SUM(CASE WHEN {due} AND s.state <> 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN {due} AND s.state = 0 THEN 1 ELSE 0 END), 0)
         FROM cards c
         JOIN notes n ON n.id = c.note_id
         JOIN decks d ON d.id = c.deck_id
         LEFT JOIN card_schedules s ON s.card_id = c.id
         WHERE {current}
         GROUP BY c.deck_id, d.subject_id",
        current = studyability::current_card_sql("c", "n", "d"),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params![time.now_utc_millis], |row| {
            Ok(DeckCounts {
                deck_id: row.get(0)?,
                subject_id: row.get(1)?,
                card_count: row.get(2)?,
                due_reviews: row.get(3)?,
                due_new: row.get(4)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>();
    rows.map_err(CommandError::database)
}

/// The learner's typical active time per card: the median of the most recent
/// [`ESTIMATE_SAMPLE_REVIEWS`] reviews with a measured active time from the
/// last [`ESTIMATE_WINDOW_DAYS`] days. The median keeps one card left open on
/// screen from inflating the estimate. With fewer than
/// [`ESTIMATE_MIN_REVIEWS`] such reviews it is [`DEFAULT_MS_PER_CARD`].
/// Returns the time and the number of reviews it came from (0 for the
/// default). `CROSS JOIN` keeps the review-time index as the outer loop, so
/// the read stops after the sample however long the history is.
fn recent_ms_per_card(
    connection: &Connection,
    time: &StudyTimeContext,
) -> Result<(i64, i64), CommandError> {
    let since = time.now_utc_millis - ESTIMATE_WINDOW_DAYS * DAY_MS;
    let mut statement = connection
        .prepare(
            "SELECT context.active_time_ms
             FROM review_events r
             CROSS JOIN review_event_scheduler_context context
               ON context.review_event_id = r.id
             WHERE r.reviewed_at >= ?1 AND r.reviewed_at <= ?2
               AND context.active_time_ms IS NOT NULL
             ORDER BY r.reviewed_at DESC
             LIMIT ?3",
        )
        .map_err(CommandError::database)?;
    let mut sample = statement
        .query_map(
            params![since, time.now_utc_millis, ESTIMATE_SAMPLE_REVIEWS],
            |row| row.get::<_, i64>(0),
        )
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(median_ms_per_card(&mut sample))
}

fn median_ms_per_card(sample: &mut [i64]) -> (i64, i64) {
    if sample.len() < ESTIMATE_MIN_REVIEWS {
        return (DEFAULT_MS_PER_CARD, 0);
    }
    sample.sort_unstable();
    let middle = sample.len() / 2;
    let median = if sample.len().is_multiple_of(2) {
        (sample[middle - 1] + sample[middle]) / 2
    } else {
        sample[middle]
    };
    (median, i64::try_from(sample.len()).unwrap_or(i64::MAX))
}

/// Minutes for `cards` at `ms_per_card`, rounded to the nearest minute and at
/// least 1 when there is anything to study.
fn estimated_minutes(cards: i64, ms_per_card: i64) -> i64 {
    if cards <= 0 {
        return 0;
    }
    let total = cards.saturating_mul(ms_per_card.max(0));
    (total.saturating_add(30_000) / 60_000).max(1)
}

#[cfg(test)]
#[path = "home_summary_tests.rs"]
mod tests;
