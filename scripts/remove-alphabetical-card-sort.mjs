#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";

const path = "src-tauri/src/authoring/mod.rs";
let text = readFileSync(path, "utf8");

const validation = '"relevance" | "updated_desc" | "question_asc" | "due_asc" | "last_reviewed_desc"';
if (text.split(validation).length - 1 !== 1) {
  throw new Error("Expected exactly one alphabetical sort validation contract");
}
text = text.replace(
  validation,
  '"relevance" | "updated_desc" | "due_asc" | "last_reviewed_desc"',
);

const alphabeticalSql = `        "question_asc" => {
            "CASE WHEN n.note_type = 'basic' AND c.direction = 'reverse' THEN n.back_text ELSE n.front_text END COLLATE NOCASE ASC, n.updated_at DESC, c.template_ordinal ASC"
        }
`;
if (text.split(alphabeticalSql).length - 1 !== 1) {
  throw new Error("Expected exactly one alphabetical SQL sort branch");
}
text = text.replace(alphabeticalSql, "");
writeFileSync(path, text);

console.log("Removed alphabetical card sorting from the Rust search contract.");
