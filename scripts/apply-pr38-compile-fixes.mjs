#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/ai/document_intelligence.rs", import.meta.url));
const source = await readFile(path, "utf8");
const search = "body_budget.min(100).max(8)";
if (!source.includes(search)) {
  throw new Error("Could not find manual clamp in document_intelligence.rs");
}
await writeFile(path, source.replace(search, "body_budget.clamp(8, 100)"), "utf8");
