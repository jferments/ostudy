#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const applicatorPath = join(root, "scripts", "apply-platform-p2-backup-v3.mjs");
let source = await readFile(applicatorPath, "utf8");

const replacements = [
  ['"fn validate_snapshot_database(\\n"', '"fn validate_snapshot_database("'],
  ['"fn validate_managed_asset_path(\\n"', '"fn validate_managed_asset_path("'],
  ['"fn replace_file(\\n"', '"fn replace_file("'],
];
for (const [before, after] of replacements) {
  if (!source.includes(before)) {
    throw new Error(`The backup applicator boundary ${before} was not found.`);
  }
  source = source.replace(before, after);
}
await writeFile(applicatorPath, source, "utf8");
await import(new URL("./apply-platform-p2-backup-v3.mjs", import.meta.url));
