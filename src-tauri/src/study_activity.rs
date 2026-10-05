//! The study activity ledger and the review rollup cache (release schema 7).
//!
//! Both count reviews by local wall-clock date and hour, read from each
//! review's stored instant and UTC offset. A study date is that date and hour
//! read under the "New day starts at" setting ([`study_date`]), so changing the
//! setting regroups every past day exactly, the same way the live history
//! queries do.
//!
//! **Ledger.** The learner's own days (calendar, streaks, day totals and study
//! time) count every review still stored, whether or not its card is deleted,
//! plus `study_activity_retained`: the totals a finalized deletion kept when it
//! purged review history. Deleting cards therefore never shortens a streak.
//!
//! **Rollups.** `review_day_rollups` holds, per deck, the same review rows the
//! history queries read: reviews of the deck's current cards, with a retired
//! collection copy counted under its surviving card. Triggers add each new
//! review to a built deck and unlist a deck whenever its history could change
//! any other way; [`refresh_rollups`] rebuilds unlisted decks before a read.

use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection, TransactionBehavior};

use crate::{
    error::CommandError,
    learning_metrics::{STATE_NEW, STATE_REVIEW},
    projections::scope::{chunks, push_value},
};

/// A review attribution's local wall-clock date, from alias `a`.
pub const WALL_DATE_SQL: &str =
    "date((a.reviewed_at + a.utc_offset_minutes * 60000) / 1000, 'unixepoch')";
/// A review attribution's local wall-clock hour (0 to 23), from alias `a`.
pub const WALL_HOUR_SQL: &str = "((a.reviewed_at + a.utc_offset_minutes * 60000) / 3600000 % 24)";

/// The study date of reviews made at `wall_hour` on `wall_date`: the day
/// before when the hour is earlier than the hour a new study day starts.
pub fn study_date(wall_date: NaiveDate, wall_hour: u32, new_day_start_hour: u32) -> NaiveDate {
    if wall_hour < new_day_start_hour {
        wall_date - Duration::days(1)
    } else {
        wall_date
    }
}

/// One study date's totals for the learner, whatever their cards' fate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DayActivity {
    pub review_count: i64,
    pub remembered_count: i64,
    pub new_card_count: i64,
    pub study_time_ms: i64,
}

impl DayActivity {
    fn add(&mut self, other: Self) {
        self.review_count += other.review_count;
        self.remembered_count += other.remembered_count;
        self.new_card_count += other.new_card_count;
        self.study_time_ms += other.study_time_ms;
    }
}

/// Keeps the totals of the reviews of `card_ids` that a finalized deletion
/// operation is about to purge. Runs in the purge's transaction, before the
/// reviews are deleted.
pub fn retain_purged_reviews(
    connection: &Connection,
    operation_id: &str,
    card_ids: &[String],
) -> Result<(), CommandError> {
    for chunk in chunks(card_ids) {
        let mut values = vec![SqlValue::Text(operation_id.to_string())];
        let placeholders = chunk
            .iter()
            .map(|id| format!("?{}", push_value(&mut values, id.clone())))
            .collect::<Vec<_>>()
            .join(", ");
        connection
            .execute(
                &format!(
                    "INSERT INTO study_activity_retained(
                         operation_id, wall_date, wall_hour, review_count, remembered_count,
                         new_card_count, study_time_ms)
                     SELECT ?1, {WALL_DATE_SQL}, {WALL_HOUR_SQL}, COUNT(*),
                            SUM(r.answer_correct),
                            SUM(r.state_before = {STATE_NEW}),
                            SUM(r.response_time_ms)
                     FROM review_events r
                     CROSS JOIN review_event_attributions a ON a.review_event_id = r.id
                     WHERE r.card_id IN ({placeholders})
                     GROUP BY 2, 3
                     ON CONFLICT(operation_id, wall_date, wall_hour) DO UPDATE SET
                         review_count = review_count + excluded.review_count,
                         remembered_count = remembered_count + excluded.remembered_count,
                         new_card_count = new_card_count + excluded.new_card_count,
                         study_time_ms = study_time_ms + excluded.study_time_ms"
                ),
                params_from_iter(values.iter()),
            )
            .map_err(CommandError::database)?;
    }
    Ok(())
}

