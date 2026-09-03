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
}
