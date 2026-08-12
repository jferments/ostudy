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
const replace = async (path, from, to) => {
  const source = await read(path);
  if (!source.includes(from)) throw new Error(`${path}: source fragment not found`);
  await write(path, source.replace(from, to));
};

await write("src/ipc/externalFiles.ts", `import { invoke } from "@tauri-apps/api/core";
import { createOperationId } from "./common";

export type ExternalImportPurpose =
  | "image"
  | "pdf_document"
  | "study_content"
  | "library_backup";

export type ExternalFileKind =
  | "png"
  | "jpeg"
  | "gif"
  | "webp"
  | "pdf"
  | "studydeck"
  | "generated_json"
  | "ostudybackup";

export interface StagedExternalFile {
  id: string;
  purpose: ExternalImportPurpose;
  detectedKind: ExternalFileKind;
  displayName: string;
  mediaType: string;
  byteSize: number;
  sha256: string;
  expiresAt: number;
}

export interface PreparedExport {
  id: string;
  purpose: "studydeck" | "library_backup";
  suggestedFilename: string;
  mediaType: string;
  byteSize: number;
  sha256: string;
  expiresAt: number;
}

export interface PublishReceipt {
  preparedExportId: string;
  displayName: string;
  destinationKind: string;
  completedAt: number;
}

export async function selectAndStageExternalImport(
  purpose: ExternalImportPurpose,
  operationId = createOperationId(),
): Promise<StagedExternalFile | null> {
  return invoke<StagedExternalFile | null>("select_and_stage_external_import", {
    input: { operationId, purpose },
  });
}

export async function releaseStagedExternalImport(stagedFileId: string): Promise<void> {
  return invoke<void>("release_staged_external_import", { input: { stagedFileId } });
}

export async function publishPreparedExport(
  preparedExportId: string,
): Promise<PublishReceipt | null> {
  return invoke<PublishReceipt | null>("publish_prepared_export", {
    input: { preparedExportId },
  });
}

export async function releasePreparedExport(preparedExportId: string): Promise<void> {
  return invoke<void>("release_prepared_export", { input: { preparedExportId } });
}
`);

await write("src/ipc/assets.ts", `import { invoke } from "@tauri-apps/api/core";

export interface Asset {
  id: string;
  originalFilename: string;
  mimeType: string;
  byteSize: number;
  createdAt: number;
}

export interface AssetDraft {
  id: string;
  purpose: "card" | "template" | "import";
  expiresAt: number;
}

export async function beginAssetDraft(
  purpose: AssetDraft["purpose"],
  ownerId?: string,
): Promise<AssetDraft> {
  return invoke<AssetDraft>("begin_asset_draft", { input: { purpose, ownerId } });
}

export async function cancelAssetDraft(draftId: string): Promise<void> {
  return invoke<void>("cancel_asset_draft", { draftId });
}

export async function syncAssetDraft(draftId: string, assetIds: string[]): Promise<void> {
  return invoke<void>("sync_asset_draft", { input: { draftId, assetIds } });
}

export async function importImage(stagedFileId: string, draftId: string): Promise<Asset> {
  return invoke<Asset>("import_image", { input: { stagedFileId, draftId } });
}
`);

