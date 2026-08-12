#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Normalize only the already-retired cleanup assumption. All source-contract
// replacements in the underlying applicator remain strict and fail closed.
const root = fileURLToPath(new URL("..", import.meta.url));
const applicatorPath = join(root, "scripts", "apply-platform-p2-atomic-native.mjs");
let source = await readFile(applicatorPath, "utf8");

const strictImportCleanup = `drafts = replaceExact(
  drafts,
  "use std::{collections::BTreeSet, path::PathBuf};",
  "use std::collections::BTreeSet;",
  "remove unused editor draft path import",
);`;
const tolerantImportCleanup = `drafts = drafts.replace(
  "use std::{collections::BTreeSet, path::PathBuf};",
  "use std::collections::BTreeSet;",
);`;
if (!source.includes(strictImportCleanup)) {
  throw new Error("The atomic applicator import-normalization block was not found.");
}
source = source.replace(strictImportCleanup, tolerantImportCleanup);
await writeFile(applicatorPath, source, "utf8");

await import(new URL("./apply-platform-p2-atomic-native.mjs", import.meta.url));
