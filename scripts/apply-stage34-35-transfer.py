#!/usr/bin/env python3
"""Wire the canonical studydeck pipeline and complete-library backup UI once."""

from __future__ import annotations

import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


# Native dependencies for SQLite snapshot backup and package fingerprints.
cargo_path = "src-tauri/Cargo.toml"
cargo = read(cargo_path)
cargo = replace_once(
    cargo,
    'rusqlite = { version = "=0.40.1", default-features = false, features = ["bundled"] }',
    'rusqlite = { version = "=0.40.1", default-features = false, features = ["bundled", "backup"] }',
    "rusqlite backup feature",
)
cargo = replace_once(
    cargo,
    'serde_json = "=1.0.145"\n',
    'serde_json = "=1.0.145"\nsha2 = "=0.10.9"\n',
    "sha2 dependency",
)
write(cargo_path, cargo)

# Replace the historical layered transfer composition with one command facade
# and apply pending complete-library restores before SQLite opens.
lib_path = "src-tauri/src/lib.rs"
lib = read(lib_path)
for old in [
    "mod stage3_transfer;\n",
    "mod tagged_transfer;\n",
    "mod transfer;\n",
    "mod transfer_lifecycle;\n",
]:
    lib = lib.replace(old, "")
lib = replace_once(lib, "mod logging;\n", "mod library_backup;\nmod logging;\n", "backup module")
lib = replace_once(lib, "mod settings;\n", "mod settings;\nmod studydeck;\n", "studydeck module")
lib = replace_once(
    lib,
    '''            let paths = AppPaths::new(app_data_directory).map_err(io::Error::other)?;
            let database = Database::open(paths.database_path()).map_err(io::Error::other)?;

            deletion::finalize_expired_deletions(&database)''',
    '''            let paths = AppPaths::new(app_data_directory).map_err(io::Error::other)?;
            let pending_restore =
                library_backup::apply_pending_restore(&paths).map_err(io::Error::other)?;
            let database = Database::open(paths.database_path()).map_err(io::Error::other)?;
            if let Some(restore) = pending_restore {
                restore.commit();
            }

            deletion::finalize_expired_deletions(&database)''',
    "pending restore startup",
)
lib = replace_once(
    lib,
    '''            settings::get_app_info,
            practice_selection::get_practice_workload,''',
    '''            settings::get_app_info,
            library_backup::export_library_backup,
            library_backup::prepare_library_restore,
            practice_selection::get_practice_workload,''',
    "backup commands",
)
lib = replace_once(
    lib,
    '''            analytics::get_analytics,
            analytics::get_recent_practice_reviews,
            stage3_transfer::export_deck_with_card_types,
            authoring::import_deck_with_single_cloze_normalization
''',
    '''            analytics::get_analytics,
            analytics::get_recent_practice_reviews,
            studydeck::export_deck,
            studydeck::import_deck
''',
    "studydeck commands",
)
write(lib_path, lib)

# The old transfer wrapper was the only authoring include dedicated to package
# orchestration. Generated-card move/copy remains in stage3/transfer.rs.
stage3_entry_path = "src-tauri/src/authoring/stage3_v2.rs"
stage3_entry = read(stage3_entry_path)
stage3_entry = stage3_entry.replace('include!("stage3_v2/transfer_wrapper.rs");\n', "")
stage3_entry = stage3_entry.replace(
    "structural rendering, compatibility normalization, and transfer wrappers all",
    "structural rendering and compatibility normalization",
)
write(stage3_entry_path, stage3_entry)

# Frontend contracts: one import operation identity and complete-library backup.
ipc_path = "src/ipc.ts"
ipc = read(ipc_path)
ipc = replace_once(
    ipc,
    '''export interface ImportResult {
  deck: Deck;
  cardCount: number;
  assetCount: number;
}
''',
    '''export interface ImportResult {
  deck: Deck;
  cardCount: number;
  assetCount: number;
}

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
''',
    "backup DTOs",
)
ipc = replace_once(
    ipc,
    '''export async function importDeck(sourcePath: string): Promise<ImportResult> {
  return invoke<ImportResult>("import_deck", { sourcePath });
}
''',
    '''export async function importDeck(
  sourcePath: string,
  operationId: string,
): Promise<ImportResult> {
  return invoke<ImportResult>("import_deck", {
    input: { sourcePath, operationId },
  });
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
''',
    "backup IPC functions",
)
write(ipc_path, ipc)

