#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/ai/document_intelligence.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

function replaceRequired(label, search, replacement) {
  if (!source.includes(search)) throw new Error(`Could not find ${label}`);
  source = source.replace(search, replacement);
}

if (!source.includes("struct OutlineSourceBounds")) {
  replaceRequired(
    "outline fragment record",
    `struct OutlineFragmentRecord {
    window_index: usize,
    page_start: i64,
    page_end: i64,
    source_tokens: usize,
    outline: DocumentOutline,
}
`,
    `struct OutlineFragmentRecord {
    window_index: usize,
    page_start: i64,
    page_end: i64,
    source_tokens: usize,
    outline: DocumentOutline,
}

#[derive(Clone, Copy, Debug)]
struct OutlineSourceBounds {
    page_start: i64,
    page_end: i64,
    page_count: i64,
}
`,
  );
}

replaceRequired(
  "outline bounds call",
  `        images,
        window.page_start,
        window.page_end,
        page_count,
    )`,
  `        images,
        OutlineSourceBounds {
            page_start: window.page_start,
            page_end: window.page_end,
            page_count,
        },
    )`,
);

replaceRequired(
  "outline bounds signature",
  `    images: &[SourcePageImage],
    page_start: i64,
    page_end: i64,
    page_count: i64,
) -> Result<DocumentOutline, CommandError> {`,
  `    images: &[SourcePageImage],
    bounds: OutlineSourceBounds,
) -> Result<DocumentOutline, CommandError> {`,
);

source = source.replaceAll(
  `parse_outline_fragment(value, page_start, page_end, page_count)`,
  `parse_outline_fragment(value, bounds.page_start, bounds.page_end, bounds.page_count)`,
);

if (source !== original) await writeFile(path, source, "utf8");
