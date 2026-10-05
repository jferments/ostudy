import { beforeEach, describe, expect, it, vi } from "vitest";

import { PageImageCache, type PageImage } from "./pageImages";

vi.mock("../../../ipc/studyGuides", () => ({ readDocumentSourceAsset: vi.fn() }));

const documentId = "d".repeat(32);

function settle(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

/** Reads that finish when the test says so. */
function reads() {
  const pending = new Map<string, { resolve: () => void; reject: () => void }>();
  const read = vi.fn((_documentId: string, assetId: string) => new Promise<ArrayBuffer>((resolve, reject) => {
    pending.set(assetId, { resolve: () => resolve(new ArrayBuffer(8)), reject: () => reject(new Error("unreadable")) });
  }));
  return { read, pending, requested: () => read.mock.calls.map(([, assetId]) => assetId) };
}

let createUrl: ReturnType<typeof vi.fn<(blob: Blob) => string>>;
let revokeUrl: ReturnType<typeof vi.fn<(url: string) => void>>;

beforeEach(() => {
  let count = 0;
  createUrl = vi.fn(() => `blob:page-${(count += 1)}`);
  revokeUrl = vi.fn();
  Object.defineProperty(URL, "createObjectURL", { configurable: true, writable: true, value: createUrl });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, writable: true, value: revokeUrl });
});

describe("page images", () => {
  it("reads three pages at a time and shares a page's image between its users", async () => {
    const { read, pending, requested } = reads();
    const cache = new PageImageCache(documentId, read, 24, 3);
    const seen: PageImage[] = [];
    const late: PageImage[] = [];
    for (const assetId of ["a", "b", "c", "d"]) cache.acquire(assetId, (image) => seen.push(image));
    cache.acquire("a", (image) => seen.push(image));
    expect(requested()).toEqual(["a", "b", "c"]);

    pending.get("a")?.resolve();
    await settle();
    expect(requested()).toEqual(["a", "b", "c", "d"]);
    expect(seen).toEqual([{ url: "blob:page-1", failed: false }, { url: "blob:page-1", failed: false }]);
    expect(createUrl.mock.calls[0]?.[0].type).toBe("image/png");

    // A page that asks after the read has finished hears at once.
    cache.acquire("a", (image) => late.push(image));
    expect(late).toEqual([{ url: "blob:page-1", failed: false }]);
  });

  it("never reads a page scrolled past before its turn", async () => {
    const { read, pending, requested } = reads();
    const cache = new PageImageCache(documentId, read, 24, 1);
    cache.acquire("a", () => undefined);
    const release = cache.acquire("b", () => undefined);
    cache.acquire("c", () => undefined);
    release();
    release();
    pending.get("a")?.resolve();
    await settle();
    expect(requested()).toEqual(["a", "c"]);
  });

  it("reports a page whose image could not be read", async () => {
    const { read, pending } = reads();
    const cache = new PageImageCache(documentId, read);
    const seen: PageImage[] = [];
    cache.acquire("a", (image) => seen.push(image));
    pending.get("a")?.reject();
    await settle();
    expect(seen).toEqual([{ url: null, failed: true }]);
    expect(createUrl).not.toHaveBeenCalled();
  });

  it("lets go of the oldest unused images beyond its limit", async () => {
    const { read, pending, requested } = reads();
    const cache = new PageImageCache(documentId, read, 2, 3);
    for (const assetId of ["a", "b", "c"]) {
      const release = cache.acquire(assetId, () => undefined);
      pending.get(assetId)?.resolve();
      await settle();
      release();
    }
    expect(revokeUrl).toHaveBeenCalledTimes(1);
    expect(revokeUrl).toHaveBeenCalledWith("blob:page-1");
    cache.acquire("a", () => undefined);
    expect(requested()).toEqual(["a", "b", "c", "a"]);
  });

  it("revokes every image when closed and ignores reads still out", async () => {
    const { read, pending } = reads();
    const cache = new PageImageCache(documentId, read);
    const seen: PageImage[] = [];
    cache.acquire("a", () => undefined);
    pending.get("a")?.resolve();
    await settle();
    cache.acquire("b", (image) => seen.push(image));
    cache.dispose();
    expect(revokeUrl).toHaveBeenCalledWith("blob:page-1");
    pending.get("b")?.resolve();
    await settle();
    expect(createUrl).toHaveBeenCalledTimes(1);
    expect(seen).toEqual([]);
  });
});
