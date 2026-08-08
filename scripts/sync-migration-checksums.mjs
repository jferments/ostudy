#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const migrationsDirectory = join(root, "src-tauri", "migrations");
const databasePath = join(root, "src-tauri", "src", "database.rs");
const write = process.argv.includes("--write");

const migrationFiles = (await readdir(migrationsDirectory))
  .filter((name) => /^\d{4}_.+\.sql$/u.test(name))
  .sort();
let database = await readFile(databasePath, "utf8");
const mismatches = [];

for (const name of migrationFiles) {
  const version = Number(name.slice(0, 4));
  const sql = await readFile(join(migrationsDirectory, name));
  const checksum = createHash("sha256").update(sql).digest("hex");
  const blockPattern = new RegExp(
    `(Migration\\s*\\{[\\s\\S]*?version:\\s*${version},[\\s\\S]*?checksum:\\s*")([0-9a-f]{64})("[\\s\\S]*?include_str!\\(\\"\\.\\./migrations/${escapeRegExp(name)}\\"\\)[\\s\\S]*?\\})`,
    "u",
  );
  const match = database.match(blockPattern);
  if (!match) {
    throw new Error(`Migration registry entry not found for ${name}.`);
  }
  if (match[2] === checksum) continue;
  mismatches.push({ name, version, expected: checksum, actual: match[2] });
  if (write) database = database.replace(blockPattern, `$1${checksum}$3`);
}

if (write && mismatches.length > 0) await writeFile(databasePath, database, "utf8");

if (mismatches.length === 0) {
  console.log(`OK   ${migrationFiles.length} migration checksums match the registry.`);
  process.exit(0);
}

for (const mismatch of mismatches) {
  console.error(
    `${basename(mismatch.name)}: registry ${mismatch.actual} != file ${mismatch.expected}`,
  );
}
if (write) {
  console.log(`Updated ${mismatches.length} migration checksum(s) in src-tauri/src/database.rs.`);
  process.exit(0);
}
console.error("Run: node ./scripts/sync-migration-checksums.mjs --write");
process.exit(1);

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
}
