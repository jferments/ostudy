import {
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
