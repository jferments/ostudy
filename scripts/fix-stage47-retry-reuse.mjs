#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/documents.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

const start = source.indexOf("pub async fn retry_pdf_flashcard_generation(");
const end = source.indexOf("\n#[tauri::command]\npub fn set_document_archived", start);
if (start < 0 || end < 0) throw new Error("Could not find retry command");
let section = source.slice(start, end);
if (!section.includes("&input.document_id,\n        false,")) {
  const updated = section.replace(
    "&input.document_id,\n        true,",
    "&input.document_id,\n        false,",
  );
  if (updated === section) throw new Error("Could not find forced extraction flag");
  source = `${source.slice(0, start)}${updated}${source.slice(end)}`;
}

if (source !== original) await writeFile(path, source, "utf8");
