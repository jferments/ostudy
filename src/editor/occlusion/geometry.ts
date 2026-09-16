import type { NormalizedRect } from "../../ipc/questions";

export interface Point { x: number; y: number; }
export interface Size { width: number; height: number; }
/** Zoom relative to Fit; the normalized center survives viewport changes. */
export interface ImageViewport { zoom: number; center: Point; }
export interface ImageTransform { left: number; top: number; width: number; height: number; }
export type ResizeHandle = "nw" | "ne" | "sw" | "se";

export const MAX_ZOOM = 32;
export const FIT_VIEWPORT: ImageViewport = { zoom: 1, center: { x: 0.5, y: 0.5 } };
const PADDING = 16;

function finite(...values: number[]): void {
  if (!values.every(Number.isFinite)) throw new Error("Image geometry must be finite.");
}
function validSize(size: Size): void {
  finite(size.width, size.height);
  if (size.width <= 0 || size.height <= 0) throw new Error("Image dimensions must be positive.");
}
export function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}

function fittedSize(image: Size, frame: Size, zoom: number): Size {
  validSize(image);
  validSize(frame);
  const availableWidth = Math.max(1, frame.width - 2 * PADDING);
  const availableHeight = Math.max(1, frame.height - 2 * PADDING);
  const scale = Math.min(availableWidth / image.width, availableHeight / image.height) * zoom;
  return { width: image.width * scale, height: image.height * scale };
}

export function clampViewport(view: ImageViewport, image: Size, frame: Size): ImageViewport {
  finite(view.zoom, view.center.x, view.center.y);
  const zoom = clamp(view.zoom, 1, MAX_ZOOM);
  const fitted = fittedSize(image, frame, zoom);
  const minimumX = Math.min(0.5, Math.max(1, frame.width - 2 * PADDING) / (2 * fitted.width));
  const minimumY = Math.min(0.5, Math.max(1, frame.height - 2 * PADDING) / (2 * fitted.height));
  return {
    zoom,
    center: {
      x: clamp(view.center.x, minimumX, 1 - minimumX),
      y: clamp(view.center.y, minimumY, 1 - minimumY),
    },
  };
}

export function imageTransform(view: ImageViewport, image: Size, frame: Size): ImageTransform {
  const bounded = clampViewport(view, image, frame);
  const fitted = fittedSize(image, frame, bounded.zoom);
  return {
    left: frame.width / 2 - bounded.center.x * fitted.width,
    top: frame.height / 2 - bounded.center.y * fitted.height,
    ...fitted,
  };
}

/** Points and frame dimensions are CSS pixels, independent of devicePixelRatio. */
export function toImagePoint(point: Point, transform: ImageTransform): Point {
  finite(point.x, point.y, transform.left, transform.top);
  validSize(transform);
  return { x: (point.x - transform.left) / transform.width, y: (point.y - transform.top) / transform.height };
}
export function toScreenPoint(point: Point, transform: ImageTransform): Point {
  finite(point.x, point.y, transform.left, transform.top);
  validSize(transform);
  return { x: transform.left + point.x * transform.width, y: transform.top + point.y * transform.height };
}
export function insideImage(point: Point): boolean {
  return Number.isFinite(point.x) && Number.isFinite(point.y)
    && point.x >= 0 && point.x <= 1 && point.y >= 0 && point.y <= 1;
}
export function panViewport(view: ImageViewport, image: Size, frame: Size, delta: Point): ImageViewport {
  finite(delta.x, delta.y);
  const transform = imageTransform(view, image, frame);
  const bounded = clampViewport(view, image, frame);
  return clampViewport({ zoom: bounded.zoom, center: {
    x: bounded.center.x - delta.x / transform.width,
    y: bounded.center.y - delta.y / transform.height,
  } }, image, frame);
}
export function zoomViewport(view: ImageViewport, image: Size, frame: Size, zoom: number, anchor: Point): ImageViewport {
  finite(zoom, anchor.x, anchor.y);
  const transform = imageTransform(view, image, frame);
  const fixed = toImagePoint(anchor, transform);
  const nextZoom = clamp(zoom, 1, MAX_ZOOM);
  const fitted = fittedSize(image, frame, nextZoom);
  return clampViewport({ zoom: nextZoom, center: {
    x: fixed.x - (anchor.x - frame.width / 2) / fitted.width,
    y: fixed.y - (anchor.y - frame.height / 2) / fitted.height,
  } }, image, frame);
}

export function zoomToRegion(rect: NormalizedRect, image: Size, frame: Size): ImageViewport {
  validateRectangle(rect);
  const fitted = fittedSize(image, frame, 1);
  const zoom = Math.min(
    Math.max(1, frame.width - 80) / (fitted.width * rect.width),
    Math.max(1, frame.height - 80) / (fitted.height * rect.height),
  );
  return clampViewport({ zoom, center: { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 } }, image, frame);
}

export function validateRectangle(rect: NormalizedRect): void {
  finite(rect.x, rect.y, rect.width, rect.height);
  if (rect.x < 0 || rect.y < 0 || rect.width <= 0 || rect.height <= 0
    || rect.x + rect.width > 1 + 1e-12 || rect.y + rect.height > 1 + 1e-12) {
    throw new Error("A region must be a positive rectangle within the image.");
  }
}

export function rectangleFromCorners(a: Point, b: Point, image: Size): NormalizedRect | null {
  finite(a.x, a.y, b.x, b.y);
  validSize(image);
  const x = clamp(Math.min(a.x, b.x), 0, 1);
  const y = clamp(Math.min(a.y, b.y), 0, 1);
  const width = clamp(Math.max(a.x, b.x), 0, 1) - x;
  const height = clamp(Math.max(a.y, b.y), 0, 1) - y;
  if (width + 1e-12 < 1 / image.width || height + 1e-12 < 1 / image.height) return null;
  return { x, y, width, height };
}

export function moveRectangle(rect: NormalizedRect, delta: Point): NormalizedRect {
  validateRectangle(rect);
  finite(delta.x, delta.y);
  return { ...rect, x: clamp(rect.x + delta.x, 0, 1 - rect.width), y: clamp(rect.y + delta.y, 0, 1 - rect.height) };
}

export function resizeRectangle(rect: NormalizedRect, handle: ResizeHandle, delta: Point, image: Size): NormalizedRect {
  validateRectangle(rect);
  validSize(image);
  finite(delta.x, delta.y);
  const minimumWidth = Math.min(rect.width, 1 / image.width);
  const minimumHeight = Math.min(rect.height, 1 / image.height);
  const right = rect.x + rect.width;
  const bottom = rect.y + rect.height;
  const left = handle.endsWith("w") ? clamp(rect.x + delta.x, 0, right - minimumWidth) : rect.x;
  const top = handle.startsWith("n") ? clamp(rect.y + delta.y, 0, bottom - minimumHeight) : rect.y;
  const newRight = handle.endsWith("e") ? clamp(right + delta.x, rect.x + minimumWidth, 1) : right;
  const newBottom = handle.startsWith("s") ? clamp(bottom + delta.y, rect.y + minimumHeight, 1) : bottom;
  return { x: left, y: top, width: newRight - left, height: newBottom - top };
}

export function sameRectangle(a: NormalizedRect, b: NormalizedRect): boolean {
  return a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;
}
export function rectanglesOverlap(a: NormalizedRect, b: NormalizedRect): boolean {
  validateRectangle(a);
  validateRectangle(b);
  return a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
}
