#!/usr/bin/env node

import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (...parts) => readFile(join(root, ...parts), "utf8");
const write = (path, text) => writeFile(join(root, path), text, "utf8");
const id = '"1".repeat(32)';
const staged = (kind, purpose, name) =>
  `{ id: ${id}, purpose: "${purpose}", detectedKind: "${kind}", displayName: "${name}", mediaType: "application/octet-stream", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 }`;

function insertAfterImports(text, snippet) {
  const matches = [...text.matchAll(/^import[\s\S]*?;\n/gmu)];
  const last = matches.at(-1);
  if (!last || last.index === undefined) {
    throw new Error("Could not find an import block for durable-draft test mocking.");
  }
  const insertion = last.index + last[0].length;
  return text.slice(0, insertion) + snippet + text.slice(insertion);
}

let factories = await read("src", "test", "factories.ts");
factories = factories.replace(/^\s*filePath:\s*[^\n]+\n/gmu, "");
await write("src/test/factories.ts", factories);

const durableMock = (specifier) => `\nvi.mock("${specifier}", () => ({\n  useDurableDraft: () => ({\n    draftId: "d".repeat(32),\n    assetDraftId: "7".repeat(32),\n    revision: 1,\n    status: "saved",\n    errorMessage: null,\n    recovered: false,\n    hasUnsavedLocalChanges: false,\n    flush: vi.fn().mockResolvedValue(undefined),\n    discard: vi.fn().mockResolvedValue(undefined),\n    commit: vi.fn().mockResolvedValue(undefined),\n    restart: vi.fn().mockResolvedValue(undefined),\n  }),\n}));\n`;

