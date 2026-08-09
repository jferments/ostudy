#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function edit(relative, transform) {
  const path = fileURLToPath(new URL(`../${relative}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

await edit("src-tauri/src/ai/document_intelligence.rs", (source) => {
  source = source.replace(
    "const OUTLINE_PROMPT_VERSION: i64 = 3;\nconst OUTLINE_SCHEMA_VERSION: i64 = 1;",
    "pub(crate) const OUTLINE_PROMPT_VERSION: i64 = 4;\npub(crate) const OUTLINE_SCHEMA_VERSION: i64 = 1;",
  );
  if (!source.includes("pub(crate) const OUTLINE_PROMPT_VERSION: i64 = 4;")) {
    throw new Error("Could not update outline prompt/schema constants");
  }
  return source;
});

await edit("src-tauri/src/document_checkpoints.rs", (source) => {
  const oldUse = "ai::document_intelligence::{DocumentOutline, DocumentTaskContext},";
  const newUse = "ai::document_intelligence::{\n        DocumentOutline, DocumentTaskContext, OUTLINE_PROMPT_VERSION, OUTLINE_SCHEMA_VERSION,\n    },";
  if (!source.includes(newUse)) {
    if (!source.includes(oldUse)) throw new Error("Could not find document intelligence import");
    source = source.replace(oldUse, newUse);
  }
  const oldWhere = `                   AND credential_epoch = ?6
                   AND extraction_backend = ?7"`;
  const newWhere = `                   AND credential_epoch = ?6
                   AND extraction_backend = ?7
                   AND schema_version = ?8
                   AND prompt_version = ?9"`;
  if (!source.includes(newWhere)) {
    if (!source.includes(oldWhere)) throw new Error("Could not find final outline cache WHERE clause");
    source = source.replace(oldWhere, newWhere);
  }
  const oldParams = `                    task.credential_epoch,
                    pdf_backend::active_backend_id(),
                ],`;
  const newParams = `                    task.credential_epoch,
                    pdf_backend::active_backend_id(),
                    OUTLINE_SCHEMA_VERSION,
                    OUTLINE_PROMPT_VERSION,
                ],`;
  if (!source.includes(newParams)) {
    const index = source.indexOf(oldParams);
    if (index < 0) throw new Error("Could not find final outline cache params");
    source = `${source.slice(0, index)}${newParams}${source.slice(index + oldParams.length)}`;
  }
  return source;
});

await edit("src-tauri/src/documents.rs", (source) => {
  const oldSql = `                 ) VALUES (?1, 1, 4, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)`;
  const newSql = `                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)`;
  if (!source.includes(newSql)) {
    if (!source.includes(oldSql)) throw new Error("Could not find outline INSERT values");
    source = source.replace(oldSql, newSql);
  }
  const oldParams = `                params![
                    document_id,
                    task.profile_id,
                    task.provider_kind,
                    task.model_id,
                    task.profile_revision,
                    task.credential_epoch,
                    pdf_backend::active_backend_id(),
                    outline_json,
                    outline.summary,
                    now,
                ],`;
  const newParams = `                params![
                    document_id,
                    document_intelligence::OUTLINE_SCHEMA_VERSION,
                    document_intelligence::OUTLINE_PROMPT_VERSION,
                    task.profile_id,
                    task.provider_kind,
                    task.model_id,
                    task.profile_revision,
                    task.credential_epoch,
                    pdf_backend::active_backend_id(),
                    outline_json,
                    outline.summary,
                    now,
                ],`;
  if (!source.includes(newParams)) {
    const index = source.indexOf(oldParams);
    if (index < 0) throw new Error("Could not find persist_outline params");
    source = `${source.slice(0, index)}${newParams}${source.slice(index + oldParams.length)}`;
  }
  return source;
});
