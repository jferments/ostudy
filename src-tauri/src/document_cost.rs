//! Empirical document-AI cost calibration.
//!
//! Provider-declared context limits remain hard constraints. Curated calibration
//! rows may tighten prompt/image overhead estimates, while raw acceptance-test
//! observations live in a committed benchmark fixture and are analyzed from safe
//! diagnostic aggregates. Benchmark evidence never automatically widens a model
//! context or overrides provider capability facts.

use rusqlite::{params, OptionalExtension};

use crate::{database::Database, error::CommandError};

const MAX_RECENT_SAMPLES: i64 = 96;
const DEFAULT_PROMPT_MULTIPLIER_MILLI: usize = 1_600;
const MIN_PROMPT_MULTIPLIER_MILLI: usize = 1_000;
const MAX_PROMPT_MULTIPLIER_MILLI: usize = 8_000;
const MAX_IMAGE_PROMPT_TOKENS: usize = 8_192;

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

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BaselineCorpus {
        format_version: i64,
        observations: Vec<BaselineObservation>,
        required_benchmark_scenarios: Vec<BenchmarkScenario>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BaselineObservation {
        id: String,
        provider_kind: String,
        source_format: String,
        source_unit_count: usize,
        image_input: bool,
        aggregate_tokens_approx: usize,
        outcome: String,
        provenance: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BenchmarkScenario {
        id: String,
        source_unit_class: String,
        image_input: bool,
    }

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

    #[test]
    fn empirical_baseline_corpus_preserves_real_stage4_observations_and_matrix() {
        let corpus: BaselineCorpus = serde_json::from_str(include_str!(
            "../tests/fixtures/document-generation/usage-baselines.json"
        ))
        .expect("document usage baseline corpus should parse");
        assert_eq!(corpus.format_version, 1);
        assert!(corpus.observations.iter().any(|observation| {
            observation.id == "deepseek-small-pdf-pr38"
                && observation.provider_kind == "deepseek"
                && observation.source_format == "pdf"
                && observation.source_unit_count == 11
                && !observation.image_input
                && observation.aggregate_tokens_approx == 45_000
                && observation.outcome == "succeeded"
                && !observation.provenance.is_empty()
        }));
        assert!(corpus.observations.iter().any(|observation| {
            observation.id == "deepseek-large-pdf-pre-pr39"
                && observation.source_unit_count == 138
                && observation.aggregate_tokens_approx == 1_450_000
                && observation.outcome == "failed_late_truncation"
        }));
        assert!(corpus.required_benchmark_scenarios.iter().any(|scenario| {
            scenario.id == "small-multimodal"
                && scenario.source_unit_class == "small"
                && scenario.image_input
        }));
    }
}