# Retain an import operation ID only after an ambiguous transport result.
decks_path = "src/screens/DecksScreen.tsx"
decks = read(decks_path)
decks = replace_once(
    decks,
    '''  exportDeck,
  getDeck,
  getErrorMessage,
  importDeck,''',
    '''  createOperationId,
  exportDeck,
  getDeck,
  getErrorCode,
  getErrorMessage,
  importDeck,''',
    "deck IPC imports",
)
decks = replace_once(
    decks,
    '''  const transferringRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);''',
    '''  const transferringRef = useRef(false);
  const pendingImportRef = useRef<{ sourcePath: string; operationId: string } | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);''',
    "pending import ref",
)
decks = replace_once(
    decks,
    '''      if (normalizedPath.endsWith(".studydeck")) {
        const result = await importDeck(selected);
        await showImportedDeck(''',
    '''      if (normalizedPath.endsWith(".studydeck")) {
        const pending =
          pendingImportRef.current?.sourcePath === selected
            ? pendingImportRef.current
            : { sourcePath: selected, operationId: createOperationId() };
        pendingImportRef.current = pending;
        let result;
        try {
          result = await importDeck(selected, pending.operationId);
          pendingImportRef.current = null;
        } catch (error: unknown) {
          if (getErrorCode(error) !== null) pendingImportRef.current = null;
          throw error;
        }
        await showImportedDeck(''',
    "retry-safe deck import",
)
write(decks_path, decks)

# Storage UI for exact full-library backup and restart-safe restore preparation.
settings_path = "src/screens/SettingsScreen.tsx"
settings = read(settings_path)
settings = replace_once(
    settings,
    'import { useEffect, useRef, useState } from "react";\n',
    'import { useEffect, useRef, useState } from "react";\nimport { open, save } from "@tauri-apps/plugin-dialog";\n',
    "settings dialogs",
)
settings = replace_once(
    settings,
    '''  getAppInfo,
  getErrorMessage,
  saveSettings,''',
    '''  exportLibraryBackup,
  getAppInfo,
  getErrorMessage,
  prepareLibraryRestore,
  saveSettings,''',
    "settings backup imports",
)
settings = replace_once(
    settings,
    '''  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);''',
    '''  const [isSaving, setIsSaving] = useState(false);
  const [isTransferringLibrary, setIsTransferringLibrary] = useState(false);
  const [restorePrepared, setRestorePrepared] = useState(false);
  const savingRef = useRef(false);''',
    "settings backup state",
)
settings = replace_once(
    settings,
    '''  async function persist(): Promise<void> {
''',
    '''  async function backupLibrary(): Promise<void> {
    if (isTransferringLibrary) return;
    const destination = await save({
      title: "Back up the complete ostudy library",
      defaultPath: `ostudy-library-${new Date().toISOString().slice(0, 10)}.ostudybackup`,
      filters: [{ name: "ostudy library backup", extensions: ["ostudybackup"] }],
    });
    if (typeof destination !== "string") return;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    try {
      const result = await exportLibraryBackup(destination);
      showToast(
        "success",
        `Backed up ${result.deckCount} ${result.deckCount === 1 ? "deck" : "decks"}, ${result.templateCount} ${result.templateCount === 1 ? "template" : "templates"}, and ${result.reviewCount} review ${result.reviewCount === 1 ? "event" : "events"}.`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsTransferringLibrary(false);
    }
  }

  async function restoreLibrary(): Promise<void> {
    if (isTransferringLibrary) return;
    const source = await open({
      title: "Choose an ostudy library backup",
      multiple: false,
      directory: false,
      filters: [{ name: "ostudy library backup", extensions: ["ostudybackup"] }],
    });
    if (typeof source !== "string") return;
    if (!window.confirm(
      "Restore this complete library on the next ostudy launch? The current library will be replaced after validation.",
    )) return;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    try {
      const result = await prepareLibraryRestore(source);
      setRestorePrepared(result.restartRequired);
      showToast(
        "info",
        `Restore prepared for ${result.deckCount} ${result.deckCount === 1 ? "deck" : "decks"}. Close and reopen ostudy to apply it.`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsTransferringLibrary(false);
    }
  }

  async function persist(): Promise<void> {
''',
    "settings backup handlers",
)
settings = replace_once(
    settings,
    '''              <PathRow label="Schema" value={String(appInfo.databaseSchemaVersion)} />
            </dl>''',
    '''              <PathRow label="Schema" value={String(appInfo.databaseSchemaVersion)} />
            </dl>
          ) : (
            <div className="inline-progress">Reading storage paths…</div>
          )}
          <div className="storage-backup-actions">
            <button
              type="button"
              className="secondary-button"
              disabled={isTransferringLibrary}
              onClick={() => void backupLibrary()}
            >
              {isTransferringLibrary ? "Working…" : "Back up library"}
            </button>
            <button
              type="button"
              className="secondary-button"
              disabled={isTransferringLibrary}
              onClick={() => void restoreLibrary()}
            >
              Restore backup
            </button>
          </div>
          <p className="settings-note">
            Complete backups include every deck, reusable template, managed image,
            schedule, review event, practice session, analytics source record, and setting.
            Shared .studydeck files remain content-only and receive fresh schedules when imported.
          </p>
          {restorePrepared ? (
            <div className="inline-alert info-alert">
              Restore is prepared. Close every ostudy window and reopen the application to apply it.
            </div>
          ) : null}
        </section>
      </div>
    </div>
  );
}

interface ThemeOptionProps {
''',
    "settings storage actions",
)
# The replacement above intentionally consumes the old Storage tail and the
# component boundary. Remove the duplicate old tail left after the insertion.
settings = re.sub(
    r'''\n          \) : \(\n            <div className="inline-progress">Reading storage paths…</div>\n          \)\}\n        </section>\n      </div>\n    </div>\n  \);\n\}\n\ninterface ThemeOptionProps \{\n''',
    "\n",
    settings,
    count=1,
)
write(settings_path, settings)

