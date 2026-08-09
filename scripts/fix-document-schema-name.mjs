#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/ai/document_intelligence.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

source = source.replace("    schema_name: String,\n", "");
source = source.replace(/^    let schema_name = format!\([^\n]+\);\n/gmu, "");
source = source.replace(/^\s*schema_name: schema_name\.clone\(\),\n/gmu, "");
source = source.replace(/^\s*schema_name,\n/gmu, "");

if (source.includes("schema_name")) {
  throw new Error("document_intelligence.rs still contains schema_name after cleanup");
}

if (source !== original) await writeFile(path, source, "utf8");
