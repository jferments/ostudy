#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const write = async (path, content) => {
  const target = join(root, path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, content, "utf8");
};
const replace = async (path, from, to) => {
  const source = await read(path);
  if (!source.includes(from)) throw new Error(`${path}: source fragment not found`);
  await write(path, source.replace(from, to));
};

const migration = await read("src-tauri/migrations/0025_durable_editor_drafts.sql");
const checksum = createHash("sha256").update(migration).digest("hex");
let database = await read("src-tauri/src/database.rs");
const anchor = `    Migration {
        version: 24,
        name: "external_file_transfers",
        checksum: "`;
const start = database.indexOf(anchor);
const end = database.indexOf("    },\n];", start);
if (start < 0 || end < 0) throw new Error("database migration 24 registry entry not found");
const entryEnd = end + "    },\n".length;
database = database.slice(0, entryEnd) + `    Migration {
        version: 25,
        name: "durable_editor_drafts",
        checksum: "${checksum}",
        sql: include_str!("../migrations/0025_durable_editor_drafts.sql"),
    },
` + database.slice(entryEnd);
database = database.replace("LATEST_SCHEMA_VERSION: i64 = 24", "LATEST_SCHEMA_VERSION: i64 = 25");
await write("src-tauri/src/database.rs", database);

let assets = await read("src-tauri/src/asset_lifecycle.rs");
assets = assets
  .replace(
    '"SELECT purpose, expires_at, committed_at, canceled_at\n             FROM asset_drafts WHERE id = ?1"',
    '"SELECT purpose, expires_at, committed_at, canceled_at, retention_mode\n             FROM asset_drafts WHERE id = ?1"',
  )
  .replace(
    "let row: Option<(String, i64, Option<i64>, Option<i64>)> = connection",
    "let row: Option<(String, i64, Option<i64>, Option<i64>, String)> = connection",
  )
  .replace(
    "|row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),",
    "|row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),",
  )
  .replace("Some((_, _, Some(_), _))", "Some((_, _, Some(_), _, _))")
  .replace("Some((_, _, _, Some(_)))", "Some((_, _, _, Some(_), _))")
  .replace(
    "Some((_, expires_at, None, None)) if expires_at < now",
    "Some((_, expires_at, None, None, retention_mode))\n            if retention_mode == \"ephemeral\" && expires_at < now",
  )
  .replace("Some((purpose, _, None, None))", "Some((purpose, _, None, None, _))")
  .replace(
    "WHERE committed_at IS NULL AND canceled_at IS NULL AND expires_at < ?1",
    "WHERE committed_at IS NULL AND canceled_at IS NULL\n                   AND retention_mode = 'ephemeral' AND expires_at < ?1",
  );
const wrapperAnchor = "fn retire_unused_draft_assets(\n";
const wrapperIndex = assets.indexOf(wrapperAnchor);
if (wrapperIndex < 0) throw new Error("asset retirement helper not found");
const wrappers = `pub(crate) fn retire_unused_editor_assets(
    connection: &Connection,
    paths: &AppPaths,
    draft_id: &str,
    retained: &BTreeSet<String>,
    now: i64,
    reason: &str,
) -> Result<Vec<PathBuf>, CommandError> {
    retire_unused_draft_assets(connection, paths, draft_id, retained, now, reason)
}

pub(crate) fn remove_editor_cleanup_paths(paths: &[PathBuf]) {
    remove_paths_best_effort(paths);
}

`;
assets = assets.slice(0, wrapperIndex) + wrappers + assets.slice(wrapperIndex);
await write("src-tauri/src/asset_lifecycle.rs", assets);

