#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const path = fileURLToPath(
  new URL("../src-tauri/src/documents.rs", import.meta.url),
);
let source = await readFile(path, "utf8");

function replaceOnce(label, before, after) {
  const first = source.indexOf(before);
  if (first < 0) {
    throw new Error(`Could not find ${label} in documents.rs`);
  }
  if (source.indexOf(before, first + before.length) >= 0) {
    throw new Error(`${label} occurred more than once in documents.rs`);
  }
  source = `${source.slice(0, first)}${after}${source.slice(first + before.length)}`;
}

replaceOnce(
  "startup reconciliation query collection",
  `        statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)`,
  `        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(rows)`,
);

replaceOnce(
  "managed PDF ID generation",
  "    let document_id = database.with_connection(generate_id)?;",
  "    let document_id = database.with_connection(|connection| generate_id(connection))?;",
);

replaceOnce(
  "document list query collection",
  `    statement
        .query_map(
            params![
                if include_archived { 1_i64 } else { 0_i64 },
                MAX_DOCUMENTS_RETURNED
            ],
            document_row,
        )
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)`,
  `    let rows = statement
        .query_map(
            params![
                if include_archived { 1_i64 } else { 0_i64 },
                MAX_DOCUMENTS_RETURNED
            ],
            document_row,
        )
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)`,
);

await writeFile(path, source, "utf8");
