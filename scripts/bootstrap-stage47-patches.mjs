#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

for (const relative of [
  "apply-stage47-ai.mjs",
  "apply-stage47-pipeline.mjs",
  "apply-stage47-more.mjs",
]) {
  const path = fileURLToPath(new URL(`./${relative}`, import.meta.url));
  const source = await readFile(path, "utf8");
  // The patch scripts only use ASCII structural patterns. Removing the Unicode
  // regex flag avoids JavaScript's stricter lone-brace parsing for Rust block
  // delimiters such as `\n}\n`.
  const updated = source.replaceAll("/u,", "/,").replaceAll("/u)", "/)");
  if (updated !== source) await writeFile(path, updated, "utf8");
}
