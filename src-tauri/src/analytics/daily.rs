use rusqlite::Connection;

use crate::{error::CommandError, settings::AppSettingsDto, workload::completed_today};

use super::{scope::AnalyticsContext, DailyLimitUsageDto};

pub fn load(
    connection: &Connection,
    context: &AnalyticsContext,
    settings: &AppSettingsDto,
) -> Result<DailyLimitUsageDto, CommandError> {
    let (new_completed, reviews_completed) = completed_today(connection, &context.time)?;
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
