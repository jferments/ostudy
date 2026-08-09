#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function patch(relativePath, transform) {
  const path = fileURLToPath(new URL(`../${relativePath}`, import.meta.url));
  const source = await readFile(path, "utf8");
  const updated = transform(source);
  if (updated === source) throw new Error(`No changes applied to ${relativePath}`);
  await writeFile(path, updated, "utf8");
}

await patch("src-tauri/src/ai/document_intelligence.rs", (source) => {
  const search = `        let outline = serde_json::to_string(&outline_schema()).unwrap();`;
  if (!source.includes(search)) throw new Error("Could not find schema coverage test");
  return source.replace(
    search,
    `        let outline = serde_json::to_string(&outline_fragment_schema()).unwrap();`,
  );
});

await patch("src-tauri/src/document_operations.rs", (source) => {
  const search = `    statement
        .query_map([document_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
}`;
  const replacement = `    let rows = statement
        .query_map([document_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(CommandError::database)?;
    let decks = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(decks)
}`;
  if (!source.includes(search)) throw new Error("Could not find surviving deck query collection");
  return source.replace(search, replacement);
});
