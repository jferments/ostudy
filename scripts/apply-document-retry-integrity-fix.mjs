#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(new URL("../src-tauri/src/documents.rs", import.meta.url));
let source = await readFile(path, "utf8");
const original = source;

const retryStart = source.indexOf("pub async fn retry_pdf_flashcard_generation(");
const retryEnd = source.indexOf("\n#[tauri::command]\npub fn set_document_archived", retryStart);
if (retryStart < 0 || retryEnd < 0) {
  throw new Error("Could not find the PDF retry command");
}
const retrySource = source.slice(retryStart, retryEnd);
if (!retrySource.includes("&input.document_id,\n        true,")) {
  const fixedRetry = retrySource.replace(
    "&input.document_id,\n        false,",
    "&input.document_id,\n        true,",
  );
  if (fixedRetry === retrySource) {
    throw new Error("Could not update retry extraction policy");
  }
  source = `${source.slice(0, retryStart)}${fixedRetry}${source.slice(retryEnd)}`;
}

const oldIntegrity = "if bytes.is_empty() || bytes.len() > MAX_RENDER_BYTES {";
const newIntegrity = `if bytes.is_empty()
                || bytes.len() > MAX_RENDER_BYTES
                || i64::try_from(bytes.len()).unwrap_or(i64::MAX) != page.render_byte_size
                || sha256_hex(&bytes) != page.render_sha256
            {`;
if (!source.includes("sha256_hex(&bytes) != page.render_sha256")) {
  if (!source.includes(oldIntegrity)) {
    throw new Error("Could not find rendered-page request validation");
  }
  source = source.replace(oldIntegrity, newIntegrity);
}

if (source !== original) await writeFile(path, source, "utf8");
