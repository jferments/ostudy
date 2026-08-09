#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function edit(relative, transform) {
  const path = fileURLToPath(new URL(`../${relative}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

await edit("src-tauri/src/document_semantics.rs", (source) => {
  const old = `            let enough_for_semantic_split = token_total
                >= plan.partition_target_tokens.saturating_div(3).max(1_500)
                || pages.len() >= plan.partition_target_pages.saturating_div(3).max(4);
            let reached_target = token_total >= plan.partition_target_tokens
                || pages.len() >= plan.partition_target_pages;

            if exceeds_hard_boundary
                || (heading_changed && enough_for_semantic_split)
                || (end > start && reached_target)
            {
                break;
            }`;
  const replacement = `            // A real top-level heading change is a stronger semantic boundary
            // than the preferred partition size. Require a small amount of
            // accumulated material so noisy one-line headings do not create
            // pathological singleton partitions, but never carry a mature
            // chapter across into the next chapter merely to hit a size target.
            let meaningful_heading_boundary = heading_changed
                && (token_total >= 1_000 || pages.len() >= 2);
            let reached_target = token_total >= plan.partition_target_tokens
                || pages.len() >= plan.partition_target_pages;

            if exceeds_hard_boundary
                || meaningful_heading_boundary
                || (end > start && reached_target)
            {
                break;
            }`;
  if (!source.includes(replacement)) {
    if (!source.includes(old)) throw new Error("Could not find semantic partition threshold block");
    source = source.replace(old, replacement);
  }
  return source;
});

await edit("src-tauri/src/ai/document_intelligence.rs", (source) => {
  source = source.replace(
    `        assert_eq!(
            outline_completion_tokens_for_provider(ProviderKind::Deepseek),
            6_144
        );
        assert_eq!(
            outline_completion_tokens_for_provider(ProviderKind::LlamaCpp),
            3_072
        );`,
    `        assert_eq!(
            outline_completion_tokens_for_provider(ProviderKind::Deepseek),
            3_072
        );
        assert_eq!(
            outline_completion_tokens_for_provider(ProviderKind::LlamaCpp),
            2_048
        );`,
  );
  return source;
});
