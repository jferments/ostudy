#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function patch(relativePath, transform) {
  const path = fileURLToPath(new URL(`../${relativePath}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

function required(source, label, search, replacement) {
  if (!source.includes(search)) throw new Error(`Could not find ${label}`);
  return source.replace(search, replacement);
}

await patch("src-tauri/src/document_more.rs", (source) =>
  required(
    source,
    "additional-generation window constants",
    `const GENERATION_WINDOW_MAX_TOKENS: usize = 20_000;
const GENERATION_WINDOW_MAX_PAGES: usize = 40;
const GENERATION_WINDOW_OVERLAP_TOKENS: usize = 2_000;`,
    `// Additional passes use the same conservative provider boundary as initial
// generation. Reusing a persisted outline must not reintroduce the old large
// source-window context risk.
const GENERATION_WINDOW_MAX_TOKENS: usize = 5_000;
const GENERATION_WINDOW_MAX_PAGES: usize = 10;
const GENERATION_WINDOW_OVERLAP_TOKENS: usize = 700;`,
  ),
);

await patch("src-tauri/src/documents.rs", (source) =>
  required(
    source,
    "persisted outline prompt revision",
    `                 ) VALUES (?1, 1, 1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)`,
    `                 ) VALUES (?1, 1, 2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)`,
  ),
);