await write("src/ipc/decks.ts", `import { invoke } from "@tauri-apps/api/core";

import type { DeletionResult } from "./common";
import type { PreparedExport } from "./externalFiles";

export interface DeckSummary {
  id: string;
  name: string;
  description: string;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableDueCount: number;
  archivedAt: number | null;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface CreateDeckInput {
  name: string;
  description?: string;
}

export interface UpdateDeckInput {
  deckId: string;
  name: string;
  description?: string;
  revision: number;
}

export interface SetDeckArchivedInput {
  deckId: string;
  revision: number;
  archived: boolean;
}

export interface DeckCardSummaryQuery {
  deckId: string;
  query: string;
  sort: "created_desc" | "practiced_desc" | "practiced_asc";
  limit: number;
  offset: number;
}

export interface DeckCardSummary {
  noteId: string;
  primaryCardId: string;
  questionExcerpt: string;
  answerExcerpt: string;
  generatedCardCount: number;
  imageCount: number;
  reversible: boolean;
  updatedAt: number;
}

export interface DeckCardSummaryPage {
  cards: DeckCardSummary[];
  totalCount: number;
  offset: number;
  hasMore: boolean;
}

export interface PreparedDeckExport {
  preparedExport: PreparedExport;
  cardCount: number;
  assetCount: number;
}

export interface ImportResult {
  deck: DeckSummary;
  cardCount: number;
  assetCount: number;
}

export async function listDecks(): Promise<DeckSummary[]> {
  return invoke<DeckSummary[]>("list_decks");
}

export async function getDeck(deckId: string): Promise<DeckSummary> {
  return invoke<DeckSummary>("get_deck", { deckId });
}

export async function createDeck(input: CreateDeckInput): Promise<DeckSummary> {
  return invoke<DeckSummary>("create_deck", { input });
}

export async function updateDeck(input: UpdateDeckInput): Promise<DeckSummary> {
  return invoke<DeckSummary>("update_deck", { input });
}

export async function setDeckArchived(input: SetDeckArchivedInput): Promise<DeckSummary> {
  return invoke<DeckSummary>("set_deck_archived", { input });
}

export async function deleteDeck(deckId: string): Promise<DeletionResult> {
  return invoke<DeletionResult>("delete_deck", { deckId });
}

export async function listDeckCardSummaries(
  input: DeckCardSummaryQuery,
): Promise<DeckCardSummaryPage> {
  return invoke<DeckCardSummaryPage>("list_deck_card_summaries", { input });
}

export async function prepareDeckExport(
  deckId: string,
  operationId: string,
): Promise<PreparedDeckExport> {
  return invoke<PreparedDeckExport>("prepare_deck_export", {
    input: { deckId, operationId },
  });
}

export async function importDeck(
  stagedFileId: string,
  operationId: string,
): Promise<ImportResult> {
  return invoke<ImportResult>("import_deck", {
    input: { stagedFileId, operationId },
  });
}
`);

await write("src/ipc/backup.ts", `import { invoke } from "@tauri-apps/api/core";
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
  return invoke<PreparedLibraryBackup>("prepare_library_backup", {
    input: { operationId },
  });
}

export async function prepareLibraryRestore(
  stagedFileId: string,
): Promise<LibraryRestoreResult> {
  return invoke<LibraryRestoreResult>("prepare_library_restore", {
    input: { stagedFileId },
  });
}
`);

await write("src/ipc/generated-import.ts", `import { invoke } from "@tauri-apps/api/core";

export interface FlashcardImportResult {
  deckId: string;
  createdNoteCount: number;
  createdCardCount: number;
  skippedDuplicateCount: number;
}

export async function importGeneratedFlashcards(
  stagedFileId: string,
): Promise<FlashcardImportResult> {
  return invoke<FlashcardImportResult>("import_generated_flashcards", {
    input: { stagedFileId },
  });
}
`);

await replace(
  "src/ipc/documents.ts",
  "export async function importPdfAndGenerateFlashcards(\n  sourcePath: string,\n): Promise<DocumentGenerationResult> {",
  "export async function importPdfAndGenerateFlashcards(\n  stagedFileId: string,\n): Promise<DocumentGenerationResult> {",
);
await replace(
  "src/ipc/documents.ts",
  "    input: { sourcePath },",
  "    input: { stagedFileId },",
);

await write("src/platform/files.ts", `import {
  publishPreparedExport,
  selectAndStageExternalImport,
  type ExternalImportPurpose,
  type PublishReceipt,
  type StagedExternalFile,
} from "../ipc/externalFiles";

export function selectExternalImport(
  purpose: ExternalImportPurpose,
): Promise<StagedExternalFile | null> {
  return selectAndStageExternalImport(purpose);
}

export function publishExport(preparedExportId: string): Promise<PublishReceipt | null> {
  return publishPreparedExport(preparedExportId);
}
`);

await write("src/platform/assets.ts", `import { convertFileSrc } from "@tauri-apps/api/core";

export function managedAssetUrl(assetId: string): string {
  return convertFileSrc(assetId, "ostudy-asset");
}
`);

