import { describe, expect, it } from "vitest";

import { cropTile } from "./cropTile";

describe("figure crop tiles", () => {
  it("draws a small figure at about 200 dpi with a little page around it", () => {
    const tile = cropTile(3, 612, 792, { left: 72, top: 100, width: 144, height: 72 });
    expect(tile.pageNumber).toBe(3);
    expect(tile.pageWidth).toBe(Math.round(612 * 200 / 72));
    expect(tile.left).toBe(Math.floor(68 * 200 / 72));
    expect(tile.top).toBe(Math.floor(96 * 200 / 72));
    expect(tile.width).toBe(Math.ceil(220 * 200 / 72) - tile.left);
  });

  it("shrinks a large figure to fit one tile and stays on the page", () => {
    const tile = cropTile(1, 612, 792, { left: 0, top: 0, width: 612, height: 600 });
    expect(Math.max(tile.width, tile.height)).toBeLessThanOrEqual(1024);
    expect(tile.left).toBe(0);
    expect(tile.top).toBe(0);
    expect(tile.left + tile.width).toBeLessThanOrEqual(tile.pageWidth);
    expect(tile.top + tile.height).toBeLessThanOrEqual(tile.pageHeight);
  });
});
