//! The full review log, newest first, one keyset page at a time.

use rusqlite::{types::Value as SqlValue, Connection};
use serde::{Deserialize, Serialize};

use crate::{error::CommandError, projections::scope::push_value, util::validate_entity_id};

use super::super::{recent, scope::AnalyticsContext, RecentReviewDto};

pub const REVIEW_LOG_PAGE_SIZE: i64 = 50;

/// Wall time, offset included, can put a study date up to a day either side of
/// the instant's own; the exact study-date filter still applies.
const WINDOW_MARGIN_MS: i64 = 86_400_000;

/// The log walks `review_events_reviewed_idx` from the newest review and stops
/// after a page, so a page costs about the same at any depth and for any
/// range. Without a bound on `reviewed_at` SQLite preferred scanning a covering
/// index and sorting every review, so the index is named. Rows are the same as every other history query's: a merged copy's
/// review is listed under its surviving card.
pub(in crate::analytics) const REVIEW_LOG_FROM_SQL: &str =
    "review_events r INDEXED BY review_events_reviewed_idx
         CROSS JOIN review_event_attributions a ON a.review_event_id = r.id
         LEFT JOIN card_merges merged ON merged.retired_card_id = r.card_id
         CROSS JOIN cards c ON c.id = COALESCE(merged.surviving_card_id, r.card_id)
         CROSS JOIN notes n ON n.id = c.note_id
         CROSS JOIN decks d ON d.id = c.deck_id";

/// Where the next page starts: the last row of the previous one.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewLogCursorDto {
    pub reviewed_at: i64,
    pub created_at: i64,
    pub id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewLogPageDto {
    pub reviews: Vec<RecentReviewDto>,
    /// `None` on the last page.
    pub next: Option<ReviewLogCursorDto>,
}

pub fn load_page(
    connection: &Connection,
    context: &AnalyticsContext,
    before: Option<&ReviewLogCursorDto>,
) -> Result<ReviewLogPageDto, CommandError> {
    if let Some(cursor) = before {
        validate_entity_id(&cursor.id, "review")?;
    }
    let window = context.window.current.as_ref();
    let mut reviews = recent::load_review_rows(
        connection,
        context,
        REVIEW_LOG_FROM_SQL,
        |values: &mut Vec<SqlValue>| {
            let mut conditions = Vec::new();
            if let Some(window) = window {
                let start = push_value(
                    values,
                    window.start_utc_millis.saturating_sub(WINDOW_MARGIN_MS),
                );
                let end = push_value(
                    values,
                    window.end_utc_millis.saturating_add(WINDOW_MARGIN_MS),
                );
                conditions.push(format!(
                    "r.reviewed_at >= ?{start} AND r.reviewed_at < ?{end}"
                ));
            }
            if let Some(cursor) = before {
                let reviewed_at = push_value(values, cursor.reviewed_at);
                let created_at = push_value(values, cursor.created_at);
                let id = push_value(values, cursor.id.clone());
                // The first term lets the index range stop at the cursor.
                conditions.push(format!(
                    "r.reviewed_at <= ?{reviewed_at}
                     AND (r.reviewed_at, r.created_at, r.id)
                         < (?{reviewed_at}, ?{created_at}, ?{id})"
                ));
            }
            conditions
        },
        REVIEW_LOG_PAGE_SIZE + 1,
    )?;
    let more = i64::try_from(reviews.len()).unwrap_or(i64::MAX) > REVIEW_LOG_PAGE_SIZE;
    reviews.truncate(usize::try_from(REVIEW_LOG_PAGE_SIZE).unwrap_or_default());
    let next = more
        .then(|| reviews.last())
        .flatten()
        .map(|last| ReviewLogCursorDto {
            reviewed_at: last.reviewed_at,
            created_at: last.created_at,
            id: last.id.clone(),
        });
    Ok(ReviewLogPageDto { reviews, next })
}
