import { beforeEach, describe, expect, it, vi } from "vitest";

import { applyMaterialInkChanges, readMaterialInk, type InkMark } from "../../ipc/materialInk";
import { inkAnnotations, materialInkSource } from "./materialInkSource";

vi.mock("../../ipc/materialInk", () => ({ readMaterialInk: vi.fn(), applyMaterialInkChanges: vi.fn() }));

const pages = [
  { key: "title", width: 720, height: 405, image: "blob:1" },
  { key: "step:loop", width: 720, height: 405, image: "blob:2" },
];
const shape = { kind: "line" as const, x1: 1, y1: 1, x2: 9, y2: 9, color: "#111111", strokeWidth: 2 };

function mark(id: string, pageKey: string): InkMark {
  return { id, pageKey, shape, revision: 1, createdAt: 1, updatedAt: 1 };
}

describe("materialInkSource", () => {
  beforeEach(() => {
    vi.mocked(readMaterialInk).mockReset();
    vi.mocked(applyMaterialInkChanges).mockReset();
  });

  it("numbers the pages in order and leaves out marks on pages not shown", () => {
    const loaded = inkAnnotations(pages, [mark("a", "step:loop"), mark("b", "step:gone")]);
    expect(loaded.pages.map((page) => [page.pageNumber, page.width, page.height])).toEqual([[1, 720, 405], [2, 720, 405]]);
    expect(loaded.annotations).toEqual([expect.objectContaining({ id: "a", pageNumber: 2 })]);
  });

  it("saves a mark under its page's key", async () => {
    const material = { kind: "lesson" as const, lessonId: "l".repeat(32) };
    const source = materialInkSource(material, pages);
    vi.mocked(readMaterialInk).mockResolvedValue({ marks: [mark("a", "title")], unavailableLinkIds: [] });
    expect((await source.load("lesson")).annotations[0]?.pageNumber).toBe(1);

    await source.send("lesson", [{ op: "put", id: "b", pageNumber: 2, shape }, { op: "delete", id: "a" }]);
    expect(applyMaterialInkChanges).toHaveBeenCalledWith(material, [
      { op: "put", id: "b", pageKey: "step:loop", shape },
      { op: "delete", id: "a" },
    ]);
  });

  it("tells the guide which of its links lead nowhere", async () => {
    const heard = vi.fn();
    const source = materialInkSource({ kind: "studyGuide", documentId: "d".repeat(32) }, pages, heard);
    vi.mocked(readMaterialInk).mockResolvedValue({ marks: [mark("a", "title")], unavailableLinkIds: ["a"] });
    await source.load("guide");
    expect(heard).toHaveBeenCalledWith(new Set(["a"]));
  });
});
