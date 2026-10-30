import { beforeEach, describe, expect, it, vi } from "vitest";

import * as assetsIpc from "../../ipc/assets";
import type { Asset } from "../../ipc/assets";
import * as notebooksIpc from "../../ipc/notebooks";
import { clipToNotebook } from "./clipToNotebook";

vi.mock("../../ipc/assets", () => ({ beginAssetDraft: vi.fn(), cancelAssetDraft: vi.fn() }));
vi.mock("../../ipc/notebooks", () => ({
  addNotebookPage: vi.fn(),
  applyNotebookItemChanges: vi.fn(),
  getNotebook: vi.fn(),
  stageDocumentClip: vi.fn(),
}));

const notebookId = "b".repeat(32);
const documentId = "d".repeat(32);
const draftId = "f".repeat(32);
const asset = { id: "a".repeat(32) } as Asset;
const region: notebooksIpc.DocumentClipRegion = {
  documentId, pageNumber: 4, pageWidth: 612, pageHeight: 792, x: 100, y: 200, width: 300, height: 150,
};

function page(seed: string, position: number): notebooksIpc.NotebookPage {
  return { id: seed.repeat(32), position, paper: "blank", width: 612, height: 792, layout: "page" };
}

function contents(pages: notebooksIpc.NotebookPage[], items: notebooksIpc.NotebookItem[] = []): notebooksIpc.NotebookContents {
  return {
    notebook: {
      id: notebookId, subjectId: null, title: "Cardiac cycle map", paper: "blank", pageSize: "letter",
      revision: 1, createdAt: 1, updatedAt: 1,
    },
    pages,
    items,
    unavailableLinkIds: [],
    clipDocuments: {},
  };
}

function box(pageId: string, y: number, height: number): notebooksIpc.NotebookItem {
  return {
    id: `${y}`.padStart(32, "0"), pageId, revision: 1, createdAt: 1, updatedAt: 1,
    shape: { kind: "rectangle", x: 60, y, width: 200, height, color: "#dc2626", strokeWidth: 2 },
  };
}

beforeEach(() => {
  vi.mocked(assetsIpc.beginAssetDraft).mockResolvedValue({ id: draftId } as Awaited<ReturnType<typeof assetsIpc.beginAssetDraft>>);
  vi.mocked(assetsIpc.cancelAssetDraft).mockResolvedValue();
  vi.mocked(notebooksIpc.stageDocumentClip).mockResolvedValue({ asset, width: 300, height: 150 });
  vi.mocked(notebooksIpc.applyNotebookItemChanges).mockResolvedValue({ items: [], deletedIds: [] });
});

describe("clipToNotebook", () => {
  it("places the clip below the last page's marks, centred, naming the page it came from", async () => {
    const last = page("2", 2);
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents([page("1", 1), last], [box(last.id, 48, 100)]));

    const placed = await clipToNotebook(notebookId, region);

    expect(assetsIpc.beginAssetDraft).toHaveBeenCalledWith("import", notebookId);
    expect(notebooksIpc.stageDocumentClip).toHaveBeenCalledWith(draftId, region);
    const [, changes] = vi.mocked(notebooksIpc.applyNotebookItemChanges).mock.calls[0]!;
    expect(changes).toEqual([{
      op: "put", id: placed.itemId, pageId: last.id, assetDraftId: draftId,
      // The box ends at 149, half its stroke below its edge; the clip sits 24 points under it.
      shape: { kind: "image", x: 156, y: 173, width: 300, height: 150, assetId: asset.id, source: { documentId, pageNumber: 4 } },
    }]);
    expect(placed).toMatchObject({ notebookId, pageId: last.id });
    expect(notebooksIpc.addNotebookPage).not.toHaveBeenCalled();
  });

  it("starts a new page when the last one has no room left", async () => {
    const full = page("1", 1);
    const fresh = page("2", 2);
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents([full], [box(full.id, 48, 650)]));
    vi.mocked(notebooksIpc.addNotebookPage).mockResolvedValue(contents([full, fresh]));

    const placed = await clipToNotebook(notebookId, region);

    expect(notebooksIpc.addNotebookPage).toHaveBeenCalledWith(notebookId, 2);
    expect(placed.pageId).toBe(fresh.id);
    const [, changes] = vi.mocked(notebooksIpc.applyNotebookItemChanges).mock.calls[0]!;
    expect(changes[0]).toMatchObject({ pageId: fresh.id, shape: { y: 48 } });
  });

  it("gives back the image draft when the clip cannot be placed", async () => {
    vi.mocked(notebooksIpc.getNotebook).mockResolvedValue(contents([page("1", 1)]));
    vi.mocked(notebooksIpc.applyNotebookItemChanges).mockRejectedValue(new Error("Notes could not be saved."));

    await expect(clipToNotebook(notebookId, region)).rejects.toThrow("Notes could not be saved.");
    expect(assetsIpc.cancelAssetDraft).toHaveBeenCalledWith(draftId);
  });
});
