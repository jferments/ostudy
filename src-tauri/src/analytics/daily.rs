use rusqlite::Connection;

use crate::{error::CommandError, settings::AppSettingsDto};

use super::{scope::AnalyticsContext, DailyLimitUsageDto};

pub fn load(
    connection: &Connection,
    context: &AnalyticsContext,
    settings: &AppSettingsDto,
) -> Result<DailyLimitUsageDto, CommandError> {
    let (new_completed, reviews_completed): (i64, i64) = connection
        .query_row(
            "SELECT
                 COALESCE(SUM(CASE WHEN r.state_before = 0 THEN 1 ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN r.state_before <> 0 THEN 1 ELSE 0 END), 0)
             FROM review_events r
             JOIN review_event_attributions a ON a.review_event_id = r.id
             WHERE a.review_local_date = ?1",
            [context.time.local_date.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(CommandError::database)?;
    Ok(DailyLimitUsageDto {
        new_limit: settings.daily_new_limit,
        review_limit: settings.daily_review_limit,
        new_completed,
        reviews_completed,
        new_remaining: settings
            .daily_new_limit
            .saturating_sub(new_completed)
            .max(0),
        reviews_remaining: settings
            .daily_review_limit
            .saturating_sub(reviews_completed)
            .max(0),
    })
}
