#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/ai/document_intelligence.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

function required(label, search, replacement) {
  if (!source.includes(search)) throw new Error(`Could not find ${label}`);
  source = source.replace(search, replacement);
}

required(
  "obsolete outline text constant",
  "const MAX_OUTLINE_TEXT_CHARS: usize = 100_000;\n",
  "",
);

required(
  "outline token helper",
  `fn outline_completion_tokens(task: &DocumentTaskContext) -> usize {
    match task.profile.provider_kind {
        ProviderKind::Deepseek => 6_144,
        ProviderKind::LlamaCpp => 3_072,
        _ => 3_072,
    }
}`,
  `fn outline_completion_tokens(task: &DocumentTaskContext) -> usize {
    outline_completion_tokens_for_provider(task.profile.provider_kind)
}

fn outline_completion_tokens_for_provider(provider_kind: ProviderKind) -> usize {
    match provider_kind {
        ProviderKind::Deepseek => 6_144,
        ProviderKind::LlamaCpp => 3_072,
        _ => 3_072,
    }
}`,
);

const testStart = source.indexOf(
  "    #[test]\n    fn outline_fragment_schema_is_deliberately_small() {",
);
const nextTest = source.indexOf(
  "\n    #[test]\n    fn global_outline_digest_reaches_the_end_of_the_document() {",
  testStart,
);
if (testStart < 0 || nextTest < 0) {
  throw new Error("Could not find outline fragment schema test block");
}
source = `${source.slice(0, testStart)}    #[test]
    fn outline_fragment_schema_is_deliberately_small() {
        let schema = outline_fragment_schema();
        assert_eq!(
            schema
                .pointer("/properties/sections/maxItems")
                .and_then(Value::as_u64),
            Some(MAX_FRAGMENT_SECTIONS as u64)
        );
        assert_eq!(
            outline_completion_tokens_for_provider(ProviderKind::Deepseek),
            6_144
        );
        assert_eq!(
            outline_completion_tokens_for_provider(ProviderKind::LlamaCpp),
            3_072
        );
    }
${source.slice(nextTest)}`;

const digestStart = source.indexOf("fn global_outline_digest(outline: &DocumentOutline) -> String {");
const digestEnd = source.indexOf("\nfn outline_context_for_window(", digestStart);
if (digestStart < 0 || digestEnd < 0) {
  throw new Error("Could not find global outline digest function");
}
const digestReplacement = String.raw`fn global_outline_digest(outline: &DocumentOutline) -> String {
    // Reserve most of the bounded digest for section coverage so the whole
    // document, including its tail, is represented. Objectives/concepts are
    // useful global hints but must never crowd later sections out of the digest.
    let mut digest = String::new();
    digest.push_str("Native whole-document summary: ");
    digest.push_str(&sanitize_outline_text(&outline.summary, 1_200, ""));
    digest.push_str("\nMajor objectives collected across fragments:\n");
    for objective in outline.learning_objectives.iter().take(8) {
        digest.push_str("- ");
        digest.push_str(&sanitize_outline_text(objective, 180, ""));
        digest.push('\n');
    }
    digest.push_str("Major concepts collected across fragments:\n");
    for concept in outline.key_concepts.iter().take(12) {
        digest.push_str("- ");
        digest.push_str(&sanitize_outline_text(concept, 120, ""));
        digest.push('\n');
    }
    digest.push_str("Sections spanning the full document:\n");

    let prefix_chars = digest.chars().count();
    let remaining = MAX_GLOBAL_SYNTHESIS_INPUT_CHARS.saturating_sub(prefix_chars);
    let section_count = outline.sections.len().max(1);
    let line_budget = (remaining / section_count).max(40);
    for section in &outline.sections {
        let fixed = format!("- pages {}–{} | ", section.page_start, section.page_end);
        let fixed_chars = fixed.chars().count();
        let body_budget = line_budget.saturating_sub(fixed_chars).max(12);
        let title_budget = body_budget.min(100).max(8);
        let title = sanitize_outline_text(&section.title, title_budget, "Section");
        let used = title.chars().count().saturating_add(3);
        let summary_budget = body_budget.saturating_sub(used);
        digest.push_str(&fixed);
        digest.push_str(&title);
        if summary_budget >= 20 {
            let summary = sanitize_outline_text(&section.summary, summary_budget, "");
            if !summary.is_empty() {
                digest.push_str(" | ");
                digest.push_str(&summary);
            }
        }
        digest.push('\n');
    }

    digest
        .chars()
        .take(MAX_GLOBAL_SYNTHESIS_INPUT_CHARS)
        .collect()
}`;
source = `${source.slice(0, digestStart)}${digestReplacement}${source.slice(digestEnd)}`;

if (source === original) throw new Error("No postfix changes were applied");
await writeFile(path, source, "utf8");
