#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const applicatorPath = join(root, "scripts", "apply-platform-p0-p2-final-docs.mjs");
let source = await readFile(applicatorPath, "utf8");

const oldAnchor = "  `.ostudybackup is the complete personal-library recovery format. It combines one coherent SQLite snapshot with the managed files required by that snapshot. Provider metadata may be backed up; credential bytes are excluded because they never enter SQLite.`,";
const newAnchor = "  `\\`.ostudybackup\\` is the complete personal-library recovery format. It combines one coherent SQLite snapshot with the managed files required by that snapshot. Provider metadata may be backed up; credential bytes are excluded because they never enter SQLite.`,";
const oldReplacement = "  `.ostudybackup is the complete personal-library recovery format. It combines one coherent SQLite snapshot with the managed files required by that snapshot. Backup format version 3 includes active persistent authoring drafts and their draft-owned staged images; version-1 and version-2 packages remain readable. Provider metadata may be backed up, but credential bytes are excluded because they never enter SQLite or draft payloads.`,";
const newReplacement = "  `\\`.ostudybackup\\` is the complete personal-library recovery format. It combines one coherent SQLite snapshot with the managed files required by that snapshot. Backup format version 3 includes active persistent authoring drafts and their draft-owned staged images; version-1 and version-2 packages remain readable. Provider metadata may be backed up, but credential bytes are excluded because they never enter SQLite or draft payloads.`,";

if (!source.includes(oldAnchor) || !source.includes(oldReplacement)) {
  throw new Error("The final documentation backup-format anchors were not found.");
}
source = source.replace(oldAnchor, newAnchor).replace(oldReplacement, newReplacement);
await writeFile(applicatorPath, source, "utf8");

await import(new URL("./apply-platform-p0-p2-final-docs.mjs", import.meta.url));
