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

await import(new URL("./apply-platform-p2-pathfree-frontend.mjs", import.meta.url));

for (const editorPath of [
  "src/editor/RichTextEditor.tsx",
  "src/editor/StudyRichEditor.tsx",
  "src/editor/richText.ts",
]) {
  let source = await read(editorPath);
  source = source.replace('import { convertFileSrc } from "@tauri-apps/api/core";\n', "");
  if (!source.includes('from "../platform/assets"')) {
    const anchor = source.includes('from "../ipc/common";')
      ? 'from "../ipc/common";'
      : 'from "../ipc/assets";';
    source = source.replace(anchor, `${anchor}\nimport { managedAssetUrl } from "../platform/assets";`);
  }
  source = source.replaceAll("convertFileSrc(asset.filePath)", "managedAssetUrl(asset.id)");
  await write(editorPath, source);
}

let factories = await read("src/test/factories.ts");
factories = factories.replace('    filePath: "/tmp/ostudy/diagram.png",\n', "");
await write("src/test/factories.ts", factories);

await write(
  "src/editor/richText.test.ts",
  `import { beforeEach, describe, expect, it, vi } from "vitest";

import { managedAssetUrl } from "../platform/assets";
import { makeAsset, paragraphDocument } from "../test/factories";
import {
  canonicalizeDocument,
  cloneDocument,
  collectAssetIds,
  hydrateDocument,
  isDocumentEmpty,
} from "./richText";

vi.mock("../platform/assets", () => ({
  managedAssetUrl: vi.fn((assetId: string) => \`asset-url:\${assetId}\`),
}));

describe("rich document helpers", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("canonicalizes managed images without mutating editor state", () => {
    const source = {
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: "a".repeat(32),
            src: "asset-url:temporary",
            width: "640",
            height: 20_000,
          },
        },
      ],
    };

    const canonical = canonicalizeDocument(source);

    expect(canonical).not.toBe(source);
    expect(canonical).toEqual({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: "a".repeat(32),
            src: \`study-asset://\${"a".repeat(32)}\`,
            width: 640,
            height: null,
          },
        },
      ],
    });
    expect(source.content[0]?.attrs.src).toBe("asset-url:temporary");
  });

  it("hydrates only known managed assets and supplies missing alt text", () => {
    const asset = makeAsset({
      id: "b".repeat(32),
      originalFilename: "heart.png",
    });
    const document = {
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: asset.id,
            src: \`study-asset://\${asset.id}\`,
            alt: "",
          },
        },
        {
          type: "image",
          attrs: {
            assetId: "c".repeat(32),
            src: \`study-asset://\${"c".repeat(32)}\`,
            alt: "Unresolved",
          },
        },
      ],
    };

    const hydrated = hydrateDocument(document, [asset]);

    expect(managedAssetUrl).toHaveBeenCalledWith(asset.id);
    expect(hydrated).toEqual({
      type: "doc",
      content: [
        {
          type: "image",
          attrs: {
            assetId: asset.id,
            src: \`asset-url:\${asset.id}\`,
            alt: asset.originalFilename,
          },
        },
        {
          type: "image",
          attrs: {
            assetId: "c".repeat(32),
            src: \`study-asset://\${"c".repeat(32)}\`,
            alt: "Unresolved",
          },
        },
      ],
    });
  });

  it("collects unique image identities from nested documents", () => {
    const first = "d".repeat(32);
    const second = "e".repeat(32);
    expect(
      collectAssetIds({
        type: "doc",
        content: [
          { type: "image", attrs: { assetId: first } },
          {
            type: "blockquote",
            content: [
              { type: "image", attrs: { assetId: second } },
              { type: "image", attrs: { assetId: first } },
            ],
          },
        ],
      }),
    ).toEqual([first, second]);
  });

  it("treats image-only documents as non-empty", () => {
    expect(isDocumentEmpty("", paragraphDocument(""))).toBe(true);
    expect(
      isDocumentEmpty("", {
        type: "doc",
        content: [{ type: "image", attrs: { assetId: "f".repeat(32) } }],
      }),
    ).toBe(false);
    expect(isDocumentEmpty("Visible text", paragraphDocument("Visible text"))).toBe(false);
  });

  it("deep-clones a document before callers mutate it", () => {
    const original = paragraphDocument("Original");
    const clone = cloneDocument(original);
    const paragraph = (clone.content as Array<Record<string, unknown>>)[0];
    const text = (paragraph?.content as Array<Record<string, unknown>>)[0];
    if (text) text.text = "Changed";

    expect(original).toEqual(paragraphDocument("Original"));
    expect(clone).toEqual(paragraphDocument("Changed"));
  });
});
`,
);

