import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AnnotatedPage, DocumentPageTileRequest } from "../../../ipc/documentAnnotations";
import {
  createTileCache,
  MAX_TILE_DENSITY,
  PAGE_IMAGE_DENSITY,
  parseTileKey,
  tileDensity,
  tilesFor,
  usePageTiles,
  type PageTile,
} from "./pageTiles";

vi.mock("../../../ipc/documentAnnotations", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../../ipc/documentAnnotations")>()),
  renderDocumentPageTile: vi.fn(),
}));

const documentId = "d".repeat(32);
const letter: AnnotatedPage = { pageNumber: 2, assetId: "a", width: 612, height: 792, imageAvailable: true };

beforeEach(() => {
  let count = 0;
  Object.defineProperty(URL, "createObjectURL", { configurable: true, writable: true, value: vi.fn(() => `blob:tile-${(count += 1)}`) });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, writable: true, value: vi.fn() });
});

describe("tile density", () => {
  it("leaves pages alone where the page image is already sharp", () => {
    expect(tileDensity(letter, 120 / 72, 1)).toBeNull();
    expect(tileDensity(letter, 1, 1.5)).toBeNull();
    expect(tileDensity(letter, 0.5, 2)).toBeNull();
  });

  it("draws at least as sharp as the screen shows, in steps of √2", () => {
    const density = tileDensity(letter, 120 / 72, 2);
    expect(density).toBeCloseTo(PAGE_IMAGE_DENSITY * 2);
    // A slightly different zoom reuses the same tiles.
    expect(tileDensity(letter, 1.6, 2)).toBe(density);
    const between = tileDensity(letter, 2.1, 1)!;
    expect(between).toBeGreaterThanOrEqual(2.1);
    expect(between).toBeCloseTo(PAGE_IMAGE_DENSITY * Math.SQRT2);
  });

  it("stops at the sharpest density and at the largest page the renderer draws", () => {
    expect(tileDensity(letter, 4, 3)).toBe(MAX_TILE_DENSITY);
    const poster = { width: 14_400, height: 14_400 };
    expect(tileDensity(poster, 4, 2)).toBeNull();
    expect(tileDensity({ width: 4_000, height: 4_000 }, 4, 2)).toBeCloseTo(16_384 / 4_000);
  });
});

describe("tiles", () => {
  it("covers just the region in view, with the last row and column cut to the page", () => {
    const density = PAGE_IMAGE_DENSITY * 2; // 2040 by 2640 pixels
    const tiles = tilesFor(letter, density, { x: 0, y: 0, width: 612, height: 792 });
    expect(tiles).toHaveLength(4 * 6);
    const corner = tiles[tiles.length - 1]!;
    expect(corner).toMatchObject({ pageNumber: 2, pageWidth: 2040, pageHeight: 2640, left: 1536, top: 2560, width: 504, height: 80 });

    const middle = tilesFor(letter, density, { x: 200, y: 320, width: 50, height: 10 });
    expect(middle.map(({ left, top }) => [left, top])).toEqual([[512, 1024]]);
  });

  it("names each tile with the request it stands for", () => {
    const tile = tilesFor(letter, PAGE_IMAGE_DENSITY * 2, { x: 0, y: 0, width: 1, height: 1 })[0]!;
    const { key, ...request } = tile;
    expect(parseTileKey(key)).toEqual(request);
    expect(() => parseTileKey("2:x")).toThrow();
  });

  it("asks the native renderer for a tile by its key", async () => {
    const render = vi.fn((_documentId: string, _tile: DocumentPageTileRequest) => Promise.resolve(new ArrayBuffer(4)));
    const cache = createTileCache(documentId, render);
    const tile = tilesFor(letter, PAGE_IMAGE_DENSITY * 2, { x: 0, y: 0, width: 1, height: 1 })[0]!;
    cache.acquire(tile.key, () => {});
    await act(async () => {});
    const { key: _key, ...request } = tile;
    expect(render).toHaveBeenCalledWith(documentId, request);
    cache.dispose();
  });
});

describe("shown tiles", () => {
  function controlledCache() {
    const pending = new Map<string, () => void>();
    const failing = new Map<string, () => void>();
    const render = vi.fn((_documentId: string, tile: DocumentPageTileRequest) => new Promise<ArrayBuffer>((resolve, reject) => {
      const key = `${tile.left},${tile.top}`;
      pending.set(key, () => resolve(new ArrayBuffer(4)));
      failing.set(key, () => reject(new Error("unreadable")));
    }));
    return { cache: createTileCache(documentId, render), pending, failing };
  }

  const at = (density: number, x: number) => tilesFor(letter, density, { x, y: 0, width: 1, height: 1 });

  it("keeps the tiles already shown underneath until every new tile is drawn", async () => {
    const { cache, pending, failing } = controlledCache();
    const first = at(PAGE_IMAGE_DENSITY * 2, 0);
    const { result, rerender } = renderHook(({ tiles }: { tiles: readonly PageTile[] }) => usePageTiles(cache, tiles), {
      initialProps: { tiles: first },
    });
    expect(result.current).toEqual([]);
    await act(async () => pending.get("0,0")!());
    expect(result.current.map((tile) => tile.key)).toEqual([first[0]!.key]);

    // Two tiles at a sharper density: the old one stays until both have finished.
    const sharper = [...at(PAGE_IMAGE_DENSITY * 4, 0), ...at(PAGE_IMAGE_DENSITY * 4, 120)];
    rerender({ tiles: sharper });
    expect(result.current.map((tile) => tile.key)).toEqual([first[0]!.key]);
    await act(async () => pending.get("0,0")!());
    expect(result.current.map((tile) => tile.key)).toEqual([first[0]!.key, sharper[0]!.key]);
    await act(async () => failing.get("512,0")!());
    expect(result.current.map((tile) => tile.key)).toEqual([sharper[0]!.key]);

    // Zoomed back out: no tiles, the page image shows alone.
    rerender({ tiles: [] });
    expect(result.current).toEqual([]);
    cache.dispose();
  });
});
