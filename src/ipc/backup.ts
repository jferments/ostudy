import { invoke } from "@tauri-apps/api/core";

export interface LibraryBackupResult {
  destinationPath: string;
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

export async function exportLibraryBackup(
  destinationPath: string,
): Promise<LibraryBackupResult> {
  return invoke<LibraryBackupResult>("export_library_backup", {
    input: { destinationPath },
  });
}

export async function prepareLibraryRestore(
  sourcePath: string,
): Promise<LibraryRestoreResult> {
  return invoke<LibraryRestoreResult>("prepare_library_restore", {
    input: { sourcePath },
  });
}