let richText = await read("src/editor/richText.ts");
richText = richText
  .replace('import { convertFileSrc } from "@tauri-apps/api/core";\n\n', '')
  .replace(
    'import {\n  type Asset,\n} from "../ipc/assets";',
    'import {\n  type Asset,\n} from "../ipc/assets";\nimport { managedAssetUrl } from "../platform/assets";',
  )
  .replace("node.attrs.src = convertFileSrc(asset.filePath);", "node.attrs.src = managedAssetUrl(asset.id);");
await write("src/editor/richText.ts", richText);

for (const editorPath of ["src/editor/RichTextEditor.tsx", "src/editor/StudyRichEditor.tsx"]) {
  let source = await read(editorPath);
  source = source
    .replace('import { convertFileSrc } from "@tauri-apps/api/core";\n', '')
    .replace('import { openFile } from "../platform/files";', 'import { managedAssetUrl } from "../platform/assets";\nimport { selectExternalImport } from "../platform/files";')
    .replace(/const selected = await openFile\(\{[\s\S]*?\n\s*\}\);\n\s*if \(typeof selected !== "string"\) return;/u, 'const selected = await selectExternalImport("image");\n      if (!selected) return;')
    .replace("const asset = await importImage(selected, assetDraftId);", "const asset = await importImage(selected.id, assetDraftId);")
    .replace("src: convertFileSrc(asset.filePath),", "src: managedAssetUrl(asset.id),");
  await write(editorPath, source);
}

let decksController = await read("src/controllers/decksController.ts");
decksController = decksController
  .replace("  exportDeck,", "  prepareDeckExport,")
  .replace("  type ExportResult,", "  type PreparedDeckExport,")
  .replace(
    "  export(deckId: string, destinationPath: string): Promise<ExportResult> {\n    return exportDeck(deckId, destinationPath);\n  }",
    "  prepareExport(deckId: string, operationId: string): Promise<PreparedDeckExport> {\n    return prepareDeckExport(deckId, operationId);\n  }",
  )
  .replace(
    "  importStudydeck(sourcePath: string, operationId: string): Promise<ImportResult> {\n    return importDeck(sourcePath, operationId);\n  }",
    "  importStudydeck(stagedFileId: string, operationId: string): Promise<ImportResult> {\n    return importDeck(stagedFileId, operationId);\n  }",
  )
  .replace(
    "  importGenerated(sourcePath: string): Promise<FlashcardImportResult> {\n    return importGeneratedFlashcards(sourcePath);\n  }",
    "  importGenerated(stagedFileId: string): Promise<FlashcardImportResult> {\n    return importGeneratedFlashcards(stagedFileId);\n  }",
  );
await write("src/controllers/decksController.ts", decksController);

let decksScreen = await read("src/screens/DecksScreen.tsx");
decksScreen = decksScreen
  .replace('import { openFile, saveFile } from "../platform/files";', 'import { publishExport, selectExternalImport } from "../platform/files";')
  .replace(
    "const pendingImportRef = useRef<{ sourcePath: string; operationId: string } | null>(null);",
    "const pendingImportRef = useRef<{ stagedFileId: string; operationId: string } | null>(null);",
  );