/// The learner's totals for each study date from `start` through `end`
/// (either open), with the day starting at `new_day_start_hour`.
pub fn learner_days(
    connection: &Connection,
    new_day_start_hour: u32,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) -> Result<BTreeMap<NaiveDate, DayActivity>, CommandError> {
    // A study date's reviews fall on its own wall date or the next one.
    let mut values = Vec::<SqlValue>::new();
    let mut live = vec!["1 = 1".to_string()];
    let mut retained = vec!["1 = 1".to_string()];
    if let Some(start) = start {
        let index = push_value(&mut values, start.to_string());
        // The stored local date can differ from the wall date only by a zone
        // change on that day; a day's margin covers it.
        let margin = push_value(&mut values, (start - Duration::days(1)).to_string());
        live.push(format!(
            "a.review_local_date >= ?{margin} AND {WALL_DATE_SQL} >= ?{index}"
        ));
        retained.push(format!("x.wall_date >= ?{index}"));
    }
    if let Some(end) = end {
        let next = (end + Duration::days(1)).to_string();
        let index = push_value(&mut values, next);
        let margin = push_value(&mut values, (end + Duration::days(2)).to_string());
        live.push(format!(
            "a.review_local_date <= ?{margin} AND {WALL_DATE_SQL} <= ?{index}"
        ));
        retained.push(format!("x.wall_date <= ?{index}"));
    }
    let sql = format!(
        "SELECT {WALL_DATE_SQL}, {WALL_HOUR_SQL}, COUNT(*), SUM(r.answer_correct),
                SUM(r.state_before = {STATE_NEW}), SUM(r.response_time_ms)
         FROM review_event_attributions a
         CROSS JOIN review_events r ON r.id = a.review_event_id
         WHERE {live}
         GROUP BY 1, 2
         UNION ALL
         SELECT x.wall_date, x.wall_hour, SUM(x.review_count), SUM(x.remembered_count),
                SUM(x.new_card_count), SUM(x.study_time_ms)
         FROM study_activity_retained x
         WHERE {retained}
         GROUP BY 1, 2",
        live = live.join(" AND "),
        retained = retained.join(" AND "),
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u32>(1)?,
                DayActivity {
                    review_count: row.get(2)?,
                    remembered_count: row.get(3)?,
                    new_card_count: row.get(4)?,
                    study_time_ms: row.get(5)?,
                },
            ))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut days = BTreeMap::<NaiveDate, DayActivity>::new();
    for (wall_date, wall_hour, activity) in rows {
        let date = study_date(parse_date(&wall_date)?, wall_hour, new_day_start_hour);
        if start.is_some_and(|start| date < start) || end.is_some_and(|end| date > end) {
            continue;
        }
        days.entry(date).or_default().add(activity);
    }
    Ok(days)
}

/// Every study date with a review the learner made, oldest first: stored
/// reviews of any card plus purged history the ledger kept.
pub fn learner_study_dates(
    connection: &Connection,
    new_day_start_hour: u32,
) -> Result<Vec<NaiveDate>, CommandError> {
    let sql = format!(
        "SELECT DISTINCT {WALL_DATE_SQL}, {WALL_HOUR_SQL} FROM review_event_attributions a
         UNION
         SELECT wall_date, wall_hour FROM study_activity_retained"
    );
    let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    let mut dates = rows
        .iter()
        .map(|(wall_date, wall_hour)| {
            Ok(study_date(
                parse_date(wall_date)?,
                *wall_hour,
                new_day_start_hour,
            ))
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    dates.sort_unstable();
    dates.dedup();
    Ok(dates)
}

/// Rebuilds the rollups of every undeleted deck that triggers unlisted since
/// it was last built. A deck the library never built has no rows yet.
pub fn refresh_rollups(connection: &mut Connection) -> Result<(), CommandError> {
    let stale = {
        let mut statement = connection
            .prepare(
                "SELECT d.id FROM decks d
                 WHERE d.deleted_at IS NULL
                   AND NOT EXISTS (
                       SELECT 1 FROM review_day_rollup_built_decks b WHERE b.deck_id = d.id
                   )
                 ORDER BY d.id",
            )
            .map_err(CommandError::database)?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        ids
    };
    if stale.is_empty() {
        return Ok(());
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(CommandError::database)?;
    let rebuild = format!(
        "INSERT INTO review_day_rollups(
             deck_id, wall_date, wall_hour, review_count, new_card_count, remembered_count,
             again_count, hard_count, good_count, easy_count, response_time_ms,
             retention_review_count, retention_remembered_count)
         SELECT ?1, {WALL_DATE_SQL}, {WALL_HOUR_SQL}, COUNT(*),
                SUM(r.state_before = {STATE_NEW}),
                SUM(r.answer_correct),
                SUM(r.rating = 1), SUM(r.rating = 2), SUM(r.rating = 3), SUM(r.rating = 4),
                SUM(r.response_time_ms),
                SUM({retention}),
                SUM({retention} AND r.answer_correct = 1)
         FROM cards c
         CROSS JOIN notes n ON n.id = c.note_id
         CROSS JOIN review_events r ON r.card_id IN (
             SELECT c.id
             UNION ALL
             SELECT merged.retired_card_id FROM card_merges merged
             WHERE merged.surviving_card_id = c.id
         )
         CROSS JOIN review_event_attributions a ON a.review_event_id = r.id
         WHERE c.deck_id = ?1
           AND c.deleted_at IS NULL
           AND n.deleted_at IS NULL
           AND NOT EXISTS (
               SELECT 1 FROM card_merges retired WHERE retired.retired_card_id = c.id
           )
         GROUP BY 2, 3",
        retention = format!("(r.state_before = {STATE_REVIEW} AND r.due_before <= r.reviewed_at)"),
    );
    for deck_id in &stale {
        transaction
            .execute(
                "DELETE FROM review_day_rollups WHERE deck_id = ?1",
                [deck_id],
            )
            .map_err(CommandError::database)?;
        transaction
            .execute(&rebuild, [deck_id])
            .map_err(CommandError::database)?;
        transaction
            .execute(
                "INSERT INTO review_day_rollup_built_decks(deck_id) VALUES (?1)",
                [deck_id],
            )
            .map_err(CommandError::database)?;
    }
    transaction.commit().map_err(CommandError::database)
}

pub fn parse_date(value: &str) -> Result<NaiveDate, CommandError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        CommandError::new(
            "invalid_activity_date",
            "A stored study date is outside the supported calendar.",
        )
    })
}

#[cfg(test)]
mod tests;
