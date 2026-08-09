#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function edit(relative, transform) {
  const path = fileURLToPath(new URL(`../${relative}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated !== source) await writeFile(path, updated, "utf8");
}

await edit("src-tauri/src/document_checkpoints.rs", (source) => {
  if (source.includes("pub(crate) fn load_final_outline(")) return source;
  const marker = "pub(crate) fn outline_contract_key";
  const index = source.indexOf(marker);
  if (index < 0) throw new Error("Could not find outline contract function");
  const helper = `pub(crate) fn load_final_outline(\n    database: &Database,\n    document_id: &str,\n    task: &DocumentTaskContext,\n) -> Result<Option<DocumentOutline>, CommandError> {\n    let outline_json = database.with_connection(|connection| {\n        connection\n            .query_row(\n                \"SELECT outline_json\n                 FROM document_outlines\n                 WHERE document_id = ?1\n                   AND profile_id = ?2\n                   AND provider_kind = ?3\n                   AND model_id = ?4\n                   AND profile_revision = ?5\n                   AND credential_epoch = ?6\n                   AND extraction_backend = ?7\",\n                params![\n                    document_id,\n                    task.profile_id,\n                    task.provider_kind,\n                    task.model_id,\n                    task.profile_revision,\n                    task.credential_epoch,\n                    pdf_backend::active_backend_id(),\n                ],\n                |row| row.get::<_, String>(0),\n            )\n            .optional()\n            .map_err(CommandError::database)\n    })?;\n    let Some(json) = outline_json else {\n        return Ok(None);\n    };\n    match serde_json::from_str::<DocumentOutline>(&json) {\n        Ok(outline) => Ok(Some(outline)),\n        Err(_) => {\n            crate::logging::info(\n                \"document_final_outline_cache_invalid\",\n                format!(\"document={document_id}\"),\n            );\n            Ok(None)\n        }\n    }\n}\n\n`;
  return `${source.slice(0, index)}${helper}${source.slice(index)}`;
});

await edit("src-tauri/src/documents.rs", (source) => {
  if (source.includes("document_final_outline_reused")) return source;
  const startMarker = "    let outline_contract = document_checkpoints::outline_contract_key(&task, plan);";
  const endMarker = "\n    let generation_contract = document_checkpoints::generation_contract_key(";
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start);
  if (start < 0 || end < 0) throw new Error("Could not locate outline pipeline block");
  const replacement = `    let outline_contract = document_checkpoints::outline_contract_key(&task, plan);\n    let reusable_final_outline =\n        document_checkpoints::load_final_outline(database.inner(), document_id, &task)?;\n    let outline = if let Some(outline) = reusable_final_outline {\n        initialize_document_ai_plan(\n            database.inner(),\n            document_id,\n            plan,\n            estimated_source_tokens,\n            outline_windows.len(),\n            outline_windows.len(),\n            generation_windows.len(),\n            0,\n        )?;\n        crate::logging::info(\n            \"document_final_outline_reused\",\n            format!(\n                \"document={} provider={} model={} sections={}\",\n                document_id,\n                task.provider_kind,\n                task.model_id,\n                outline.sections.len(),\n            ),\n        );\n        outline\n    } else {\n        let cached_outline = document_checkpoints::load_outline_fragments(\n            database.inner(),\n            document_id,\n            &outline_contract,\n            &outline_windows,\n        )?;\n        let outline_completed = cached_outline\n            .iter()\n            .filter(|value| value.is_some())\n            .count();\n        initialize_document_ai_plan(\n            database.inner(),\n            document_id,\n            plan,\n            estimated_source_tokens,\n            outline_windows.len(),\n            outline_completed,\n            generation_windows.len(),\n            0,\n        )?;\n        mark_outline_building(database.inner(), document_id)?;\n        let outline = document_intelligence::build_document_outline(\n            runtime.inner(),\n            &task,\n            &stored.dto.title,\n            i64::try_from(pages.len()).unwrap_or(i64::MAX),\n            &outline_windows,\n            &cached_outline,\n            |window, fragment| {\n                document_checkpoints::persist_outline_fragment(\n                    database.inner(),\n                    document_id,\n                    &outline_contract,\n                    window,\n                    fragment,\n                )?;\n                let completed = document_checkpoints::count_outline_checkpoints(\n                    database.inner(),\n                    document_id,\n                    &outline_contract,\n                )?;\n                update_outline_progress(database.inner(), document_id, completed)?;\n                Ok(())\n            },\n        )\n        .await?;\n        update_outline_progress(\n            database.inner(),\n            document_id,\n            i64::try_from(outline_windows.len()).unwrap_or(i64::MAX),\n        )?;\n        persist_outline(database.inner(), document_id, &task, &outline)?;\n        outline\n    };\n`;
  return `${source.slice(0, start)}${replacement}${source.slice(end)}`;
});