const exportStart = decksScreen.indexOf("  async function handleExport(deck: DeckSummary): Promise<void> {");
const exportEnd = decksScreen.indexOf("\n  async function showImportedDeck", exportStart);
if (exportStart < 0 || exportEnd < 0) throw new Error("DecksScreen export function not found");
decksScreen = decksScreen.slice(0, exportStart) + `  async function handleExport(deck: DeckSummary): Promise<void> {
    if (transferringRef.current) return;
    transferringRef.current = true;
    setIsTransferring(true);
    try {
      const prepared = await controller.prepareExport(deck.id, createOperationId());
      const receipt = await publishExport(prepared.preparedExport.id);
      if (!receipt) return;
      showToast(
        "success",
        \`Exported \${prepared.cardCount} \${prepared.cardCount === 1 ? "card" : "cards"}.\`,
      );
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      transferringRef.current = false;
      setIsTransferring(false);
    }
  }
` + decksScreen.slice(exportEnd);
const importStart = decksScreen.indexOf("  async function handleImport(): Promise<void> {");
const importEnd = decksScreen.indexOf("\n  if (view === \"editor\"", importStart);
if (importStart < 0 || importEnd < 0) throw new Error("DecksScreen import function not found");
decksScreen = decksScreen.slice(0, importStart) + `  async function handleImport(): Promise<void> {
    if (transferringRef.current) return;
    transferringRef.current = true;
    setIsTransferring(true);
    try {
      const selected = await selectExternalImport("study_content");
      if (!selected) return;

      if (selected.detectedKind === "studydeck") {
        const pending =
          pendingImportRef.current?.stagedFileId === selected.id
            ? pendingImportRef.current
            : { stagedFileId: selected.id, operationId: createOperationId() };
        pendingImportRef.current = pending;
        let result;
        try {
          result = await controller.importStudydeck(selected.id, pending.operationId);
          pendingImportRef.current = null;
        } catch (error: unknown) {
          if (getErrorCode(error) !== null) pendingImportRef.current = null;
          throw error;
        }
        await showImportedDeck(
          result.deck.id,
          \`Imported “\${result.deck.name}” with \${result.cardCount} \${
            result.cardCount === 1 ? "card" : "cards"
          }.\`,
          result.deck,
        );
        return;
      }

      if (selected.detectedKind === "generated_json") {
        const result = await controller.importGenerated(selected.id);
        const duplicateMessage =
          result.skippedDuplicateCount > 0
            ? \` \${result.skippedDuplicateCount} exact \${
                result.skippedDuplicateCount === 1 ? "duplicate was" : "duplicates were"
              } skipped.\`
            : "";
        await showImportedDeck(
          result.deckId,
          \`Imported \${result.createdNoteCount} \${
            result.createdNoteCount === 1 ? "card entry" : "card entries"
          } as \${result.createdCardCount} \${
            result.createdCardCount === 1 ? "scheduled card" : "scheduled cards"
          }.\${duplicateMessage}\`,
        );
        return;
      }

      throw new Error("Select an ostudy deck or generated-card file.");
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      transferringRef.current = false;
      setIsTransferring(false);
    }
  }
` + decksScreen.slice(importEnd);
await write("src/screens/DecksScreen.tsx", decksScreen);

let documentsScreen = await read("src/screens/DocumentsScreen.tsx");
documentsScreen = documentsScreen
  .replace('import { openFile } from "../platform/files";', 'import { selectExternalImport } from "../platform/files";')
  .replace(/    const selected = await openFile\(\{[\s\S]*?\n    \}\);\n    if \(typeof selected !== "string"\) return;/u, '    const selected = await selectExternalImport("pdf_document");\n    if (!selected) return;')
  .replace("const result = await importPdfAndGenerateFlashcards(selected);", "const result = await importPdfAndGenerateFlashcards(selected.id);");
await write("src/screens/DocumentsScreen.tsx", documentsScreen);

let storage = await read("src/screens/settings/StorageBackupSettings.tsx");
storage = storage
  .replace('import { exportLibraryBackup, prepareLibraryRestore } from "../../ipc/backup";', 'import { prepareLibraryBackup, prepareLibraryRestore } from "../../ipc/backup";')
  .replace('import { getErrorMessage } from "../../ipc/common";', 'import { createOperationId, getErrorMessage } from "../../ipc/common";')
  .replace('import { openFile, saveFile } from "../../platform/files";', 'import { publishExport, selectExternalImport } from "../../platform/files";');
