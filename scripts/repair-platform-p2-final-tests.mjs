#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");

let domain = await read("src/ipc/domain-contracts.test.ts");
domain = domain.replace(
  /await expectInvocation\(\(\) => importImage\([^)]*\), "import_image", \{\s*input: \{\s*draftId\s*\},\s*\}\);/u,
  'await expectInvocation(() => importImage("1".repeat(32), draftId), "import_image", { input: { stagedFileId: "1".repeat(32), draftId } });',
);
await write("src/ipc/domain-contracts.test.ts", domain);

let cardEditor = await read("src/screens/CardEditorScreen.test.tsx");
cardEditor = cardEditor
  .replace(
    '      expect(assetsIpc.beginAssetDraft).toHaveBeenCalledWith("card", card.noteId);\n',
    "",
  )
  .replace("          assetDraftId: draftId,", '          assetDraftId: "7".repeat(32),');
await write("src/screens/CardEditorScreen.test.tsx", cardEditor);

let templates = await read("src/screens/TemplatesScreen.test.tsx");
templates = templates
  .replace('const ASSET_DRAFT_ID = "e".repeat(32);', 'const ASSET_DRAFT_ID = "7".repeat(32);')
  .replace(
    '    expect(assetsIpc.beginAssetDraft).toHaveBeenCalledWith("template", template.id);\n',
    "",
  );
await write("src/screens/TemplatesScreen.test.tsx", templates);

let decks = await read("src/screens/DecksScreen.extended.test.tsx");
decks = decks
  .replace(
    '        "Select an ostudy .studydeck or generated .ostudy.json file.",',
    '        "Unsupported external file.",',
  )
  .replace(
    '    vi.mocked(decksIpc.listDeckCardSummaries).mockResolvedValue({',
    `    vi.mocked(decksIpc.prepareDeckExport).mockResolvedValue({
      preparedExport: {
        id: "2".repeat(32),
        purpose: "studydeck",
        suggestedFilename: "cardiology.studydeck",
        mediaType: "application/vnd.ostudy.studydeck",
        byteSize: 128,
        sha256: "0".repeat(64),
        expiresAt: Date.now() + 60_000,
      },
      cardCount: 1,
      assetCount: 0,
    });
    vi.mocked(decksIpc.listDeckCardSummaries).mockResolvedValue({`,
  )
  .replace(
    `    await waitFor(() => expect(platformFiles.publishExport).toHaveBeenCalledOnce());
    expect(decksIpc.prepareDeckExport).not.toHaveBeenCalled();`,
    `    await waitFor(() => expect(decksIpc.prepareDeckExport).toHaveBeenCalledOnce());
    expect(platformFiles.publishExport).toHaveBeenCalledOnce();`,
  );
await write("src/screens/DecksScreen.extended.test.tsx", decks);

const templateAssets = `import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as templatesIpc from "../ipc/templates";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { TemplatesScreen } from "./TemplatesScreen";

const durableDraftMock = vi.hoisted(() => ({
  draftId: "d".repeat(32),
  assetDraftId: "7".repeat(32) as string | null,
  revision: 1,
  status: "saved" as "opening" | "saved",
  errorMessage: null,
  recovered: false,
  hasUnsavedLocalChanges: false,
  flush: vi.fn().mockResolvedValue(undefined),
  discard: vi.fn().mockResolvedValue(undefined),
  commit: vi.fn().mockResolvedValue(undefined),
  restart: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => durableDraftMock,
}));
vi.mock("../ipc/common", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../ipc/templates", () => ({
  createCardTemplate: vi.fn(),
  deleteCardTemplate: vi.fn(),
  getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn(),
  updateCardTemplate: vi.fn(),
}));
vi.mock("./templates/TemplateLayoutEditor", () => ({
  TemplateLayoutEditor: ({ label }: { label: string }) => <div>{label}</div>,
}));

const assetDraftId = "7".repeat(32);

function savedTemplate(): templatesIpc.CardTemplate {
  return {
    id: "b".repeat(32),
    name: "Medication",
    description: "",
    fields: [
      {
        id: "c".repeat(32),
        name: "Field",
        description: "",
        position: 0,
      },
    ],
    rules: [
      {
        id: "d".repeat(32),
        name: "Card",
        frontDoc: { type: "doc", content: [] },
        backDoc: { type: "doc", content: [] },
        position: 0,
      },
    ],
    assets: [],
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}

function renderTemplates() {
  return render(
    <OverlayProvider>
      <TemplatesScreen onDirtyChange={vi.fn()} showToast={vi.fn()} />
    </OverlayProvider>,
  );
}

describe("TemplatesScreen durable managed assets", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    durableDraftMock.status = "saved";
    durableDraftMock.assetDraftId = assetDraftId;
    durableDraftMock.flush.mockResolvedValue(undefined);
    durableDraftMock.discard.mockResolvedValue(undefined);
    durableDraftMock.commit.mockResolvedValue(undefined);
    vi.mocked(templatesIpc.listCardTemplateSummaries).mockResolvedValue([]);
    vi.mocked(templatesIpc.createCardTemplate).mockResolvedValue(savedTemplate());
  });

  it("uses the persistent draft asset identity when saving a template", async () => {
    renderTemplates();

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Medication" },
    });
    fireEvent.click(await screen.findByRole("button", { name: "Save template" }));

    await waitFor(() => {
      expect(templatesIpc.createCardTemplate).toHaveBeenCalledWith(
        expect.objectContaining({ name: "Medication", assetDraftId }),
      );
      expect(durableDraftMock.flush).toHaveBeenCalled();
      expect(durableDraftMock.commit).toHaveBeenCalledWith(savedTemplate().id);
    });
  });

  it("discards the persistent draft when edited template work is canceled", async () => {
    renderTemplates();

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Medication" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Discard changes" }),
    );

    await waitFor(() => expect(durableDraftMock.discard).toHaveBeenCalledOnce());
    expect(
      await screen.findByRole("heading", { name: "Create a reusable card template" }),
    ).toBeTruthy();
  });

  it("blocks save while the durable draft is opening and still permits discard", async () => {
    durableDraftMock.status = "opening";
    durableDraftMock.assetDraftId = null;
    renderTemplates();

    await screen.findByRole("heading", { name: "Create a reusable card template" });
    fireEvent.click(screen.getByRole("button", { name: "New template" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Name" }), {
      target: { value: "Medication" },
    });

    const preparing = await screen.findByRole("button", { name: "Preparing…" });
    expect((preparing as HTMLButtonElement).disabled).toBe(true);
    expect(templatesIpc.createCardTemplate).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Discard changes" }),
    );

    await waitFor(() => expect(durableDraftMock.discard).toHaveBeenCalledOnce());
    expect(templatesIpc.createCardTemplate).not.toHaveBeenCalled();
  });
});
`;
await write("src/screens/TemplatesScreen.assets.test.tsx", templateAssets);

console.log("Aligned remaining frontend tests with final P0-P2 contracts.");