async function patchTests(directory) {
  for (const entry of await readdir(join(root, directory), { withFileTypes: true })) {
    const relative = join(directory, entry.name).replaceAll("\\", "/");
    if (entry.isDirectory()) {
      await patchTests(relative);
      continue;
    }
    if (!/\.(?:test|spec)\.(?:ts|tsx)$/u.test(entry.name)) continue;
    let text = await readFile(join(root, relative), "utf8");
    const original = text;

    text = text
      .replaceAll("exportLibraryBackup", "prepareLibraryBackup")
      .replaceAll("exportDeck", "prepareDeckExport")
      .replaceAll("platformFiles.openFile", "platformFiles.selectExternalImport")
      .replaceAll("platformFiles.saveFile", "platformFiles.publishExport")
      .replaceAll("openFile: vi.fn()", "selectExternalImport: vi.fn()")
      .replaceAll("saveFile: vi.fn()", "publishExport: vi.fn()")
      .replaceAll("filePath: \"/tmp/ostudy/diagram.png\",", "")
      .replaceAll("filePath: \"/tmp/image.png\",", "")
      .replaceAll('importImage("/tmp/image.png", draftId)', `importImage(${id}, draftId)`)
      .replaceAll('{ filePath: "/tmp/image.png", draftId }', `{ stagedFileId: ${id}, draftId }`)
      .replaceAll('importDeck("/tmp/deck.studydeck", importOperationId)', `importDeck(${id}, importOperationId)`)
      .replaceAll('{ sourcePath: "/tmp/deck.studydeck", operationId: importOperationId }', `{ stagedFileId: ${id}, operationId: importOperationId }`)
      .replaceAll('prepareLibraryRestore("/tmp/library.ostudybackup")', `prepareLibraryRestore(${id})`)
      .replaceAll('{ sourcePath: "/tmp/library.ostudybackup" }', `{ stagedFileId: ${id} }`)
      .replace(/importGeneratedFlashcards\("\/tmp\/[^"\n]+"\)/gu, `importGeneratedFlashcards(${id})`)
      .replace(/importPdfAndGenerateFlashcards\("\/tmp\/[^"\n]+"\)/gu, `importPdfAndGenerateFlashcards(${id})`)
      .replace(/input:\s*\{\s*sourcePath:\s*"\/tmp\/[^"\n]+"\s*\}/gu, `input: { stagedFileId: ${id} }`);

    text = text.replace(
      /expect\(invoke\)\.toHaveBeenCalledWith\("import_generated_flashcards",\s*\{\s*sourcePath:\s*"\/tmp\/[^"\n]+",?\s*\}\);/gu,
      `expect(invoke).toHaveBeenCalledWith("import_generated_flashcards", { input: { stagedFileId: ${id} } });`,
    );

    text = text.replace(
      /mockResolvedValue\("\/tmp\/[^"\n]+\.studydeck"\)/gu,
      `mockResolvedValue(${staged("studydeck", "study_content", "deck.studydeck")})`,
    );
    text = text.replace(
      /mockResolvedValue\("\/tmp\/[^"\n]+(?:\.ostudy)?\.json"\)/gu,
      `mockResolvedValue(${staged("generated_json", "study_content", "cards.ostudy.json")})`,
    );
    text = text.replace(
      /mockResolvedValue\("\/tmp\/[^"\n]+\.pdf"\)/gu,
      `mockResolvedValue(${staged("pdf", "pdf_document", "document.pdf")})`,
    );
    text = text.replace(
      /mockResolvedValue\("\/tmp\/[^"\n]+\.ostudybackup"\)/gu,
      `mockResolvedValue(${staged("ostudybackup", "library_backup", "library.ostudybackup")})`,
    );

    text = text.replace(
      /^(\s*)vi\.mocked\((?:decksIpc\.)?prepareDeckExport\)\.mockResolvedValue\([^\n]*\);\s*$/gmu,
      '$1vi.mocked(decksIpc.prepareDeckExport).mockResolvedValue({ preparedExport: { id: "2".repeat(32), purpose: "studydeck", suggestedFilename: "deck.studydeck", mediaType: "application/vnd.ostudy.studydeck", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 }, cardCount: 3, assetCount: 1 });',
    );
    text = text.replace(
      /^(\s*)vi\.mocked\((?:backupIpc\.)?prepareLibraryBackup\)\.mockResolvedValue\([^\n]*\);\s*$/gmu,
      '$1vi.mocked(backupIpc.prepareLibraryBackup).mockResolvedValue({ preparedExport: { id: "2".repeat(32), purpose: "library_backup", suggestedFilename: "library.ostudybackup", mediaType: "application/vnd.ostudy.backup", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 }, deckCount: 3, templateCount: 2, reviewCount: 41, assetCount: 4 });',
    );
    text = text.replace(
      /^(\s*)vi\.mocked\(platformFiles\.publishExport\)\.mockResolvedValue\([^\n]*\);\s*$/gmu,
      '$1vi.mocked(platformFiles.publishExport).mockResolvedValue({ preparedExportId: "2".repeat(32), displayName: "export", destinationKind: "desktop_file", completedAt: Date.now() });',
    );

    if (
      (text.includes("CardEditorScreen") ||
        text.includes("TemplatesScreen") ||
        text.includes("SettingsScreen")) &&
      !text.includes('vi.mock("../controllers/useDurableDraft"')
    ) {
      text = insertAfterImports(text, durableMock("../controllers/useDurableDraft"));
    }
    if (
      text.includes("BulkEntryModal") &&
      !text.includes('vi.mock("../../controllers/useDurableDraft"')
    ) {
      text = insertAfterImports(text, durableMock("../../controllers/useDurableDraft"));
    }

    if (text !== original) await write(relative, text);
  }
}

await patchTests("src");

let domain = await read("src", "ipc", "domain-contracts.test.ts");
domain = domain
  .replace(
    /await expectInvocation\(\n\s*\(\) => prepareDeckExport\([\s\S]*?\n\s*\);/u,
    `await expectInvocation(() => prepareDeckExport(deckId, importOperationId), "prepare_deck_export", { input: { deckId, operationId: importOperationId } });`,
  )
  .replace(
    /await expectInvocation\(\n\s*\(\) => prepareLibraryBackup\([\s\S]*?\n\s*\);/u,
    `await expectInvocation(() => prepareLibraryBackup(importOperationId), "prepare_library_backup", { input: { operationId: importOperationId } });`,
  );
await write("src/ipc/domain-contracts.test.ts", domain);

console.log("Updated tests for opaque file handles and durable draft hooks.");
