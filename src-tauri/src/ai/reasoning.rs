use crate::error::CommandError;

pub(crate) const DEFAULT_REASONING_EFFORT: &str = "low";
pub(crate) const REASONING_EFFORTS: &[&str] = &[
    "automatic",
    "none",
    "minimal",
    "low",
    "medium",
    "high",
    "xhigh",
    "max",
];

pub(crate) fn default_reasoning_effort() -> String {
    DEFAULT_REASONING_EFFORT.to_string()
}

pub(crate) fn validate_reasoning_effort(value: &str) -> Result<String, CommandError> {
    let normalized = value.trim();
    if REASONING_EFFORTS.contains(&normalized) {
        Ok(normalized.to_string())
    } else {
        Err(CommandError::new(
            "invalid_ai_reasoning_effort",
            "Select Automatic or a reasoning effort supported by the configured model.",
        ))
    }
}

pub(crate) fn resolve_declared_effort(
    requested: &str,
    supported_efforts: Option<&[String]>,
    default_effort: Option<&str>,
    default_enabled: Option<bool>,
    mandatory: bool,
) -> String {
    let supports = |effort: &str| {
        supported_efforts.is_none_or(|efforts| efforts.iter().any(|value| value == effort))
    };
    let lowest_supported = || {
        ["minimal", "low", "medium", "high", "xhigh", "max"]
            .into_iter()
            .find(|effort| supports(effort))
            .unwrap_or("medium")
            .to_string()
    };

    if requested == "none" {
        return if mandatory {
            lowest_supported()
        } else {
            "none".to_string()
        };
    }
    if requested == "automatic" {
        if default_enabled != Some(true) && !mandatory {
            return "none".to_string();
        }
        if let Some(default) = default_effort {
            if default == "none" && !mandatory {
                return "none".to_string();
            }
            if supports(default) {
                return default.to_string();
            }
        }
        return if mandatory {
            lowest_supported()
        } else {
            "none".to_string()
        };
    }
    if supports(requested) {
        return requested.to_string();
    }
    if mandatory {
        lowest_supported()
    } else {
        "none".to_string()
    }
}

/// Structured responses need room for both the visible JSON and hidden model
/// reasoning. The completion ceiling is the combined provider budget; when the
/// provider supports an explicit reasoning-token cap, the matching headroom can
/// also be sent on the wire so reasoning cannot consume the visible JSON reserve.
pub(crate) fn structured_completion_token_ceiling(
    effective_effort: &str,
    visible_completion_tokens: usize,
) -> usize {
    let percent = visible_completion_percent(effective_effort);
    visible_completion_tokens
        .saturating_mul(100)
        .saturating_add(percent - 1)
        / percent
}

pub(crate) fn structured_reasoning_token_limit(
    effective_effort: &str,
    completion_token_ceiling: usize,
    supports_max_tokens: bool,
) -> Option<usize> {
    if !supports_max_tokens {
        return None;
    }
    let percent = visible_completion_percent(effective_effort);
    if percent == 100 {
        return None;
    }
    let visible_reserve = completion_token_ceiling.saturating_mul(percent) / 100;
    let reasoning = completion_token_ceiling.saturating_sub(visible_reserve);
    (reasoning > 0).then_some(reasoning)
}

fn visible_completion_percent(effective_effort: &str) -> usize {
    match effective_effort {
        "minimal" => 90,
        // Real OpenRouter reasoning models can spend substantially more than
        // 20% of a low-effort completion on hidden reasoning. Reserve 40% and,
        // when declared, cap reasoning to that same budget.
        "low" => 60,
        "medium" => 50,
        "high" => 20,
        "xhigh" | "max" => 5,
        _ => 100,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_complete_profile_reasoning_vocabulary() {
        for value in REASONING_EFFORTS {
            assert_eq!(validate_reasoning_effort(value).unwrap(), *value);
        }
        assert!(validate_reasoning_effort("extreme").is_err());
    }

    #[test]
    fn declared_reasoning_resolution_matches_runtime_semantics() {
        let efforts = vec!["low".to_string(), "high".to_string()];
        assert_eq!(
            resolve_declared_effort("low", Some(&efforts), Some("high"), Some(true), false),
            "low"
        );
        assert_eq!(
            resolve_declared_effort("automatic", Some(&efforts), Some("high"), Some(true), false),
            "high"
        );
        assert_eq!(
            resolve_declared_effort("none", Some(&efforts), Some("high"), Some(true), false),
            "none"
        );
        assert_eq!(
            resolve_declared_effort("medium", Some(&efforts), Some("high"), Some(true), false),
            "none"
        );
    }

    #[test]
    fn low_reasoning_preserves_visible_json_and_can_be_capped_separately() {
        let total = structured_completion_token_ceiling("low", 6_000);
        assert_eq!(total, 10_000);
        assert_eq!(
            structured_reasoning_token_limit("low", total, true),
            Some(4_000)
        );
        assert_eq!(
            structured_reasoning_token_limit("low", total, false),
            None
        );
        assert_eq!(structured_completion_token_ceiling("none", 6_000), 6_000);
    }
}
