#!/usr/bin/env node

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

const generatorPath = "scripts/apply-platform-p2-files-foundation.mjs";
let generator = await read(generatorPath);
generator = generator.replace("`ExternalFilePort`", "'ExternalFilePort'");
const pathsStart = generator.indexOf("let pathsWithDirs = paths.replace(");
const pathsEnd = generator.indexOf("const pathMethods = `", pathsStart);
if (pathsStart < 0 || pathsEnd < 0) {
  throw new Error("P2 generator paths transformation was not found.");
}
const correctedPathsTransformation = String.raw`let pathsWithDirs = paths.replace(
  "        let document_staging_dir = documents_dir.join(\"staging\");\n        std::fs::create_dir_all(&images_dir)?;\n        std::fs::create_dir_all(&asset_staging_dir)?;\n        std::fs::create_dir_all(&asset_quarantine_dir)?;\n        std::fs::create_dir_all(&documents_dir)?;\n        std::fs::create_dir_all(&document_staging_dir)?;",
  "        let document_staging_dir = documents_dir.join(\"staging\");\n        let transfer_imports_dir = app_data_dir.join(\"transfers\").join(\"imports\");\n        let transfer_exports_dir = app_data_dir.join(\"transfers\").join(\"exports\");\n        std::fs::create_dir_all(&images_dir)?;\n        std::fs::create_dir_all(&asset_staging_dir)?;\n        std::fs::create_dir_all(&asset_quarantine_dir)?;\n        std::fs::create_dir_all(&documents_dir)?;\n        std::fs::create_dir_all(&document_staging_dir)?;\n        std::fs::create_dir_all(&transfer_imports_dir)?;\n        std::fs::create_dir_all(&transfer_exports_dir)?;",
);
if (pathsWithDirs === paths) throw new Error("paths.rs: directory block not found");
`;
generator =
  generator.slice(0, pathsStart) + correctedPathsTransformation + generator.slice(pathsEnd);
await write(generatorPath, generator);

await import(new URL("./apply-platform-p2-files-foundation.mjs", import.meta.url));

await write(
  "src/platform/files.ts",
  `import {
  open as openNativeDialog,
  save as saveNativeDialog,
} from "@tauri-apps/plugin-dialog";

import {
  publishPreparedExport,
  selectAndStageExternalImport,
  type ExternalImportPurpose,
  type PublishReceipt,
  type StagedExternalFile,
} from "../ipc/externalFiles";

export interface FileFilter {
  name: string;
  extensions: string[];
}

export interface OpenFileOptions {
  title?: string;
  multiple?: boolean;
  directory?: boolean;
  filters?: FileFilter[];
  defaultPath?: string;
}

export interface SaveFileOptions {
  title?: string;
  filters?: FileFilter[];
  defaultPath?: string;
}

/** Temporary desktop compatibility path while P2 feature call sites are migrated. */
export async function openFile(
  options: OpenFileOptions,
): Promise<string | string[] | null> {
  return openNativeDialog(options);
}

/** Temporary desktop compatibility path while P2 feature call sites are migrated. */
export async function saveFile(options: SaveFileOptions): Promise<string | null> {
  return saveNativeDialog(options);
}

export function selectExternalImport(
  purpose: ExternalImportPurpose,
): Promise<StagedExternalFile | null> {
  return selectAndStageExternalImport(purpose);
}

export function publishExport(preparedExportId: string): Promise<PublishReceipt | null> {
  return publishPreparedExport(preparedExportId);
}
`,
);

let paths = await read("src-tauri/src/paths.rs");
if (!paths.includes("pub fn transfer_imports_dir")) {
  const methods = String.raw`

    pub fn transfer_imports_dir(&self) -> PathBuf {
        self.app_data_dir.join("transfers").join("imports")
    }

    pub fn transfer_exports_dir(&self) -> PathBuf {
        self.app_data_dir.join("transfers").join("exports")
    }

    pub fn staged_import_partial_relative_path(id: &str) -> String {
        format!("transfers/imports/{id}.partial")
    }

    pub fn staged_import_relative_path(id: &str, extension: &str) -> String {
        format!("transfers/imports/{id}.{extension}")
    }

    pub fn prepared_export_relative_path(id: &str, extension: &str) -> String {
        format!("transfers/exports/{id}.{extension}")
    }
`;
  const close = paths.lastIndexOf("\n}");
  if (close < 0) throw new Error("paths.rs: impl closing brace not found");
  paths = paths.slice(0, close) + methods + paths.slice(close);
  await write("src-tauri/src/paths.rs", paths);
}

let external = await read("src-tauri/src/external_files.rs");
external = external
  .replace(
    "path::{Component, Path, PathBuf}",
    "path::{Path, PathBuf}",
  )
  .replace(
    "paths.resolve_relative(\n            AppPaths::staged_import_partial_relative_path",
    "paths.resolve_relative(\n            &AppPaths::staged_import_partial_relative_path",
  )
  .replace(
    "paths.resolve_relative(AppPaths::staged_import_partial_relative_path(&id))",
    "paths.resolve_relative(&AppPaths::staged_import_partial_relative_path(&id))",
  )
  .replace(
    "remove_unknown_transfer_files(paths.transfer_imports_dir(), &referenced)?;",
    "remove_unknown_transfer_files(&paths.transfer_imports_dir(), &referenced)?;",
  )
  .replace(
    "remove_unknown_transfer_files(paths.transfer_exports_dir(), &referenced)?;",
    "remove_unknown_transfer_files(&paths.transfer_exports_dir(), &referenced)?;",
  )
  .replace("    ensure_managed_path(path)?;\n", "")
  .replace(
    `            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?
`,
    `            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            rows
`,
  )
  .replace(
    `            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?
`,
    `            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .map_err(CommandError::database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(CommandError::database)?;
            rows
`,
  );
const unsafeHelperStart = external.indexOf("fn ensure_managed_path(path: &Path)");
if (unsafeHelperStart >= 0) {
  const nextHelper = external.indexOf("fn create_parent", unsafeHelperStart);
  if (nextHelper < 0) throw new Error("external_files.rs: helper boundary not found");
  external = external.slice(0, unsafeHelperStart) + external.slice(nextHelper);
}
await write("src-tauri/src/external_files.rs", external);

await rm(join(root, "scripts", "run-platform-p2-foundation.mjs"), { force: true });
console.log("Prepared and applied the qualified P2 external-file foundation.");
