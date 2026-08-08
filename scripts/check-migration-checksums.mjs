#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFile, readdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const migrationsDirectory = join(root, "src-tauri", "migrations");
const database = await readFile(join(root, "src-tauri", "src", "database.rs"), "utf8");
const migrationFiles = (await readdir(migrationsDirectory))
  .filter((name) => /^\d{4}_.+\.sql$/u.test(name))
  .sort();

let failed = false;
for (const name of migrationFiles) {
  const version = Number(name.slice(0, 4));
  const checksum = createHash("sha256")
    .update(await readFile(join(migrationsDirectory, name)))
    .digest("hex");
  const entry = [...database.matchAll(/Migration\s*\{([\s\S]*?)\},/gu)]
    .map((match) => match[1])
    .find((body) => Number(body.match(/version:\s*(\d+)/u)?.[1]) === version);
  const registered = entry?.match(/checksum:\s*"([0-9a-f]{64})"/u)?.[1];
  if (registered !== checksum) {
    failed = true;
    console.error(`${name}: ${registered ?? "missing"} -> ${checksum}`);
  }
}

if (failed) process.exit(1);
console.log(`OK   ${migrationFiles.length} migration checksums match the registry.`);
