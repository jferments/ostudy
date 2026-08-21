use crate::error::CommandError;

pub(super) fn bounded_diagnostic(value: &str, maximum_chars: usize) -> String {
    value.chars().take(maximum_chars).collect()
}

pub(super) fn output_error(message: impl Into<String>) -> CommandError {
    CommandError::new("ai_document_output_invalid", message)
}

pub(super) fn retryable_contract_error(error: &CommandError) -> bool {
    matches!(
        error.code.as_str(),
        "ai_document_output_invalid" | "ai_empty_response"
    )
}

pub(super) fn collapse_contract_error(error: CommandError, task: &str) -> CommandError {
    if retryable_contract_error(&error) {
        CommandError::new(
            "ai_document_output_invalid",
            format!(
                "The AI provider returned invalid {task} twice. Last validation detail: {}",
                bounded_diagnostic(&error.message, 500)
            ),
        )
    } else {
        error
    }
}

pub(super) fn truncated_output_error() -> CommandError {
    CommandError::new(
        "ai_document_output_truncated",
        "The AI provider reached its completion limit before finishing a valid structured document result. ostudy will retry this bounded source window with more output headroom and, for card generation, a smaller card count without reducing the token ceiling.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_contract_failures_collapse_to_a_bounded_stable_error() {
        let error = collapse_contract_error(
            output_error("x".repeat(1_000)),
            "document outline",
        );
        assert_eq!(error.code, "ai_document_output_invalid");
        assert!(error.message.contains("invalid document outline twice"));
        assert!(error.message.len() < 700);
    }

    #[test]
    fn truncation_remains_distinct_from_invalid_output() {
        let error = truncated_output_error();
        assert_eq!(error.code, "ai_document_output_truncated");
        assert!(error.message.contains("completion limit"));
    }
}
