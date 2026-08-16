import { invoke } from "@tauri-apps/api/core";
import type { PreparedExport } from "./externalFiles";

export interface PreparedLibraryBackup {
  preparedExport: PreparedExport;
  deckCount: number;
  templateCount: number;
  reviewCount: number;
  assetCount: number;
}

export interface LibraryRestoreResult {
  deckCount: number;
  templateCount: number;
  reviewCount: number;
  assetCount: number;
  restartRequired: boolean;
}

export async function prepareLibraryBackup(
  operationId: string,
): Promise<PreparedLibraryBackup> {
  return invoke<PreparedLibraryBackup>("prepare_library_backup_async", {
    input: { operationId },
  });
}

export async function prepareLibraryRestore(
  stagedFileId: string,
): Promise<LibraryRestoreResult> {
  return invoke<LibraryRestoreResult>("prepare_library_restore_async", {
    input: { stagedFileId },
  });
}
