//! Empirical document-AI usage calibration.
//!
//! Provider-declared context limits remain hard constraints. This module only
//! learns conservative planning multipliers from safe aggregate usage metadata
//! reported by successful provider calls. No source text, prompts, responses,
//! credentials, or reasoning content are persisted.

use rusqlite::{params, OptionalExtension};

use crate::{database::Database, error::CommandError, util::unix_time_millis};

const MAX_RECENT_SAMPLES: i64 = 96;
const DEFAULT_PROMPT_MULTIPLIER_MILLI: usize = 1_600;
const MIN_PROMPT_MULTIPLIER_MILLI: usize = 1_000;
const MAX_PROMPT_MULTIPLIER_MILLI: usize = 8_000;
const MAX_IMAGE_PROMPT_TOKENS: usize = 8_192;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DocumentUsageObservation {
    pub provider_kind: String,
    pub model_id: String,
    pub task_kind: String,
    pub source_format: String,
    pub source_unit_count: usize,
    pub source_text_chars: usize,
    pub estimated_source_tokens: usize,
    pub image_count: usize,
    pub max_output_tokens: usize,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub reasoning_tokens: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DocumentCostCalibration {
    /// Actual provider prompt tokens divided by the simple source-token estimate,
    /// expressed in thousandths. 1600 means roughly 1.6 provider prompt tokens
    /// per locally estimated source token after task/schema overhead.
    pub prompt_multiplier_milli: usize,
    /// Median residual prompt-token cost per attached image after text overhead.
    pub image_prompt_tokens: usize,
    pub sample_count: usize,
}

impl Default for DocumentCostCalibration {
    fn default() -> Self {
        Self {
            prompt_multiplier_milli: DEFAULT_PROMPT_MULTIPLIER_MILLI,
            image_prompt_tokens: 0,
            sample_count: 0,
        }
    }
}

impl DocumentCostCalibration {
    pub(crate) fn calibrated_prompt_tokens(
        self,
        estimated_source_tokens: usize,
        image_count: usize,
    ) -> usize {
        estimated_source_tokens
            .saturating_mul(self.prompt_multiplier_milli)
            .div_ceil(1_000)
            .saturating_add(image_count.saturating_mul(self.image_prompt_tokens))
    }
}

pub(crate) fn persist_usage_observations_best_effort(
    database: &Database,
    observations: &[DocumentUsageObservation],
) {
    if observations.is_empty() {
        return;
    }
    let result = database.with_connection(|connection| {
        let transaction = connection.transaction().map_err(CommandError::database)?;
        for observation in observations {
            let now = unix_time_millis()?;
            transaction
                .execute(
                    "INSERT INTO ai_document_usage_samples (
                         provider_kind, model_id, task_kind, source_format,
                         source_unit_count, source_text_chars, estimated_source_tokens,
                         image_count, max_output_tokens, prompt_tokens,
                         completion_tokens, reasoning_tokens, created_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        observation.provider_kind,
                        observation.model_id,
                        observation.task_kind,
                        observation.source_format,
                        to_i64(observation.source_unit_count),
                        to_i64(observation.source_text_chars),
                        to_i64(observation.estimated_source_tokens),
                        to_i64(observation.image_count),
                        to_i64(observation.max_output_tokens),
                        to_i64(observation.prompt_tokens),
                        to_i64(observation.completion_tokens),
                        to_i64(observation.reasoning_tokens),
                        now,
                    ],
                )
                .map_err(CommandError::database)?;
        }
        transaction.commit().map_err(CommandError::database)
    });
    if let Err(error) = result {
        crate::logging::error(
            "document_usage_sample_write_failed",
            format!(
                "samples={} code={} detail={}",
                observations.len(),
                error.code,
                error.message
            ),
        );
    }
}

pub(crate) fn load_cost_calibration(
    database: &Database,
    provider_kind: &str,
    model_id: &str,
) -> Result<DocumentCostCalibration, CommandError> {
    database.with_connection(|connection| {
        let exists = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'ai_document_usage_samples')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(CommandError::database)?
            .unwrap_or(0)
            != 0;
        if !exists {
            return Ok(DocumentCostCalibration::default());
        }

        let mut statement = connection
            .prepare(
                "SELECT estimated_source_tokens, image_count, prompt_tokens
                 FROM ai_document_usage_samples
                 WHERE provider_kind = ?1 AND model_id = ?2
                   AND estimated_source_tokens > 0 AND prompt_tokens > 0
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?3",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map(params![provider_kind, model_id, MAX_RECENT_SAMPLES], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;

        if rows.is_empty() {
            return Ok(DocumentCostCalibration::default());
        }

        let mut text_ratios = rows
            .iter()
            .filter(|(_, image_count, _)| *image_count == 0)
            .filter_map(|(source, _, prompt)| ratio_milli(*prompt, *source))
            .collect::<Vec<_>>();
        if text_ratios.is_empty() {
            text_ratios = rows
                .iter()
                .filter_map(|(source, _, prompt)| ratio_milli(*prompt, *source))
                .collect();
        }
        let prompt_multiplier_milli = median(&mut text_ratios)
            .unwrap_or(DEFAULT_PROMPT_MULTIPLIER_MILLI)
            .clamp(MIN_PROMPT_MULTIPLIER_MILLI, MAX_PROMPT_MULTIPLIER_MILLI);

        let mut image_residuals = rows
            .iter()
            .filter_map(|(source, image_count, prompt)| {
                let source = usize::try_from(*source).ok()?;
                let image_count = usize::try_from(*image_count).ok()?;
                let prompt = usize::try_from(*prompt).ok()?;
                if image_count == 0 {
                    return None;
                }
                let text_cost = source
                    .saturating_mul(prompt_multiplier_milli)
                    .div_ceil(1_000);
                Some(prompt.saturating_sub(text_cost) / image_count)
            })
            .collect::<Vec<_>>();
        let image_prompt_tokens = median(&mut image_residuals)
            .unwrap_or(0)
            .min(MAX_IMAGE_PROMPT_TOKENS);

        Ok(DocumentCostCalibration {
            prompt_multiplier_milli,
            image_prompt_tokens,
            sample_count: rows.len(),
        })
    })
}

fn ratio_milli(prompt: i64, source: i64) -> Option<usize> {
    let prompt = usize::try_from(prompt).ok()?;
    let source = usize::try_from(source).ok()?.max(1);
    Some(prompt.saturating_mul(1_000).div_ceil(source))
}

fn median(values: &mut [usize]) -> Option<usize> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        Some(values[middle - 1].saturating_add(values[middle]) / 2)
    } else {
        Some(values[middle])
    }
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_scales_text_and_image_cost_without_becoming_a_hard_limit() {
        let calibration = DocumentCostCalibration {
            prompt_multiplier_milli: 1_750,
            image_prompt_tokens: 900,
            sample_count: 8,
        };
        assert_eq!(calibration.calibrated_prompt_tokens(4_000, 2), 8_800);
    }

    #[test]
    fn median_is_robust_to_one_large_outlier() {
        let mut values = vec![1_500, 1_550, 1_600, 7_900, 1_625];
        assert_eq!(median(&mut values), Some(1_600));
    }

    #[test]
    fn ratio_uses_integer_ceiling_and_rejects_negative_values() {
        assert_eq!(ratio_milli(1_501, 1_000), Some(1_501));
        assert_eq!(ratio_milli(-1, 1_000), None);
    }
}