let drafts = await read("src-tauri/src/editor_drafts.rs");
drafts = drafts.replace(
  ".execute(\n                    \"UPDATE editor_drafts\n                     SET last_opened_at = ?1, updated_at = ?1\n                     WHERE id = ?2 AND revision = ?3 AND state = 'active'\",\n                    params![now, input.draft_id, input.revision],\n                )?",
  ".execute(\n                    \"UPDATE editor_drafts\n                     SET last_opened_at = ?1, updated_at = ?1\n                     WHERE id = ?2 AND revision = ?3 AND state = 'active'\",\n                    params![now, input.draft_id, input.revision],\n                )\n                .map_err(CommandError::database)?",
).replace(
  ".execute(\n                    \"UPDATE editor_drafts\n                     SET payload_schema_version = ?1, payload_json = ?2,\n                         payload_sha256 = ?3, revision = revision + 1,\n                         updated_at = ?4, last_opened_at = ?4\n                     WHERE id = ?5 AND revision = ?6 AND state = 'active'\",\n                    params![\n                        input.payload_schema_version,\n                        payload_json,\n                        payload_sha256,\n                        now,\n                        input.draft_id,\n                        input.revision,\n                    ],\n                )?",
  ".execute(\n                    \"UPDATE editor_drafts\n                     SET payload_schema_version = ?1, payload_json = ?2,\n                         payload_sha256 = ?3, revision = revision + 1,\n                         updated_at = ?4, last_opened_at = ?4\n                     WHERE id = ?5 AND revision = ?6 AND state = 'active'\",\n                    params![\n                        input.payload_schema_version,\n                        payload_json,\n                        payload_sha256,\n                        now,\n                        input.draft_id,\n                        input.revision,\n                    ],\n                )\n                .map_err(CommandError::database)?",
);
await write("src-tauri/src/editor_drafts.rs", drafts);

let lib = await read("src-tauri/src/lib.rs");
lib = lib
  .replace("mod error;\n", "mod editor_drafts;\nmod error;\n")
  .replace(
    "            external_files::reconcile_startup(&database, &paths)",
    "            external_files::reconcile_startup(&database, &paths)\n                .map_err(|error| io::Error::other(error.message))?;\n            editor_drafts::reconcile_startup(&database)",
  )
  .replace(
    "            external_files::release_prepared_export,",
    "            external_files::release_prepared_export,\n            editor_drafts::open_editor_draft,\n            editor_drafts::save_editor_draft,\n            editor_drafts::discard_editor_draft,\n            editor_drafts::commit_editor_draft,\n            editor_drafts::list_editor_drafts,",
  );
await write("src-tauri/src/lib.rs", lib);

let backup = await read("src-tauri/src/library_backup.rs");
backup = backup
  .replace("const BACKUP_VERSION: i64 = 2;", "const BACKUP_VERSION: i64 = 3;")
  .replace(
    "SELECT COUNT(*) FROM asset_drafts\n                 WHERE committed_at IS NULL AND canceled_at IS NULL",
    "SELECT COUNT(*) FROM asset_drafts\n                 WHERE committed_at IS NULL AND canceled_at IS NULL\n                   AND retention_mode = 'ephemeral'",
  );
const oldQuery = `            "SELECT id, relative_path, mime_type, byte_size
             FROM assets
             WHERE lifecycle_state = 'ready' AND deleted_at IS NULL
             ORDER BY id",`;
const newQuery = `            "SELECT a.id,
                    CASE
                      WHEN a.lifecycle_state = 'ready' THEN a.relative_path
                      ELSE a.staged_relative_path
                    END,
                    a.mime_type, a.byte_size
             FROM assets a
             LEFT JOIN asset_drafts ad ON ad.id = a.draft_id
             LEFT JOIN editor_drafts ed ON ed.id = ad.editor_draft_id
             WHERE a.deleted_at IS NULL
               AND (
                 a.lifecycle_state = 'ready'
                 OR (
                   a.lifecycle_state = 'staged'
                   AND ad.retention_mode = 'persistent_editor'
                   AND ad.committed_at IS NULL AND ad.canceled_at IS NULL
                   AND ed.state IN ('active', 'conflicted')
                 )
               )
             ORDER BY a.id",`;
if (!backup.includes(oldQuery)) throw new Error("backup asset inventory query not found");
backup = backup.replace(oldQuery, newQuery);
await write("src-tauri/src/library_backup.rs", backup);

await rm(join(root, "scripts", "apply-platform-p2-drafts-native.mjs"), { force: true });
await rm(join(root, ".github", "workflows", "apply-platform-p2-drafts-native.yml"), { force: true });
console.log("Applied durable native drafts, migration 25, and backup v3.");