# Stage 3 verification now asserts the canonical package rather than the removed
# optional metadata wrapper.
verify3_path = "scripts/verify-stage3.mjs"
verify3 = read(verify3_path)
verify3 = verify3.replace(
    'read("src-tauri", "src", "stage3_transfer.rs")',
    'read("src-tauri", "src", "studydeck.rs")',
)
verify3 = verify3.replace(
    '''  "CREATE TABLE IF NOT EXISTS card_type_metadata",
  "restore_imported_card_types",
  "insert_new_schedule",''',
    '''  "CURRENT_PACKAGE_VERSION",
  "fresh-local-schedules",
  "insert_new_schedule",''',
)
verify3 = verify3.replace(
    'if (stage3Transfer.includes("card_templates")) {',
    'if (stage3Transfer.includes("CREATE TABLE card_templates")) {',
)
write(verify3_path, verify3)

# Project verifier follows the active facade instead of a removed legacy module.
manifest_path = "scripts/verify-project-manifests.mjs"
manifest = read(manifest_path)
manifest = manifest.replace(
    'read("src-tauri", "src", "transfer.rs")',
    'read("src-tauri", "src", "studydeck.rs")',
)
write(manifest_path, manifest)

# Add focused behavioral wiring checks to the fast gate.
check_fast_path = "scripts/check-fast.sh"
check_fast = read(check_fast_path)
check_fast = replace_once(
    check_fast,
    'node "$SCRIPT_DIR/verify-stage33.mjs"\n',
    'node "$SCRIPT_DIR/verify-stage33.mjs"\nnode "$SCRIPT_DIR/verify-stage34-35.mjs"\n',
    "stage34/35 verifier",
)
write(check_fast_path, check_fast)

# Remove the superseded public/package wrapper layers. Version-1 adaptation now
# lives in the canonical reader rather than in runtime export decorators.
for relative in [
    "src-tauri/src/transfer.rs",
    "src-tauri/src/stage3_transfer.rs",
    "src-tauri/src/tagged_transfer.rs",
    "src-tauri/src/transfer_lifecycle.rs",
    "src-tauri/src/authoring/stage3_v2/transfer_wrapper.rs",
]:
    path = ROOT / relative
    if path.exists():
        path.unlink()
legacy_dir = ROOT / "src-tauri/src/tagged_transfer"
if legacy_dir.exists():
    shutil.rmtree(legacy_dir)

print("Applied Stage 3.4 transfer, backup, and UI wiring.")
