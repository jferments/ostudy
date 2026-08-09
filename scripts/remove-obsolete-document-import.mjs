#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";

const path = new URL("../src-tauri/src/documents.rs", import.meta.url);
let source = await readFile(path, "utf8");

if (!source.includes("pub async fn import_pdf_and_generate_flashcards(")) {
  process.exit(0);
}

function removeBetween(startMarker, endMarker, label) {
  const start = source.indexOf(startMarker);
  if (start < 0) throw new Error(`Could not locate start of obsolete ${label}`);
  const end = source.indexOf(endMarker, start + startMarker.length);
  if (end < 0) throw new Error(`Could not locate end of obsolete ${label}`);
  source = source.slice(0, start) + source.slice(end);
}

source = source.replace("    io::{Read, Seek, Write},\n", "    io::Write,\n");
source = source.replace("const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;\n", "");

removeBetween(
  "#[tauri::command]\npub async fn import_pdf_and_generate_flashcards(",
  "#[tauri::command]\npub async fn retry_pdf_flashcard_generation(",
  "combined PDF import/generation command",
);
removeBetween(
  "fn import_managed_pdf(",
  "fn extract_and_render_pdf(",
  "managed PDF importer",
);
removeBetween(
  "fn copy_pdf_with_hash(",
  "fn bounded_error(",
  "PDF copy and import validators",
);
removeBetween(
  "    #[test]\n    fn validates_pdf_names_and_titles() {",
  "    #[test]\n    fn samples_images_across_a_large_semantic_window() {",
  "obsolete PDF import validator test",
);

for (const obsolete of [
  "import_pdf_and_generate_flashcards",
  "fn import_managed_pdf(",
  "fn copy_pdf_with_hash(",
  "fn validate_pdf_filename(",
  "fn validate_document_title(",
]) {
  if (source.includes(obsolete)) {
    throw new Error(`Obsolete document import path survived cleanup: ${obsolete}`);
  }
}

await writeFile(path, source, "utf8");
