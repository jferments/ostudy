#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const databasePath = join(root, "src-tauri", "src", "database.rs");
const database = await readFile(databasePath, "utf8");

const registrySource =
  database.split("const MIGRATIONS:")[1]?.split("pub(crate) const LATEST_SCHEMA_VERSION")[0] ?? "";
const migrations = [];
for (const match of registrySource.matchAll(/Migration\s*\{([\s\S]*?)\}/gu)) {
  const body = match[1];
  const version = Number(body.match(/version:\s*(\d+)/u)?.[1]);
  const name = body.match(/name:\s*"([^"]+)"/u)?.[1];
  const checksum = body.match(/checksum:\s*"([0-9a-f]{64})"/u)?.[1];
  const file = body.match(/include_str!\("\.\.\/migrations\/([^"]+)"\)/u)?.[1];
  if (!Number.isInteger(version) || !name || !checksum || !file) {
    throw new Error("A public migration registry entry is malformed.");
  }
  const sql = await readFile(join(root, "src-tauri", "migrations", file), "utf8");
  const actualChecksum = createHash("sha256").update(sql).digest("hex");
  if (actualChecksum !== checksum) {
    throw new Error(`Migration ${file} checksum mismatch: ${actualChecksum}`);
  }
  migrations.push({ version, name, checksum, file });
}

migrations.sort((left, right) => left.version - right.version);
if (migrations.length === 0) throw new Error("No public migrations are registered.");
for (let index = 0; index < migrations.length; index += 1) {
  const expectedVersion = index + 1;
  if (migrations[index].version !== expectedVersion) {
    throw new Error(`Public migration registry is not contiguous at version ${expectedVersion}.`);
  }
}

const latest = Number(database.match(/LATEST_SCHEMA_VERSION:\s*i64\s*=\s*(\d+)/u)?.[1]);
const maximum = migrations.at(-1).version;
if (latest !== maximum) {
  throw new Error(`LATEST_SCHEMA_VERSION ${latest} does not match migration ${maximum}.`);
}

const mode = process.argv[2] ?? "--json";
if (mode === "--latest") {
  process.stdout.write(`${latest}\n`);
} else if (mode === "--ledger") {
  process.stdout.write(
    `${migrations.map(({ version, name, checksum }) => `${version}|${name}|${checksum}`).join("\n")}\n`,
  );
} else if (mode === "--json") {
  process.stdout.write(`${JSON.stringify({ latestSchemaVersion: latest, migrations })}\n`);
} else {
  console.error("Usage: node ./scripts/current-public-migrations.mjs [--latest|--ledger|--json]");
  process.exit(2);
}
