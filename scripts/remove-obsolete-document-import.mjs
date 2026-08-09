#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";

const path = new URL("../src-tauri/src/documents.rs", import.meta.url);
let source = await readFile(path, "utf8");

if (!source.includes("pub async fn import_pdf_and_generate_flashcards(")) {
  process.exit(0);
}

function removeOnce(pattern, label) {
  const matches = source.match(pattern);
  if (!matches) throw new Error(`Could not locate obsolete ${label}`);
  source = source.replace(pattern, "");
}

source = source.replace("    io::{Read, Seek, Write},\n", "    io::Write,\n");
source = source.replace("const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;\n", "");

removeOnce(
  /#\[tauri::command\]\npub async fn import_pdf_and_generate_flashcards\([\s\S]*?\n}\n\n(?=#\[tauri::command\]\npub async fn retry_pdf_flashcard_generation)/u,
  "combined PDF import/generation command",
);
removeOnce(
  /fn import_managed_pdf\([\s\S]*?\n}\n\n(?=fn extract_and_render_pdf)/u,
  "managed PDF importer",
);
removeOnce(
  /fn copy_pdf_with_hash\([\s\S]*?\n}\n\n(?=fn validate_pdf_filename)/u,
  "PDF copy/hash helper",
);
removeOnce(
  /fn validate_pdf_filename\([\s\S]*?\n}\n\n(?=fn validate_document_title)/u,
  "PDF filename validator",
);
removeOnce(
  /fn validate_document_title\([\s\S]*?\n}\n\n(?=fn bounded_error)/u,
  "document-title validator",
);
removeOnce(
  /    #\[test\]\n    fn validates_pdf_names_and_titles\(\) \{[\s\S]*?\n    }\n\n(?=    #\[test\]\n    fn samples_images_across_a_large_semantic_window)/u,
  "obsolete PDF import validator test",
);

if (source.includes("import_pdf_and_generate_flashcards") || source.includes("fn import_managed_pdf(")) {
  throw new Error("Obsolete document import path survived cleanup");
}

await writeFile(path, source, "utf8");
