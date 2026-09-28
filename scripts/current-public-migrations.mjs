#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const contractPath = join(root, "scripts", "generated", "database-contract.json");
const contract = JSON.parse(await readFile(contractPath, "utf8"));

if (contract?.schemaVersion !== 1 || !contract.public || !Array.isArray(contract.public.migrations)) {
  throw new Error("The generated native database contract is missing or malformed.");
}

const migrations = [...contract.public.migrations].sort((left, right) => left.version - right.version);
if (migrations.length === 0) throw new Error("No public migrations are registered.");

for (let index = 0; index < migrations.length; index += 1) {
  const migration = migrations[index];
  const expectedVersion = index + 1;
  if (
    migration.version !== expectedVersion ||
    typeof migration.name !== "string" ||
    !/^[0-9a-f]{64}$/u.test(migration.checksum ?? "") ||
    typeof migration.sqlFile !== "string"
  ) {
    throw new Error(`Public migration contract is malformed at version ${expectedVersion}.`);
  }
  const expectedFile = `${String(migration.version).padStart(4, "0")}_${migration.name}.sql`;
  if (migration.sqlFile !== expectedFile) {
    throw new Error(`Public migration ${migration.version} has an unexpected SQL filename.`);
  }
  const sql = await readFile(join(root, "src-tauri", "migrations", migration.sqlFile), "utf8");
  const actualChecksum = createHash("sha256").update(sql).digest("hex");
  if (actualChecksum !== migration.checksum) {
    throw new Error(`Migration ${migration.sqlFile} checksum mismatch: ${actualChecksum}`);
  }
}

const latest = contract.public.latestSchemaVersion;
const maximum = migrations.at(-1).version;
if (latest !== maximum || contract.public.legacyPublicSchemaVersion !== maximum) {
  throw new Error(
    `Generated public schema versions do not match terminal migration ${maximum}.`,
  );
}

const mode = process.argv[2] ?? "--json";
if (mode === "--latest") {
  process.stdout.write(`${latest}\n`);
} else if (mode === "--ledger") {
  process.stdout.write(
    `${migrations
      .map(({ version, name, checksum }) => `${version}|${name}|${checksum}`)
      .join("\n")}\n`,
  );
} else if (mode === "--json") {
  process.stdout.write(`${JSON.stringify({ latestSchemaVersion: latest, migrations })}\n`);
} else {
  console.error("Usage: node ./scripts/current-public-migrations.mjs [--latest|--ledger|--json]");
  process.exit(2);
}