const backupStart = storage.indexOf("  async function backupLibrary(): Promise<void> {");
const restoreStart = storage.indexOf("\n  async function restoreLibrary", backupStart);
if (backupStart < 0 || restoreStart < 0) throw new Error("Storage backup function not found");
storage = storage.slice(0, backupStart) + `  async function backupLibrary(): Promise<void> {
    if (isTransferringLibrary) return;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    try {
      const prepared = await prepareLibraryBackup(createOperationId());
      const receipt = await publishExport(prepared.preparedExport.id);
      if (!receipt) return;
      showToast(
        "success",
        \`Backed up \${prepared.deckCount} \${prepared.deckCount === 1 ? "deck" : "decks"}, \${prepared.templateCount} \${prepared.templateCount === 1 ? "template" : "templates"}, and \${prepared.reviewCount} review \${prepared.reviewCount === 1 ? "event" : "events"}.\`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsTransferringLibrary(false);
    }
  }
` + storage.slice(restoreStart);
const restoreFunctionStart = storage.indexOf("  async function restoreLibrary(): Promise<void> {");
const restoreFunctionEnd = storage.indexOf("\n  return (", restoreFunctionStart);
if (restoreFunctionStart < 0 || restoreFunctionEnd < 0) throw new Error("Storage restore function not found");
storage = storage.slice(0, restoreFunctionStart) + `  async function restoreLibrary(): Promise<void> {
    if (isTransferringLibrary) return;
    const source = await selectExternalImport("library_backup");
    if (!source) return;
    const accepted = await confirm({
      title: "Restore this complete library?",
      description:
        "The current library will be replaced after validation when ostudy next starts. This includes decks, templates, schedules, review history, managed images, analytics sources, AI provider metadata, task defaults, and settings. API credentials are never included.",
      confirmLabel: "Prepare restore",
      destructive: true,
    });
    if (!accepted) return;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    try {
      const result = await prepareLibraryRestore(source.id);
      setRestorePrepared(result.restartRequired);
      showToast(
        "info",
        \`Restore prepared for \${result.deckCount} \${result.deckCount === 1 ? "deck" : "decks"}. Close and reopen ostudy to apply it.\`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsTransferringLibrary(false);
    }
  }
` + storage.slice(restoreFunctionEnd);
await write("src/screens/settings/StorageBackupSettings.tsx", storage);

let contract = await read("src/ipc/domain-contracts.test.ts");
contract = contract
  .replace("  exportLibraryBackup,", "  prepareLibraryBackup,")
  .replace("  exportDeck,", "  prepareDeckExport,")
  .replace('() => importImage("/tmp/image.png", draftId)', '() => importImage("1".repeat(32), draftId)')
  .replace('{ input: { filePath: "/tmp/image.png", draftId } }', '{ input: { stagedFileId: "1".repeat(32), draftId } }')
  .replace(/await expectInvocation\(\n\s*\(\) => exportDeck\([\s\S]*?\n\s*\);/u, `await expectInvocation(
      () => prepareDeckExport(deckId, importOperationId),
      "prepare_deck_export",
      { input: { deckId, operationId: importOperationId } },
    );`)
  .replace('() => importDeck("/tmp/deck.studydeck", importOperationId)', '() => importDeck("1".repeat(32), importOperationId)')
  .replace('{ input: { sourcePath: "/tmp/deck.studydeck", operationId: importOperationId } }', '{ input: { stagedFileId: "1".repeat(32), operationId: importOperationId } }')
  .replace(/await expectInvocation\(\n\s*\(\) => exportLibraryBackup\([\s\S]*?\n\s*\);/u, `await expectInvocation(
      () => prepareLibraryBackup(importOperationId),
      "prepare_library_backup",
      { input: { operationId: importOperationId } },
    );`)
  .replace('() => prepareLibraryRestore("/tmp/library.ostudybackup")', '() => prepareLibraryRestore("1".repeat(32))')
  .replace('{ input: { sourcePath: "/tmp/library.ostudybackup" } }', '{ input: { stagedFileId: "1".repeat(32) } }');
await write("src/ipc/domain-contracts.test.ts", contract);

const exceptionPath = "scripts/policy/platform-boundary-exceptions.json";
const exceptions = JSON.parse(await read(exceptionPath));
const retiredRules = new Set([
  "frontend-direct-tauri-core-import",
  "frontend-raw-path-contract",
  "rust-raw-path-contract",
]);
exceptions.exceptions = exceptions.exceptions.filter((entry) => !retiredRules.has(entry.rule));
await write(exceptionPath, `${JSON.stringify(exceptions, null, 2)}\n`);

await rm(join(root, "scripts", "apply-platform-p2-pathfree-frontend.mjs"), { force: true });
await rm(join(root, ".github", "workflows", "apply-platform-p2-pathfree-frontend.yml"), { force: true });
console.log("Applied path-free frontend workflows and retired legacy path contracts.");
