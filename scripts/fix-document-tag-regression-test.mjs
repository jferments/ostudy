#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";

const path = "src-tauri/src/documents.rs";
let source = readFileSync(path, "utf8");

const before = `    #[test]\n    fn generated_import_envelope_uses_source_format_tag() {`;
const after = `    #[test]\n    fn generated_import_envelope_has_no_provenance_default_tags() {`;
if (source.split(before).length - 1 !== 1) {
  throw new Error("expected one legacy generated-import test name");
}
source = source.replace(before, after);

const oldAssertions = `        assert!(payload.contains("\\\"PDF\\\""));\n        assert!(payload.contains("ostudy-flashcards"));`;
const newAssertions = `        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();\n        assert_eq!(value["defaultTags"], serde_json::json!([]));\n        assert_eq!(value["cards"][0]["tags"], serde_json::json!([]));\n        assert!(!payload.contains("AI generated"));\n        assert!(!payload.contains("\\\"PDF\\\""));\n        assert!(payload.contains("ostudy-flashcards"));`;
if (source.split(oldAssertions).length - 1 !== 1) {
  throw new Error("expected one legacy generated-import assertion block");
}
source = source.replace(oldAssertions, newAssertions);
writeFileSync(path, source);
console.log("Updated document import regression test for subject-matter-only tags.");
