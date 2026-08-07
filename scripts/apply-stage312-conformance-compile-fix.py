#!/usr/bin/env python3
from pathlib import Path

path = Path("src-tauri/src/authoring/card_types_and_templates/generated_import/conformance_tests.rs")
text = path.read_text(encoding="utf-8")
old = '''    let over_error =
        parse_cloze(&over_source).expect_err("maximum plus one Cloze source should fail");
    assert_eq!(over_error.code, "invalid_cloze_text");
'''
new = '''    let over_error = match parse_cloze(&over_source) {
        Ok(_) => panic!("maximum plus one Cloze source should fail"),
        Err(error) => error,
    };
    assert_eq!(over_error.code, "invalid_cloze_text");
'''
if text.count(old) != 1:
    raise SystemExit("conformance test compile-fix target drifted")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
print("Applied Stage 3.12 conformance compile fix.")
