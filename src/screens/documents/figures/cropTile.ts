import type { DocumentPageTileRequest } from "../../../ipc/documentAnnotations";
import type { FigureBox } from "../../../ipc/documentFigures";

/** About 200 dpi: sharp enough for a figure's small labels. */
const PIXELS_PER_POINT = 200 / 72;
/** One tile is at most this many pixels a side. */
const MAX_TILE_SIDE = 1_024;
/** A little page around the figure, in points. */
const MARGIN = 4;

/**
 * The one page tile that shows `box` on a page `pageWidth` by `pageHeight`
 * points, drawn at about 200 dpi or smaller so it fits a single tile.
 */
export function cropTile(
  pageNumber: number,
  pageWidth: number,
  pageHeight: number,
  box: FigureBox,
): DocumentPageTileRequest {
  const left = Math.max(0, box.left - MARGIN);
  const top = Math.max(0, box.top - MARGIN);
  const right = Math.min(pageWidth, box.left + box.width + MARGIN);
  const bottom = Math.min(pageHeight, box.top + box.height + MARGIN);
  const scale = Math.min(PIXELS_PER_POINT, MAX_TILE_SIDE / Math.max(right - left, bottom - top, 1));
  const page = {
    width: Math.max(1, Math.round(pageWidth * scale)),
    height: Math.max(1, Math.round(pageHeight * scale)),
  };
  const tileLeft = Math.min(page.width - 1, Math.floor(left * scale));
  const tileTop = Math.min(page.height - 1, Math.floor(top * scale));
  return {
    pageNumber,
    pageWidth: page.width,
    pageHeight: page.height,
    left: tileLeft,
    top: tileTop,
    width: Math.max(1, Math.min(MAX_TILE_SIDE, page.width - tileLeft, Math.ceil(right * scale) - tileLeft)),
    height: Math.max(1, Math.min(MAX_TILE_SIDE, page.height - tileTop, Math.ceil(bottom * scale) - tileTop)),
  };
}
