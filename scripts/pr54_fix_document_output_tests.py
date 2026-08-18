#!/usr/bin/env python3
"""One-shot PR #54 regression/finalization updates for structured output."""
from pathlib import Path

path = Path("src-tauri/src/ai/document_intelligence.rs")
text = path.read_text()
old = '''    #[test]
    fn truncated_provider_output_is_reported_separately_from_schema_drift() {
        let error = ensure_document_response_complete(&json!({
            "choices": [{ "finish_reason": "length" }]
        }))
        .unwrap_err();
        assert_eq!(error.code, "ai_document_output_truncated");
        assert!(error.message.contains("completion limit"));
    }'''
intermediate = old.replace(
    "ensure_document_response_complete",
    "ensure_document_response_not_blocked",
)
new = '''    #[test]
    fn length_is_truncation_only_when_the_structured_result_is_incomplete() {
        ensure_document_response_not_blocked(&json!({
            "choices": [{ "finish_reason": "length" }]
        }))
        .expect("length alone is not a policy or resource failure");
        let error = truncated_output_error();
        assert_eq!(error.code, "ai_document_output_truncated");
        assert!(error.message.contains("completion limit"));
    }'''
if new not in text:
    if text.count(intermediate) == 1:
        text = text.replace(intermediate, new, 1)
    elif text.count(old) == 1:
        text = text.replace(old, new, 1)
    else:
        raise SystemExit(
            f"length-finish regression anchor counts old={text.count(old)} intermediate={text.count(intermediate)}"
        )
path.write_text(text)

path = Path("src-tauri/src/document_checkpoints.rs")
text = path.read_text()
old = '''            extraction_backend: "extractor-v1".to_string(),
            provider_capability_fingerprint: "caps".to_string(),
        };'''
new = '''            extraction_backend: "extractor-v1".to_string(),
            provider_capability_fingerprint: "caps".to_string(),
            reasoning_effort: "none".to_string(),
        };'''
if new not in text:
    if text.count(old) != 1:
        raise SystemExit("checkpoint identity regression anchor not found")
    text = text.replace(old, new, 1)
path.write_text(text)

path = Path("src-tauri/src/document_scale.rs")
text = path.read_text()
old = '''pub(crate) fn generation_visible_completion_tokens(maximum_cards: usize) -> usize {
    1_536_usize
        .saturating_add(maximum_cards.saturating_mul(512))
        .clamp(2_048, 10_240)
}'''
new = '''pub(crate) fn generation_visible_completion_tokens(maximum_cards: usize) -> usize {
    2_048_usize
        .saturating_add(maximum_cards.saturating_mul(384))
        .clamp(2_048, 8_192)
}'''
if new not in text:
    if text.count(old) != 1:
        raise SystemExit("generation visible-output budget anchor not found")
    text = text.replace(old, new, 1)
path.write_text(text)

print("Updated PR #54 length-finish, checkpoint, and visible-output regressions.")