function staged(kind, name = "selected") {
  const mediaType = kind === "pdf" ? "application/pdf" : "application/octet-stream";
  return `{ id: "1".repeat(32), purpose: ${
    kind === "pdf"
      ? '"pdf_document"'
      : kind === "ostudybackup"
        ? '"library_backup"'
        : '"study_content"'
  }, detectedKind: "${kind}", displayName: "${name}", mediaType: "${mediaType}", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 }`;
}

for (const path of [
  "src/screens/DecksScreen.test.tsx",
  "src/screens/DecksScreen.extended.test.tsx",
]) {
  let source = await read(path);
  source = source
    .replace(
      'vi.mock("../platform/files", () => ({\n  openFile: vi.fn(),\n  saveFile: vi.fn(),\n}));',
      'vi.mock("../platform/files", () => ({\n  selectExternalImport: vi.fn(),\n  publishExport: vi.fn(),\n}));',
    )
    .replaceAll("platformFiles.openFile", "platformFiles.selectExternalImport")
    .replaceAll("platformFiles.saveFile", "platformFiles.publishExport")
    .replace("  exportDeck: vi.fn(),", "  prepareDeckExport: vi.fn(),")
    .replaceAll("decksIpc.exportDeck", "decksIpc.prepareDeckExport");
  source = source
    .replaceAll('mockResolvedValue("/tmp/nclex.ostudy.json")', `mockResolvedValue(${staged("generated_json", "nclex.ostudy.json")})`)
    .replaceAll('mockResolvedValue("/tmp/dog-facts.studydeck")', `mockResolvedValue(${staged("studydeck", "dog-facts.studydeck")})`)
    .replaceAll('mockResolvedValue("/tmp/cards.ostudy.json")', `mockResolvedValue(${staged("generated_json", "cards.ostudy.json")})`)
    .replaceAll('mockResolvedValue("/tmp/imported.studydeck")', `mockResolvedValue(${staged("studydeck", "imported.studydeck")})`)
    .replaceAll('mockResolvedValue("/tmp/broken.studydeck")', `mockResolvedValue(${staged("studydeck", "broken.studydeck")})`)
    .replaceAll('mockResolvedValue("/tmp/retry.studydeck")', `mockResolvedValue(${staged("studydeck", "retry.studydeck")})`)
    .replaceAll('mockResolvedValue("/tmp/cards.txt")', 'mockRejectedValue(new Error("Unsupported external file."))')
    .replaceAll('"/tmp/nclex.ostudy.json"', '"1".repeat(32)')
    .replaceAll('"/tmp/dog-facts.studydeck"', '"1".repeat(32)')
    .replaceAll('"/tmp/imported.studydeck"', '"1".repeat(32)')
    .replaceAll('"/tmp/broken.studydeck"', '"1".repeat(32)')
    .replaceAll('"/tmp/retry.studydeck"', '"1".repeat(32)')
    .replaceAll('"/tmp/cards.ostudy.json"', '"1".repeat(32)')
    .replaceAll("expect(platformFiles.openFile)", "expect(platformFiles.selectExternalImport)")
    .replaceAll("expect(platformFiles.saveFile)", "expect(platformFiles.publishExport)");
  source = source
    .replace(/expect\(platformFiles\.selectExternalImport\)\.toHaveBeenCalledWith\([\s\S]*?\n\s*\);/gu, 'expect(platformFiles.selectExternalImport).toHaveBeenCalledWith("study_content");')
    .replace('expect(platformFiles.publishExport).toHaveBeenCalledOnce();\n    expect(decksIpc.prepareDeckExport).not.toHaveBeenCalled();', 'expect(decksIpc.prepareDeckExport).toHaveBeenCalledOnce();\n    expect(platformFiles.publishExport).not.toHaveBeenCalled();')
    .replace(/vi\.mocked\(platformFiles\.publishExport\)\.mockResolvedValue\([^;]+;/gu, 'vi.mocked(platformFiles.publishExport).mockResolvedValue({ preparedExportId: "2".repeat(32), displayName: "cardiology.studydeck", destinationKind: "desktop_file", completedAt: Date.now() });')
    .replace(/vi\.mocked\(decksIpc\.prepareDeckExport\)\.mockResolvedValue\(\{[\s\S]*?\n\s*\}\);/gu, 'vi.mocked(decksIpc.prepareDeckExport).mockResolvedValue({ preparedExport: { id: "2".repeat(32), purpose: "studydeck", suggestedFilename: "cardiology.studydeck", mediaType: "application/vnd.ostudy.studydeck", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 }, cardCount: 3, assetCount: 1 });')
    .replace(/expect\(decksIpc\.prepareDeckExport\)\.toHaveBeenCalledWith\([^;]+;/gu, 'expect(decksIpc.prepareDeckExport).toHaveBeenCalledWith(deck.id, importOperationId);')
    .replaceAll('expect(commonIpc.createOperationId).toHaveBeenCalledOnce();', 'expect(commonIpc.createOperationId).toHaveBeenCalled();');
  await write(path, source);
}

for (const path of [
  "src/screens/DocumentsScreen.test.tsx",
  "src/screens/DocumentsScreen.ocrFailure.test.tsx",
]) {
  let source = await read(path);
  source = source
    .replace('vi.mock("../platform/files", () => ({ openFile: vi.fn() }));', 'vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn() }));')
    .replaceAll("platformFiles.openFile", "platformFiles.selectExternalImport")
    .replaceAll('mockResolvedValue("/tmp/cardiac-physiology.pdf")', `mockResolvedValue(${staged("pdf", "cardiac-physiology.pdf")})`)
    .replaceAll('mockResolvedValue("/tmp/', 'mockResolvedValue({ id: "1".repeat(32), purpose: "pdf_document", detectedKind: "pdf", displayName: "')
    .replaceAll('importPdfAndGenerateFlashcards).toHaveBeenCalledWith(\n        "/tmp/cardiac-physiology.pdf",\n      )', 'importPdfAndGenerateFlashcards).toHaveBeenCalledWith("1".repeat(32))');
  await write(path, source);
}

for (const path of [
  "src/screens/SettingsScreen.test.tsx",
  "src/screens/SettingsScreen.extended.test.tsx",
]) {
  let source = await read(path);
  source = source
    .replace(
      'vi.mock("../platform/files", () => ({\n  openFile: vi.fn(),\n  saveFile: vi.fn(),\n}));',
      'vi.mock("../platform/files", () => ({\n  selectExternalImport: vi.fn(),\n  publishExport: vi.fn(),\n}));',
    )
    .replace('vi.mock("../platform/files", () => ({ openFile: vi.fn(), saveFile: vi.fn() }));', 'vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn(), publishExport: vi.fn() }));')
    .replace('exportLibraryBackup: vi.fn(),', 'prepareLibraryBackup: vi.fn(),')
    .replace('vi.mock("../ipc/backup", () => ({ exportLibraryBackup: vi.fn(), prepareLibraryRestore: vi.fn() }));', 'vi.mock("../ipc/backup", () => ({ prepareLibraryBackup: vi.fn(), prepareLibraryRestore: vi.fn() }));')
    .replaceAll("platformFiles.openFile", "platformFiles.selectExternalImport")
    .replaceAll("platformFiles.saveFile", "platformFiles.publishExport")
    .replaceAll("backupIpc.exportLibraryBackup", "backupIpc.prepareLibraryBackup")
    .replaceAll('mockResolvedValue("/tmp/library.ostudybackup")', `mockResolvedValue(${staged("ostudybackup", "library.ostudybackup")})`)
    .replace(/vi\.mocked\(backupIpc\.prepareLibraryBackup\)\.mockResolvedValue\(\{[\s\S]*?\n\s*\}\);/u, 'vi.mocked(backupIpc.prepareLibraryBackup).mockResolvedValue({ preparedExport: { id: "2".repeat(32), purpose: "library_backup", suggestedFilename: "library.ostudybackup", mediaType: "application/vnd.ostudy.backup", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 }, deckCount: 3, templateCount: 2, reviewCount: 41, assetCount: 4 });')
    .replaceAll('expect(backupIpc.prepareLibraryBackup).toHaveBeenCalledWith("/tmp/library.ostudybackup")', 'expect(backupIpc.prepareLibraryBackup).toHaveBeenCalled()')
    .replaceAll('expect(backupIpc.prepareLibraryRestore).toHaveBeenCalledWith("/tmp/library.ostudybackup")', 'expect(backupIpc.prepareLibraryRestore).toHaveBeenCalledWith("1".repeat(32))');
  await write(path, source);
}

await rm(join(root, "scripts", "run-platform-p2-pathfree-frontend.mjs"), { force: true });
console.log("Applied path-free frontend migration and updated affected tests.");
