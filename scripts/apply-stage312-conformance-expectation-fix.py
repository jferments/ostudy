#!/usr/bin/env python3
from pathlib import Path

valid = Path("tests/fixtures/generated-import/valid-conformance.ostudy.json")
text = valid.read_text(encoding="utf-8")
for marker, label in [
    ("---", "Horizontal rule dashes"),
    ("***", "Horizontal rule asterisks"),
    ("___", "Horizontal rule underscores"),
]:
    old = f'{{"type":"basic","front":"{marker}","back":"{label}"}}'
    new = f'{{"type":"basic","front":"Before\\n\\n{marker}\\n\\nAfter","back":"{label}"}}'
    if text.count(old) != 1:
        raise SystemExit(f"expected horizontal-rule fixture once: {marker}")
    text = text.replace(old, new, 1)
valid.write_text(text, encoding="utf-8")

path = Path("src-tauri/src/authoring/card_types_and_templates/generated_import/conformance_tests.rs")
text = path.read_text(encoding="utf-8")
old = '''    let max_source_padding = MAX_CLOZE_SOURCE_CHARS - "{{c1::x}}".chars().count();
    let max_source = format!("{}{{{{c1::x}}}}", "a".repeat(max_source_padding));
    assert_eq!(max_source.chars().count(), MAX_CLOZE_SOURCE_CHARS);
    let valid_cloze = wrapped_card(
        "maximum cloze source",
        json!({"type":"cloze","text":max_source}),
    );
    prepare_flashcard_import_bytes(
        &serde_json::to_vec(&valid_cloze).expect("valid cloze boundary should serialize"),
    )
    .expect("exact maximum Cloze source length should validate");

    let over_source = format!("{}{{{{c1::x}}}}", "a".repeat(max_source_padding + 1));
    let invalid_cloze = wrapped_card(
        "overlong cloze source",
        json!({"type":"cloze","text":over_source}),
    );
    assert!(prepare_flashcard_import_bytes(
        &serde_json::to_vec(&invalid_cloze).expect("invalid cloze boundary should serialize")
    )
    .is_err());'''
new = '''    // The Cloze parser has its own 200k source bound, while the generated
    // question/answer must independently satisfy canonical card-document/text
    // bounds. Test those layers separately so the corpus does not assume that
    // a maximum parser input is necessarily a renderable card.
    let max_source_padding = MAX_CLOZE_SOURCE_CHARS - "{{c1::x}}".chars().count();
    let max_source = format!("{}{{{{c1::x}}}}", "a".repeat(max_source_padding));
    assert_eq!(max_source.chars().count(), MAX_CLOZE_SOURCE_CHARS);
    parse_cloze(&max_source).expect("exact maximum Cloze source should pass marker parsing");

    let over_source = format!("{}{{{{c1::x}}}}", "a".repeat(max_source_padding + 1));
    let over_error = parse_cloze(&over_source).expect_err("maximum plus one Cloze source should fail");
    assert_eq!(over_error.code, "invalid_cloze_text");

    let large_renderable_source = format!("{}{{{{c1::x}}}}", "a".repeat(150_000));
    let valid_cloze = wrapped_card(
        "large renderable cloze source",
        json!({"type":"cloze","text":large_renderable_source}),
    );
    prepare_flashcard_import_bytes(
        &serde_json::to_vec(&valid_cloze).expect("large Cloze fixture should serialize"),
    )
    .expect("a large source whose rendered sides fit canonical card limits should validate");'''
if text.count(old) != 1:
    raise SystemExit("expected old Cloze boundary assertion block exactly once")
path.write_text(text.replace(old, new, 1), encoding="utf-8")

print("Corrected generated-import conformance fixture expectations.")
