#!/usr/bin/env python3
"""One-shot PR #54 regression updates for length-finished structured output."""
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
    if text.count(old) != 1:
        raise SystemExit("length-finish regression anchor not found")
    text = text.replace(old, new, 1)
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

print("Updated PR #54 length-finish and checkpoint regressions.")
