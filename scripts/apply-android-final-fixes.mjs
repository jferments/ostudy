#!/usr/bin/env node

import { readdir, readFile, writeFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");

function replaceExact(text, before, after, label) {
  if (!text.includes(before)) throw new Error(`${label}: expected source not found`);
  return text.replace(before, after);
}

let platform = await read("src-tauri/src/platform/mod.rs");
if (!platform.includes("pub mime_types: Vec<String>")) {
  platform = replaceExact(
    platform,
    `pub struct ExternalFileRequest {\n    pub title: String,\n    pub suggested_filename: Option<String>,\n    pub filters: Vec<FileFilter>,\n}`,
    `pub struct ExternalFileRequest {\n    pub title: String,\n    pub suggested_filename: Option<String>,\n    pub filters: Vec<FileFilter>,\n    pub mime_types: Vec<String>,\n}`,
    "add native MIME filters to external-file requests",
  );
}
await write("src-tauri/src/platform/mod.rs", platform);

for (const path of await rustFiles(join(root, "src-tauri", "src"))) {
  let source = await readFile(path, "utf8");
  let cursor = 0;
  let changed = false;
  while (true) {
    const start = source.indexOf("ExternalFileRequest {", cursor);
    if (start < 0) break;
    const next = source.indexOf("ExternalFileRequest {", start + 1);
    const filters = source.indexOf("filters:", start);
    if (filters < 0 || (next >= 0 && filters > next)) {
      throw new Error(`${relative(root, path)}: ExternalFileRequest has no filters field`);
    }
    const prefix = source.slice(start, filters);
    if (!prefix.includes("mime_types:")) {
      const lineStart = source.lastIndexOf("\n", filters) + 1;
      const indentation = source.slice(lineStart, filters);
      source =
        source.slice(0, lineStart) +
        `${indentation}mime_types: Vec::new(),\n` +
        source.slice(lineStart);
      changed = true;
      cursor = filters + indentation.length + 28;
    } else {
      cursor = filters + 8;
    }
  }
  if (changed) await writeFile(path, source, "utf8");
}

let external = await read("src-tauri/src/external_files.rs");
external = external
  .replace(
    `title: "Select an image".to_string(),\n                suggested_filename: None,\n                mime_types: Vec::new(),`,
    `title: "Select an image".to_string(),\n                suggested_filename: None,\n                mime_types: vec!["image/*".to_string()],`,
  )
  .replace(
    `title: "Select a PDF".to_string(),\n                suggested_filename: None,\n                mime_types: Vec::new(),`,
    `title: "Select a PDF".to_string(),\n                suggested_filename: None,\n                mime_types: vec!["application/pdf".to_string()],`,
  )
  .replace(
    `title: "Select an ostudy deck or generated JSON file".to_string(),\n                suggested_filename: None,\n                mime_types: Vec::new(),`,
    `title: "Select an ostudy deck or generated JSON file".to_string(),\n                suggested_filename: None,\n                mime_types: vec![\n                    "application/vnd.ostudy.studydeck".to_string(),\n                    "application/json".to_string(),\n                    "application/octet-stream".to_string(),\n                ],`,
  )
  .replace(
    `title: "Select an ostudy library backup".to_string(),\n                suggested_filename: None,\n                mime_types: Vec::new(),`,
    `title: "Select an ostudy library backup".to_string(),\n                suggested_filename: None,\n                mime_types: vec![\n                    "application/vnd.ostudy.backup".to_string(),\n                    "application/octet-stream".to_string(),\n                ],`,
  );
await write("src-tauri/src/external_files.rs", external);

let overlays = await read("src/ui/overlays/OverlayProvider.tsx");
overlays = replaceExact(
  overlays,
  `  useEffect(\n    () =>\n      registerPlatformBackHandler(() => {\n        const top = resolveTopOverlay(entriesRef.current);\n        if (!top) return false;\n        return top.requestClose("platform_back");\n      }),\n    [],\n  );`,
  `  useEffect(() => {\n    if (topId === null) return;\n    return registerPlatformBackHandler(() => {\n      const top = resolveTopOverlay(entriesRef.current);\n      if (!top) return false;\n      return top.requestClose("platform_back");\n    });\n  }, [topId]);`,
  "enable native Back only while an overlay can consume it",
);
await write("src/ui/overlays/OverlayProvider.tsx", overlays);

console.log("Applied final Android MIME, request-contract, and Back-stack fixes.");

async function rustFiles(directory) {
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await rustFiles(path)));
    else if (entry.isFile() && entry.name.endsWith(".rs")) output.push(path);
  }
  return output;
}
