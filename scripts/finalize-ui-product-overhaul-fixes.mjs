#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";

const mode = process.argv[2];
if (!mode || !["before", "after"].includes(mode)) throw new Error("expected before or after mode");

if (mode === "before") {
  const path = "scripts/finalize-ui-product-overhaul.mjs";
  let text = readFileSync(path, "utf8");
  const guard = `  if (text.indexOf(before, first + before.length) >= 0) {\n    throw new Error(\`${"${path}"}: source fragment was not unique\`);\n  }\n`;
  if (!text.includes(guard)) throw new Error("finalizer uniqueness guard not found");
  text = text.replace(guard, "");
  writeFileSync(path, text);
  console.log("Relaxed finalizer uniqueness guard; known duplicate output is repaired after application.");
}

if (mode === "after") {
  const documentsPath = "src-tauri/src/documents.rs";
  let documents = readFileSync(documentsPath, "utf8");
  const oldFailure = "                     safe_error_code = ?1, safe_error_message = ?2,\\n                     updated_at = ?3, revision = revision + 1\\n";
  const newFailure = "                     safe_error_code = ?1, safe_error_message = ?2,\\n                     processing_duration_ms = CASE\\n                         WHEN processing_started_at IS NULL THEN processing_duration_ms\\n                         ELSE MAX(0, ?3 - processing_started_at)\\n                     END,\\n                     updated_at = ?3, revision = revision + 1\\n";
  if (documents.includes(oldFailure)) documents = documents.replace(oldFailure, newFailure);
  writeFileSync(documentsPath, documents);

  const scopePath = "src-tauri/src/analytics/scope.rs";
  let scope = readFileSync(scopePath, "utf8");
  const previousBlock = `        let previous = current\n            .as_ref()\n            .map(|window| time.adjacent_previous_window(window))\n            .transpose()?;\n        Ok(Self {\n`;
  const fixedPrevious = `        let previous = current\n            .as_ref()\n            .map(|window| time.adjacent_previous_window(window))\n            .transpose()?;\n        let end_local_date = current\n            .as_ref()\n            .map_or(time.local_date, |window| window.end_local_date);\n        Ok(Self {\n`;
  if (!scope.includes(previousBlock)) throw new Error("analytics previous-window block not found");
  scope = scope.replace(previousBlock, fixedPrevious);
  const movedField = `                end_local_date: current\n                    .as_ref()\n                    .map_or(time.local_date, |window| window.end_local_date),\n`;
  if (!scope.includes(movedField)) throw new Error("analytics moved end-date field not found");
  scope = scope.replace(movedField, "                end_local_date,\n");
  writeFileSync(scopePath, scope);
  console.log("Applied post-finalizer duplicate and ownership fixes.");
}
