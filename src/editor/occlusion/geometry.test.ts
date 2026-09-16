import { describe, expect, it } from "vitest";

import {
  FIT_VIEWPORT, clampViewport, imageTransform, insideImage, moveRectangle, panViewport,
  rectangleFromCorners, rectanglesOverlap, resizeRectangle, toImagePoint, toScreenPoint,
  validateRectangle, zoomToRegion, zoomViewport, type ImageViewport, type ResizeHandle,
} from "./geometry";

const image = { width: 1600, height: 900 };
const frame = { width: 800, height: 600 };
const region = { x: 0.2, y: 0.3, width: 0.1, height: 0.2 };

describe("occlusion image geometry", () => {
  it("fits portrait and landscape images with letterboxing and invertible CSS coordinates", () => {
    for (const size of [image, { width: 900, height: 1600 }, { width: 1, height: 1 }]) {
      for (const viewport of [frame, { width: 320, height: 260 }, { width: 1, height: 1 }]) {
        for (const zoom of [1, 2, 8, 32]) {
          const transform = imageTransform({ zoom, center: { x: 0.35, y: 0.6 } }, size, viewport);
          for (const point of [{ x: 0, y: 0 }, { x: 0.3, y: 0.7 }, { x: 1, y: 1 }]) {
            const restored = toImagePoint(toScreenPoint(point, transform), transform);
            expect(restored.x).toBeCloseTo(point.x, 12);
            expect(restored.y).toBeCloseTo(point.y, 12);
          }
        }
      }
    }
    expect(insideImage(toImagePoint({ x: 400, y: 10 }, imageTransform(FIT_VIEWPORT, image, frame)))).toBe(false);
  });

  it("keeps the image point beneath a zoom anchor stationary away from clamped edges", () => {
    const view: ImageViewport = { zoom: 2, center: { x: 0.5, y: 0.5 } };
    const anchor = { x: 470, y: 350 };
    const before = toImagePoint(anchor, imageTransform(view, image, frame));
    const next = zoomViewport(view, image, frame, 5, anchor);
    const after = toImagePoint(anchor, imageTransform(next, image, frame));
    expect(after.x).toBeCloseTo(before.x, 12);
    expect(after.y).toBeCloseTo(before.y, 12);
  });

  it("clamps panning without modifying a stored rectangle or the relative zoom", () => {
    const view: ImageViewport = { zoom: 4, center: { x: 0.5, y: 0.5 } };
    const before = { ...region };
    const next = panViewport(view, image, frame, { x: 100000, y: -100000 });
    expect(next.zoom).toBe(4);
    expect(next.center.x).toBeGreaterThan(0);
    expect(next.center.y).toBeLessThan(1);
    expect(region).toEqual(before);
    expect(clampViewport({ zoom: -4, center: { x: 2, y: -1 } }, image, frame)).toEqual(FIT_VIEWPORT);
  });

  it("preserves image-relative focus across rotation and keyboard viewport changes", () => {
    const view: ImageViewport = { zoom: 8, center: { x: 0.4, y: 0.6 } };
    for (const viewport of [frame, { width: 400, height: 700 }, { width: 400, height: 250 }]) {
      const centered = toImagePoint({ x: viewport.width / 2, y: viewport.height / 2 }, imageTransform(view, image, viewport));
      expect(centered.x).toBeCloseTo(view.center.x, 12);
      expect(centered.y).toBeCloseTo(view.center.y, 12);
    }
  });

  it("creates equivalent rectangles in either drag direction and bounds their edges", () => {
    const a = { x: 0.2, y: 0.1 };
    const b = { x: 0.8, y: 0.6 };
    expect(rectangleFromCorners(a, b, image)).toEqual(rectangleFromCorners(b, a, image));
    expect(rectangleFromCorners({ x: -1, y: -1 }, { x: 2, y: 2 }, image)).toEqual({ x: 0, y: 0, width: 1, height: 1 });
    expect(rectangleFromCorners(a, a, image)).toBeNull();
    expect(rectangleFromCorners(a, { x: a.x + 0.00001, y: 0.7 }, image)).toBeNull();
  });

  it("moves without resizing and cannot move any part outside the source image", () => {
    const moved = moveRectangle(region, { x: 2, y: -2 });
    expect(moved).toEqual({ ...region, x: 0.9, y: 0 });
    expect(region).toEqual({ x: 0.2, y: 0.3, width: 0.1, height: 0.2 });
  });

  it.each<ResizeHandle>(["nw", "ne", "sw", "se"])("resizes %s without inversion or out-of-image geometry", (handle) => {
    for (const delta of [{ x: -10, y: -10 }, { x: 10, y: 10 }, { x: 0.05, y: 0.08 }]) {
      const result = resizeRectangle(region, handle, delta, image);
      expect(() => validateRectangle(result)).not.toThrow();
      expect(result.width).toBeGreaterThanOrEqual(1 / image.width - 1e-12);
      expect(result.height).toBeGreaterThanOrEqual(1 / image.height - 1e-12);
      if (handle.endsWith("w")) expect(result.x + result.width).toBeCloseTo(region.x + region.width, 12);
      else expect(result.x).toBe(region.x);
      if (handle.startsWith("n")) expect(result.y + result.height).toBeCloseTo(region.y + region.height, 12);
      else expect(result.y).toBe(region.y);
    }
  });

  it("zooms to a small region without exceeding the maximum", () => {
    const view = zoomToRegion({ x: 0.5, y: 0.5, width: 0.001, height: 0.001 }, image, frame);
    expect(view.zoom).toBe(32);
    expect(view.center.x).toBeCloseTo(0.5005, 12);
  });

  it("distinguishes overlapping interiors from touching edges", () => {
    expect(rectanglesOverlap(region, { x: 0.25, y: 0.35, width: 0.1, height: 0.1 })).toBe(true);
    expect(rectanglesOverlap(region, { x: region.x + region.width, y: 0.3, width: 0.1, height: 0.1 })).toBe(false);
  });

  it("rejects non-finite and invalid geometry before it can enter a draft", () => {
    expect(() => validateRectangle({ ...region, x: NaN })).toThrow();
    expect(() => validateRectangle({ ...region, width: -1 })).toThrow();
    expect(() => imageTransform(FIT_VIEWPORT, { width: 0, height: 100 }, frame)).toThrow();
    expect(() => zoomViewport(FIT_VIEWPORT, image, frame, Infinity, { x: 0, y: 0 })).toThrow();
    expect(() => moveRectangle(region, { x: NaN, y: 0 })).toThrow();
  });
});
