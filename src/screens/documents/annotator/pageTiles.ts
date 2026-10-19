import { useEffect, useRef, useState } from "react";

import {
  MAX_TILED_PAGE_SIDE,
  renderDocumentPageTile,
  type AnnotatedPage,
  type DocumentPageTileRequest,
} from "../../../ipc/documentAnnotations";
import { PageImageCache } from "./pageImages";

/** Pixels per point of the page images every PDF import renders (120 dpi). */
export const PAGE_IMAGE_DENSITY = 120 / 72;
/** The sharpest tiles drawn, 576 dpi: the largest zoom on a 2× screen. */
export const MAX_TILE_DENSITY = 8;
const TILE_SIDE = 512;
/** Tiles kept underneath newer ones while those are drawn. */
const MAX_UNDERNEATH = 48;
/** Below this many times the page image's density the page image is sharp enough. */
const SHARP_ENOUGH = 1.1;

/** A rectangle of a page in points. */
export interface PageRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PageTile extends DocumentPageTileRequest {
  key: string;
}

/**
 * The pixels per point tiles need for a page shown at `scale` CSS pixels per
 * point on a screen with `pixelRatio` device pixels per CSS pixel, or null
 * when the page image is sharp enough. Densities come in steps of √2 above
 * the page image's, so small zoom changes reuse the tiles already drawn, and
 * a page is never drawn larger than the renderer allows.
 */
export function tileDensity(page: Pick<AnnotatedPage, "width" | "height">, scale: number, pixelRatio: number): number | null {
  const wanted = scale * Math.max(1, pixelRatio);
  if (!(wanted > PAGE_IMAGE_DENSITY * SHARP_ENOUGH)) return null;
  const step = Math.ceil(2 * Math.log2(wanted / PAGE_IMAGE_DENSITY));
  const largest = MAX_TILED_PAGE_SIDE / Math.max(page.width, page.height, 1);
  const density = Math.min(PAGE_IMAGE_DENSITY * 2 ** (step / 2), MAX_TILE_DENSITY, largest);
  return density > PAGE_IMAGE_DENSITY * SHARP_ENOUGH ? density : null;
}

/** The tiles that cover `region` of a page drawn at `density` pixels per point. */
export function tilesFor(page: AnnotatedPage, density: number, region: PageRegion): PageTile[] {
  const pageWidth = Math.min(MAX_TILED_PAGE_SIDE, Math.max(1, Math.round(page.width * density)));
  const pageHeight = Math.min(MAX_TILED_PAGE_SIDE, Math.max(1, Math.round(page.height * density)));
  const xScale = pageWidth / page.width;
  const yScale = pageHeight / page.height;
  const columns = Math.ceil(pageWidth / TILE_SIDE);
  const rows = Math.ceil(pageHeight / TILE_SIDE);
  const first = (value: number, scale: number, count: number) => Math.min(count, Math.max(0, Math.floor((value * scale) / TILE_SIDE)));
  const last = (value: number, scale: number, count: number) => Math.min(count, Math.max(0, Math.ceil((value * scale) / TILE_SIDE)));
  const tiles: PageTile[] = [];
  for (let row = first(region.y, yScale, rows); row < last(region.y + region.height, yScale, rows); row += 1) {
    for (let column = first(region.x, xScale, columns); column < last(region.x + region.width, xScale, columns); column += 1) {
      const left = column * TILE_SIDE;
      const top = row * TILE_SIDE;
      const width = Math.min(TILE_SIDE, pageWidth - left);
      const height = Math.min(TILE_SIDE, pageHeight - top);
      tiles.push({
        key: `${page.pageNumber}:${pageWidth}x${pageHeight}:${left},${top}:${width}x${height}`,
        pageNumber: page.pageNumber, pageWidth, pageHeight, left, top, width, height,
      });
    }
  }
  return tiles;
}

/** The request a tile key stands for. */
export function parseTileKey(key: string): DocumentPageTileRequest {
  const match = /^(\d+):(\d+)x(\d+):(\d+),(\d+):(\d+)x(\d+)$/.exec(key);
  if (!match) throw new Error("Not a page tile key.");
  const [pageNumber, pageWidth, pageHeight, left, top, width, height] = match.slice(1).map(Number) as [
    number, number, number, number, number, number, number,
  ];
  return { pageNumber, pageWidth, pageHeight, left, top, width, height };
}

/** Recently drawn tiles of one document, two drawn at a time. */
export function createTileCache(
  documentId: string,
  render: (documentId: string, tile: DocumentPageTileRequest) => Promise<ArrayBuffer> = renderDocumentPageTile,
): PageImageCache {
  return new PageImageCache(documentId, (id, key) => render(id, parseTileKey(key)), 64, 2);
}

export interface ShownTile extends PageTile {
  url: string;
}

const NONE: readonly ShownTile[] = [];

/**
 * The drawn tiles to lay over a page. When the tiles change (a new zoom, or
 * scrolling to another part of the page), the ones already showing stay
 * underneath until every new tile has been drawn or has failed, so the page
 * never flashes back to its soft image.
 */
export function usePageTiles(cache: PageImageCache | null, tiles: readonly PageTile[]): readonly ShownTile[] {
  const [shown, setShown] = useState<readonly ShownTile[]>(NONE);
  const underneath = useRef<{ tiles: readonly ShownTile[]; release: () => void } | null>(null);
  const key = tiles.map((tile) => tile.key).join("|");
  const latestTiles = useRef(tiles);
  latestTiles.current = tiles;

  useEffect(() => {
    const wanted = latestTiles.current;
    if (!cache || wanted.length === 0) {
      underneath.current?.release();
      underneath.current = null;
      setShown(NONE);
      return undefined;
    }
    const done = new Map<string, string | null>();
    let active = true;
    const current = () => wanted.flatMap((tile) => {
      const url = done.get(tile.key);
      return url ? [{ ...tile, url }] : [];
    });
    const publish = () => {
      if (!active) return;
      if (done.size === wanted.length && underneath.current) {
        underneath.current.release();
        underneath.current = null;
      }
      setShown([...(underneath.current?.tiles ?? []), ...current()]);
    };
    const releases = wanted.map((tile) => cache.acquire(tile.key, (image) => {
      done.set(tile.key, image.url);
      publish();
    }));
    publish();
    return () => {
      active = false;
      const drawn = current();
      const releaseThese = () => releases.forEach((release) => release());
      if (drawn.length === 0) {
        releaseThese();
        return;
      }
      // Whatever shows now stays under the next tiles until they are drawn.
      const older = underneath.current;
      if (older && older.tiles.length + drawn.length <= MAX_UNDERNEATH) {
        underneath.current = {
          tiles: [...older.tiles, ...drawn],
          release: () => { older.release(); releaseThese(); },
        };
      } else {
        older?.release();
        underneath.current = { tiles: drawn, release: releaseThese };
      }
    };
  }, [cache, key]);

  useEffect(() => () => {
    underneath.current?.release();
    underneath.current = null;
  }, []);

  return shown;
}
