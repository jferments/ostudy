import type {
  AnnotationShape,
  FrameShape,
  HighlightShape,
  InkShape,
  SegmentShape,
  TextShape,
} from "../../../ipc/documentAnnotations";

/** An annotation as the annotator holds it. */
export interface Annotation {
  id: string;
  pageNumber: number;
  shape: AnnotationShape;
  /** Drawing order: earlier annotations are drawn under later ones. */
  createdAt: number;
}

export interface Point { x: number; y: number; }
export interface Size { width: number; height: number; }
export interface Bounds { left: number; top: number; right: number; bottom: number; }

export type AnnotatorTool =
  | "select" | "lasso" | "pen" | "highlighter" | "text" | "sticky" | "line" | "arrow" | "rectangle" | "ellipse" | "eraser";
export type ShapeTool = "line" | "arrow" | "rectangle" | "ellipse";
/** What the eraser takes: whole annotations, or only the parts of pen and highlighter strokes it passes over. */
export type EraseMode = "whole" | "part";
export const SHAPE_TOOLS: readonly ShapeTool[] = ["line", "arrow", "rectangle", "ellipse"];

export function isShapeTool(tool: AnnotatorTool): tool is ShapeTool {
  return (SHAPE_TOOLS as readonly string[]).includes(tool);
}

// Limits the native owner enforces (document_annotations.rs); the annotator stays inside them.
const MAX_COORDINATE = 14_400;
export const MAX_STROKE_POINTS = 10_000;
export const MAX_TEXT_CHARS = 5_000;
export const MAX_STICKY_CHARS = 20_000;
const MAX_TEXT_RECTS = 2_000;
const STROKE_WIDTH_RANGE = [0.25, 64] as const;
const FONT_SIZE_RANGE = [6, 96] as const;

/** Text notes: line height as a multiple of the font size, and inner padding in points. */
export const TEXT_LINE_HEIGHT = 1.3;
export const TEXT_PADDING = 2;
export const DEFAULT_TEXT_WIDTH = 200;
export const MIN_TEXT_WIDTH = 24;
/** A sticky note's size on the page, in points; its text opens in full when tapped. */
export const STICKY_WIDTH = 120;
export const STICKY_HEIGHT = 72;
/** A collapsed sticky note's icon: a square this many points wide. */
export const STICKY_ICON_SIZE = 18;
/** Shapes smaller than this on both sides are taken as a slip and not kept. */
const MIN_SHAPE_EXTENT = 2;

export type StyleFamily = "pen" | "highlighter" | "shape" | "text" | "sticky";
/** A tool's look. `size` is the stroke width, or the font size for text, in points. */
export interface ToolStyle { color: string; size: number; }
export type ToolStyles = Readonly<Record<StyleFamily, ToolStyle>>;
export interface Swatch { color: string; label: string; }
export interface SizeChoice { size: number; label: string; }

const INK_SWATCHES: readonly Swatch[] = [
  { color: "#1f2328", label: "Black" },
  { color: "#2563eb", label: "Blue" },
  { color: "#dc2626", label: "Red" },
  { color: "#16a34a", label: "Green" },
  { color: "#9333ea", label: "Purple" },
  { color: "#ea580c", label: "Orange" },
];
const HIGHLIGHT_SWATCHES: readonly Swatch[] = [
  { color: "#fde047", label: "Yellow" },
  { color: "#86efac", label: "Green" },
  { color: "#f9a8d4", label: "Pink" },
  { color: "#93c5fd", label: "Blue" },
  { color: "#fdba74", label: "Orange" },
];
const STROKE_SIZES: readonly SizeChoice[] = [
  { size: 1.5, label: "Fine" },
  { size: 3, label: "Medium" },
  { size: 6, label: "Bold" },
];

export const STYLE_CHOICES: Readonly<Record<StyleFamily, { swatches: readonly Swatch[]; sizes: readonly SizeChoice[] }>> = {
  pen: { swatches: INK_SWATCHES, sizes: STROKE_SIZES },
  highlighter: {
    swatches: HIGHLIGHT_SWATCHES,
    sizes: [{ size: 10, label: "Narrow" }, { size: 16, label: "Medium" }, { size: 24, label: "Wide" }],
  },
  shape: { swatches: INK_SWATCHES, sizes: STROKE_SIZES },
  text: {
    swatches: INK_SWATCHES,
    sizes: [{ size: 10, label: "Small" }, { size: 14, label: "Medium" }, { size: 20, label: "Large" }],
  },
  // A sticky note's paper, and the size of the preview text on it.
  sticky: {
    swatches: HIGHLIGHT_SWATCHES,
    sizes: [{ size: 8, label: "Small" }, { size: 10, label: "Medium" }, { size: 12, label: "Large" }],
  },
};

export const DEFAULT_STYLES: ToolStyles = {
  pen: { color: "#1f2328", size: 3 },
  highlighter: { color: "#fde047", size: 16 },
  shape: { color: "#dc2626", size: 3 },
  text: { color: "#1f2328", size: 14 },
  sticky: { color: "#fde047", size: 10 },
};

export function styleFamilyForTool(tool: AnnotatorTool): StyleFamily | null {
  if (tool === "pen" || tool === "highlighter" || tool === "text" || tool === "sticky") return tool;
  return isShapeTool(tool) ? "shape" : null;
}

export function styleFamilyForShape(shape: AnnotationShape): StyleFamily {
  if (shape.kind === "ink") return "pen";
  if (shape.kind === "highlight") return "highlighter";
  if (shape.kind === "text") return shape.sticky ? "sticky" : "text";
  return "shape";
}

/** Whether a text note is drawn as a sticky note. */
export function isStickyNote(shape: AnnotationShape): shape is TextShape & { sticky: true } {
  return shape.kind === "text" && shape.sticky === true;
}

/** Whether a sticky note is folded down to its small icon. */
export function isCollapsedSticky(shape: AnnotationShape): boolean {
  return isStickyNote(shape) && shape.collapsed === true;
}

/** How wide a text note shows on the page: a collapsed sticky note is only its icon. */
export function noteWidth(shape: TextShape): number {
  return shape.collapsed ? STICKY_ICON_SIZE : shape.width;
}

/** A sticky note's height on the page. */
export function stickyHeight(shape: TextShape): number {
  return shape.collapsed ? STICKY_ICON_SIZE : STICKY_HEIGHT;
}

export function shapeStyle(shape: AnnotationShape): ToolStyle {
  return { color: shape.color, size: shape.kind === "text" ? shape.fontSize : shape.strokeWidth };
}

/** The shape with a new color or size; geometry is unchanged. */
export function restyle(shape: AnnotationShape, style: Partial<ToolStyle>): AnnotationShape {
  const color = style.color ?? shape.color;
  if (shape.kind === "text") return { ...shape, color, fontSize: style.size ?? shape.fontSize };
  return { ...shape, color, strokeWidth: style.size ?? shape.strokeWidth };
}

/** Coordinates are stored to a hundredth of a point. */
export function round(value: number): number {
  return Math.round(value * 100) / 100;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}

/** A point on the page, kept inside it. */
export function clampToPage(point: Point, page: Size): Point {
  return {
    x: round(clamp(point.x, 0, Math.min(page.width, MAX_COORDINATE))),
    y: round(clamp(point.y, 0, Math.min(page.height, MAX_COORDINATE))),
  };
}

export function createAnnotationId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

/** A text note's height from its text, for pages whose notes have not been laid out yet. */
export function estimateTextHeight(shape: TextShape): number {
  if (shape.sticky) return stickyHeight(shape);
  const perLine = Math.max(1, Math.floor((shape.width - 2 * TEXT_PADDING) / (shape.fontSize * 0.55)));
  const lines = shape.text.split("\n")
    .reduce((total, line) => total + Math.max(1, Math.ceil(line.length / perLine)), 0);
  return lines * shape.fontSize * TEXT_LINE_HEIGHT + 2 * TEXT_PADDING;
}

function pointBounds(points: readonly number[], stride: number): Bounds {
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (let index = 0; index + 1 < points.length; index += stride) {
    const x = points[index] ?? 0;
    const y = points[index + 1] ?? 0;
    left = Math.min(left, x);
    top = Math.min(top, y);
    right = Math.max(right, x);
    bottom = Math.max(bottom, y);
  }
  return Number.isFinite(left) ? { left, top, right, bottom } : { left: 0, top: 0, right: 0, bottom: 0 };
}

function rectBounds(rects: readonly number[]): Bounds {
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (let index = 0; index + 3 < rects.length; index += 4) {
    const x = rects[index] ?? 0;
    const y = rects[index + 1] ?? 0;
    left = Math.min(left, x);
    top = Math.min(top, y);
    right = Math.max(right, x + (rects[index + 2] ?? 0));
    bottom = Math.max(bottom, y + (rects[index + 3] ?? 0));
  }
  return Number.isFinite(left) ? { left, top, right, bottom } : { left: 0, top: 0, right: 0, bottom: 0 };
}

/** Whether a highlight covers the page's text with boxes rather than following a free stroke. */
export function isTextHighlight(shape: AnnotationShape): shape is HighlightShape & { rects: number[]; text: string } {
  return shape.kind === "highlight" && shape.rects !== undefined;
}

function pad(bounds: Bounds, amount: number): Bounds {
  return {
    left: bounds.left - amount,
    top: bounds.top - amount,
    right: bounds.right + amount,
    bottom: bounds.bottom + amount,
  };
}

/** Where the shape's own geometry lies, without its stroke. */
export function shapeExtent(shape: AnnotationShape, textHeight: number): Bounds {
  switch (shape.kind) {
    case "ink": return pointBounds(shape.points, 3);
    case "highlight": return shape.rects ? rectBounds(shape.rects) : pointBounds(shape.points, 2);
    case "line":
    case "arrow":
      return {
        left: Math.min(shape.x1, shape.x2),
        top: Math.min(shape.y1, shape.y2),
        right: Math.max(shape.x1, shape.x2),
        bottom: Math.max(shape.y1, shape.y2),
      };
    case "rectangle":
    case "ellipse":
      return { left: shape.x, top: shape.y, right: shape.x + shape.width, bottom: shape.y + shape.height };
    case "text":
      return { left: shape.x, top: shape.y, right: shape.x + noteWidth(shape), bottom: shape.y + textHeight };
  }
}

/** Everything the shape paints. A full-pressure ink stroke is half again as wide as its width. */
export function shapeBounds(shape: AnnotationShape, textHeight: number): Bounds {
  const extent = shapeExtent(shape, textHeight);
  switch (shape.kind) {
    case "ink": return pad(extent, shape.strokeWidth * 0.75);
    case "arrow": return pad(extent, Math.max(shape.strokeWidth / 2, arrowHeadLength(shape)));
    case "text": return extent;
    case "highlight": return shape.rects ? extent : pad(extent, shape.strokeWidth / 2);
    default: return pad(extent, shape.strokeWidth / 2);
  }
}

function distanceToSegment(point: Point, ax: number, ay: number, bx: number, by: number): number {
  const dx = bx - ax;
  const dy = by - ay;
  const lengthSquared = dx * dx + dy * dy;
  const t = lengthSquared === 0 ? 0 : clamp(((point.x - ax) * dx + (point.y - ay) * dy) / lengthSquared, 0, 1);
  return Math.hypot(point.x - (ax + t * dx), point.y - (ay + t * dy));
}

function distanceToPolyline(point: Point, points: readonly number[], stride: number): number {
  let best = Infinity;
  if (points.length < 2) return best;
  if (points.length < stride * 2) return Math.hypot(point.x - (points[0] ?? 0), point.y - (points[1] ?? 0));
  for (let index = stride; index + 1 < points.length; index += stride) {
    best = Math.min(best, distanceToSegment(
      point,
      points[index - stride] ?? 0, points[index - stride + 1] ?? 0,
      points[index] ?? 0, points[index + 1] ?? 0,
    ));
  }
  return best;
}

function distanceToBox(point: Point, box: Bounds): number {
  const dx = Math.max(box.left - point.x, 0, point.x - box.right);
  const dy = Math.max(box.top - point.y, 0, point.y - box.bottom);
  return Math.hypot(dx, dy);
}

/** How far a point is from what the shape paints, in points; 0 on it. */
export function distanceToShape(shape: AnnotationShape, point: Point, textHeight: number): number {
  switch (shape.kind) {
    case "ink":
      return Math.max(0, distanceToPolyline(point, shape.points, 3) - shape.strokeWidth / 2);
    case "highlight": {
      if (!shape.rects) return Math.max(0, distanceToPolyline(point, shape.points, 2) - shape.strokeWidth / 2);
      let best = Infinity;
      for (let index = 0; index + 3 < shape.rects.length; index += 4) {
        best = Math.min(best, distanceToBox(point, rectBounds(shape.rects.slice(index, index + 4))));
      }
      return best;
    }
    case "line":
    case "arrow":
      return Math.max(0, distanceToSegment(point, shape.x1, shape.y1, shape.x2, shape.y2) - shape.strokeWidth / 2);
    case "rectangle": {
      const box = shapeExtent(shape, 0);
      const inside = point.x >= box.left && point.x <= box.right && point.y >= box.top && point.y <= box.bottom;
      const edge = inside
        ? Math.min(point.x - box.left, box.right - point.x, point.y - box.top, box.bottom - point.y)
        : distanceToBox(point, box);
      return Math.max(0, edge - shape.strokeWidth / 2);
    }
    case "ellipse": {
      const rx = shape.width / 2;
      const ry = shape.height / 2;
      const radius = Math.hypot((point.x - shape.x - rx) / Math.max(rx, 0.01), (point.y - shape.y - ry) / Math.max(ry, 0.01));
      return Math.max(0, Math.abs(radius - 1) * Math.min(rx, ry) - shape.strokeWidth / 2);
    }
    case "text":
      return distanceToBox(point, shapeExtent(shape, textHeight));
  }
}

/**
 * The annotation drawn on top at a point, within `slop` points of it. Text notes
 * are drawn above strokes and shapes, so they are found first.
 */
export function topmostAt(
  annotations: readonly Annotation[],
  point: Point,
  slop: number,
  textHeight: (annotation: Annotation) => number,
  accept: (annotation: Annotation) => boolean = () => true,
): Annotation | null {
  for (const layer of ["text", "graphics"] as const) {
    for (let index = annotations.length - 1; index >= 0; index -= 1) {
      const annotation = annotations[index];
      if (!annotation || (annotation.shape.kind === "text") !== (layer === "text") || !accept(annotation)) continue;
      const height = textHeight(annotation);
      const bounds = pad(shapeBounds(annotation.shape, height), slop);
      if (point.x < bounds.left || point.x > bounds.right || point.y < bounds.top || point.y > bounds.bottom) continue;
      if (distanceToShape(annotation.shape, point, height) <= slop) return annotation;
    }
  }
  return null;
}

/** Whether the eraser can take part of this shape: a pen stroke or a free highlighter stroke. */
export function isCuttable(shape: AnnotationShape): shape is InkShape | HighlightShape {
  return shape.kind === "ink" || (shape.kind === "highlight" && !isTextHighlight(shape) && shape.points.length > 0);
}

/**
 * What is left of a pen or free highlighter stroke after the eraser covers a
 * circle: the parts of it outside the circle, each its own stroke, cut where
 * the stroke's edge meets the circle. Null when the circle misses the stroke.
 */
export function cutStroke(shape: InkShape | HighlightShape, center: Point, radius: number): Array<InkShape | HighlightShape> | null {
  const stride = shape.kind === "ink" ? 3 : 2;
  const points = shape.points;
  const count = Math.floor(points.length / stride);
  if (count === 0) return null;
  const reach = radius + shape.strokeWidth / 2;
  const value = (index: number) => points[index] ?? 0;
  const at = (index: number): number[] => Array.from({ length: stride }, (_, axis) => value(index * stride + axis));
  const inside = (index: number) => Math.hypot(value(index * stride) - center.x, value(index * stride + 1) - center.y) <= reach;
  const between = (from: readonly number[], to: readonly number[], t: number) =>
    from.map((start, axis) => round(start + ((to[axis] ?? start) - start) * t));

  const runs: number[][] = [];
  let run: number[] = inside(0) ? [] : at(0);
  let cut = inside(0);
  const close = () => {
    if (run.length >= stride * 2) runs.push(run);
    run = [];
  };
  for (let index = 1; index < count; index += 1) {
    const from = at(index - 1);
    const to = at(index);
    const span = segmentInsideCircle(
      value((index - 1) * stride), value((index - 1) * stride + 1), value(index * stride), value(index * stride + 1), center, reach,
    );
    if (!span) {
      run.push(...to);
      continue;
    }
    cut = true;
    const [enter, leave] = span;
    if (enter > 0) run.push(...between(from, to, enter));
    close();
    if (leave < 1) run.push(...between(from, to, leave), ...to);
  }
  if (!cut) return null;
  close();
  return runs.map((kept) => ({ ...shape, points: kept }));
}

/** The part of the segment from (ax, ay) to (bx, by) inside a circle, as fractions of its length. */
function segmentInsideCircle(ax: number, ay: number, bx: number, by: number, center: Point, radius: number): [number, number] | null {
  const dx = bx - ax;
  const dy = by - ay;
  const fx = ax - center.x;
  const fy = ay - center.y;
  const a = dx * dx + dy * dy;
  const c = fx * fx + fy * fy - radius * radius;
  if (a === 0) return c <= 0 ? [0, 1] : null;
  const b = 2 * (fx * dx + fy * dy);
  const discriminant = b * b - 4 * a * c;
  if (discriminant < 0) return null;
  const root = Math.sqrt(discriminant);
  const enter = Math.max(0, (-b - root) / (2 * a));
  const leave = Math.min(1, (-b + root) / (2 * a));
  return enter < leave || (enter === leave && c <= 0) ? [enter, leave] : null;
}

function mapPoints(points: readonly number[], stride: number, dx: number, dy: number): number[] {
  return points.map((value, index) => {
    const axis = index % stride;
    if (axis === 0) return round(value + dx);
    if (axis === 1) return round(value + dy);
    return value;
  });
}

/** The shape moved by (dx, dy) points, as far as it can go without leaving the page. */
export function moveWithinPage(
  shape: AnnotationShape,
  dx: number,
  dy: number,
  page: Size,
  textHeight: number,
): AnnotationShape {
  const extent = shapeExtent(shape, textHeight);
  const right = Math.min(page.width, MAX_COORDINATE);
  const bottom = Math.min(page.height, MAX_COORDINATE);
  const x = clamp(dx, -extent.left, Math.max(-extent.left, right - extent.right));
  const y = clamp(dy, -extent.top, Math.max(-extent.top, bottom - extent.bottom));
  switch (shape.kind) {
    case "ink": return { ...shape, points: mapPoints(shape.points, 3, x, y) };
    case "highlight":
      return shape.rects
        ? { ...shape, rects: mapPoints(shape.rects, 4, x, y) }
        : { ...shape, points: mapPoints(shape.points, 2, x, y) };
    case "line":
    case "arrow":
      return { ...shape, x1: round(shape.x1 + x), y1: round(shape.y1 + y), x2: round(shape.x2 + x), y2: round(shape.y2 + y) };
    default:
      return { ...shape, x: round(shape.x + x), y: round(shape.y + y) };
  }
}

/** Whether a point lies inside a closed loop of `x, y` pairs. */
export function insideLoop(point: Point, loop: readonly number[]): boolean {
  let inside = false;
  const count = Math.floor(loop.length / 2);
  for (let index = 0, previous = count - 1; index < count; previous = index, index += 1) {
    const x1 = loop[index * 2] ?? 0;
    const y1 = loop[index * 2 + 1] ?? 0;
    const x2 = loop[previous * 2] ?? 0;
    const y2 = loop[previous * 2 + 1] ?? 0;
    if ((y1 > point.y) !== (y2 > point.y) && point.x < (x2 - x1) * (point.y - y1) / (y2 - y1) + x1) inside = !inside;
  }
  return inside;
}

/** The annotations a lasso loop takes: those whose middle it surrounds. */
export function annotationsInLoop(
  annotations: readonly Annotation[],
  loop: readonly number[],
  textHeight: (annotation: Annotation) => number,
): Annotation[] {
  if (loop.length < 6) return [];
  return annotations.filter((annotation) => {
    const bounds = shapeExtent(annotation.shape, textHeight(annotation));
    return insideLoop({ x: (bounds.left + bounds.right) / 2, y: (bounds.top + bounds.bottom) / 2 }, loop);
  });
}

/** A dragged selection rectangle from one corner to the other, as a loop. */
export function rectangleLoop(from: Point, to: Point): number[] {
  return [from.x, from.y, to.x, from.y, to.x, to.y, from.x, to.y];
}

/**
 * The topmost box or ellipse whose inside holds a point, so a frame can be
 * picked up by its middle as well as by its outline.
 */
export function frameAround(
  annotations: readonly Annotation[],
  point: Point,
  accept: (annotation: Annotation) => boolean = () => true,
): Annotation | null {
  for (let index = annotations.length - 1; index >= 0; index -= 1) {
    const annotation = annotations[index];
    if (!annotation || !accept(annotation)) continue;
    const { shape } = annotation;
    if (shape.kind === "rectangle") {
      if (point.x >= shape.x && point.x <= shape.x + shape.width && point.y >= shape.y && point.y <= shape.y + shape.height) {
        return annotation;
      }
    } else if (shape.kind === "ellipse") {
      const rx = shape.width / 2;
      const ry = shape.height / 2;
      if (Math.hypot((point.x - shape.x - rx) / Math.max(rx, 0.01), (point.y - shape.y - ry) / Math.max(ry, 0.01)) <= 1) {
        return annotation;
      }
    }
  }
  return null;
}

/** The box around a group of annotations, or null for none. */
export function groupBounds(group: readonly Annotation[], textHeight: (annotation: Annotation) => number): Bounds | null {
  if (group.length === 0) return null;
  const all = group.map((annotation) => shapeBounds(annotation.shape, textHeight(annotation)));
  return {
    left: Math.min(...all.map((bounds) => bounds.left)),
    top: Math.min(...all.map((bounds) => bounds.top)),
    right: Math.max(...all.map((bounds) => bounds.right)),
    bottom: Math.max(...all.map((bounds) => bounds.bottom)),
  };
}

/** A group moved together by (dx, dy) points, as far as all of it can go without leaving the page. */
export function moveGroupWithinPage(
  group: readonly Annotation[],
  dx: number,
  dy: number,
  page: Size,
  textHeight: (annotation: Annotation) => number,
): Annotation[] {
  const extents = group.map((annotation) => shapeExtent(annotation.shape, textHeight(annotation)));
  if (extents.length === 0) return [];
  const right = Math.min(page.width, MAX_COORDINATE);
  const bottom = Math.min(page.height, MAX_COORDINATE);
  const left = Math.min(...extents.map((extent) => extent.left));
  const top = Math.min(...extents.map((extent) => extent.top));
  const far = Math.max(...extents.map((extent) => extent.right));
  const low = Math.max(...extents.map((extent) => extent.bottom));
  const x = clamp(dx, -left, Math.max(-left, right - far));
  const y = clamp(dy, -top, Math.max(-top, bottom - low));
  return group.map((annotation) => ({
    ...annotation,
    shape: moveWithinPage(annotation.shape, x, y, page, textHeight(annotation)),
  }));
}

export type Handle = "start" | "end" | "nw" | "ne" | "sw" | "se" | "width";

/** The points a selected shape can be reshaped from. Strokes only move. */
export function handlesFor(shape: AnnotationShape, textHeight: number): Array<{ handle: Handle; point: Point }> {
  switch (shape.kind) {
    case "line":
    case "arrow":
      return [{ handle: "start", point: { x: shape.x1, y: shape.y1 } }, { handle: "end", point: { x: shape.x2, y: shape.y2 } }];
    case "rectangle":
    case "ellipse":
      return [
        { handle: "nw", point: { x: shape.x, y: shape.y } },
        { handle: "ne", point: { x: shape.x + shape.width, y: shape.y } },
        { handle: "sw", point: { x: shape.x, y: shape.y + shape.height } },
        { handle: "se", point: { x: shape.x + shape.width, y: shape.y + shape.height } },
      ];
    case "text":
      // A sticky note keeps its size: only its text grows.
      return shape.sticky ? [] : [{ handle: "width", point: { x: shape.x + shape.width, y: shape.y + textHeight / 2 } }];
    default:
      return [];
  }
}

/** The end of a line from `origin` toward `point`, turned to the nearest 45 degrees. */
export function snapAngle(origin: Point, point: Point): Point {
  const length = Math.hypot(point.x - origin.x, point.y - origin.y);
  const step = Math.PI / 4;
  const angle = Math.round(Math.atan2(point.y - origin.y, point.x - origin.x) / step) * step;
  return { x: origin.x + Math.cos(angle) * length, y: origin.y + Math.sin(angle) * length };
}

/** The box between two corners; `square` makes its sides equal. */
export function frameBetween(a: Point, b: Point, square: boolean): { x: number; y: number; width: number; height: number } {
  let dx = b.x - a.x;
  let dy = b.y - a.y;
  if (square) {
    const side = Math.max(Math.abs(dx), Math.abs(dy));
    dx = (dx < 0 ? -1 : 1) * side;
    dy = (dy < 0 ? -1 : 1) * side;
  }
  return {
    x: round(Math.min(a.x, a.x + dx)),
    y: round(Math.min(a.y, a.y + dy)),
    width: round(Math.abs(dx)),
    height: round(Math.abs(dy)),
  };
}

function clampFrame<T extends { x: number; y: number; width: number; height: number }>(frame: T, page: Size): T {
  const x = clamp(frame.x, 0, page.width);
  const y = clamp(frame.y, 0, page.height);
  return {
    ...frame,
    x: round(x),
    y: round(y),
    width: round(Math.min(frame.width, page.width - x)),
    height: round(Math.min(frame.height, page.height - y)),
  };
}

/** A shape drawn by dragging a shape tool from `start` to `end`. */
export function shapeFromDrag(
  tool: ShapeTool,
  start: Point,
  end: Point,
  constrain: boolean,
  style: ToolStyle,
  page: Size,
): SegmentShape | FrameShape {
  if (tool === "line" || tool === "arrow") {
    const tip = clampToPage(constrain ? snapAngle(start, end) : end, page);
    return { kind: tool, x1: start.x, y1: start.y, x2: tip.x, y2: tip.y, color: style.color, strokeWidth: style.size };
  }
  return clampFrame({ kind: tool, ...frameBetween(start, end, constrain), color: style.color, strokeWidth: style.size }, page);
}

/** Whether a dragged shape is big enough to keep. A box needs some size both ways. */
export function isDeliberateShape(shape: SegmentShape | FrameShape): boolean {
  if ("x1" in shape) return Math.hypot(shape.x2 - shape.x1, shape.y2 - shape.y1) >= MIN_SHAPE_EXTENT;
  return shape.width > 0 && shape.height > 0 && (shape.width >= MIN_SHAPE_EXTENT || shape.height >= MIN_SHAPE_EXTENT);
}

/** The shape reshaped by dragging one of its handles to `point` (already on the page). */
export function resizeShape(
  shape: AnnotationShape,
  handle: Handle,
  point: Point,
  constrain: boolean,
  page: Size,
): AnnotationShape {
  switch (shape.kind) {
    case "line":
    case "arrow": {
      if (handle !== "start" && handle !== "end") return shape;
      const fixed = handle === "start" ? { x: shape.x2, y: shape.y2 } : { x: shape.x1, y: shape.y1 };
      const moved = clampToPage(constrain ? snapAngle(fixed, point) : point, page);
      if (moved.x === fixed.x && moved.y === fixed.y) return shape;
      return handle === "start" ? { ...shape, x1: moved.x, y1: moved.y } : { ...shape, x2: moved.x, y2: moved.y };
    }
    case "rectangle":
    case "ellipse": {
      const opposite: Partial<Record<Handle, Point>> = {
        nw: { x: shape.x + shape.width, y: shape.y + shape.height },
        ne: { x: shape.x, y: shape.y + shape.height },
        sw: { x: shape.x + shape.width, y: shape.y },
        se: { x: shape.x, y: shape.y },
      };
      const fixed = opposite[handle];
      if (!fixed) return shape;
      const frame = clampFrame({ ...shape, ...frameBetween(fixed, point, constrain) }, page);
      return frame.width > 0 && frame.height > 0 ? frame : shape;
    }
    case "text": {
      if (handle !== "width") return shape;
      const width = round(clamp(point.x - shape.x, MIN_TEXT_WIDTH, Math.max(MIN_TEXT_WIDTH, page.width - shape.x)));
      return { ...shape, width };
    }
    default:
      return shape;
  }
}

/** Where an arrow's head starts and the two corners beside its tip. */
export function arrowHeadLength(shape: SegmentShape): number {
  return Math.max(6, shape.strokeWidth * 4);
}

export function arrowHead(shape: SegmentShape): { base: Point; left: Point; right: Point } {
  const length = arrowHeadLength(shape);
  const angle = Math.atan2(shape.y2 - shape.y1, shape.x2 - shape.x1);
  const spread = Math.PI / 7;
  const back = Math.min(length * Math.cos(spread) * 0.8, Math.hypot(shape.x2 - shape.x1, shape.y2 - shape.y1));
  return {
    base: { x: shape.x2 - Math.cos(angle) * back, y: shape.y2 - Math.sin(angle) * back },
    left: { x: shape.x2 - Math.cos(angle - spread) * length, y: shape.y2 - Math.sin(angle - spread) * length },
    right: { x: shape.x2 - Math.cos(angle + spread) * length, y: shape.y2 - Math.sin(angle + spread) * length },
  };
}

/**
 * Drops points that do not change a highlight's path by more than `tolerance`
 * points (Ramer-Douglas-Peucker), so long strokes stay small when stored.
 */
export function simplifyPairs(points: readonly number[], tolerance: number): number[] {
  const count = Math.floor(points.length / 2);
  if (count <= 2) return points.slice(0, count * 2);
  const keep = new Array<boolean>(count).fill(false);
  keep[0] = true;
  keep[count - 1] = true;
  const stack: Array<[number, number]> = [[0, count - 1]];
  while (stack.length > 0) {
    const [first, last] = stack.pop() ?? [0, 0];
    let farthest = -1;
    let distance = tolerance;
    for (let index = first + 1; index < last; index += 1) {
      const candidate = distanceToSegment(
        { x: points[index * 2] ?? 0, y: points[index * 2 + 1] ?? 0 },
        points[first * 2] ?? 0, points[first * 2 + 1] ?? 0,
        points[last * 2] ?? 0, points[last * 2 + 1] ?? 0,
      );
      if (candidate > distance) {
        distance = candidate;
        farthest = index;
      }
    }
    if (farthest >= 0) {
      keep[farthest] = true;
      stack.push([first, farthest], [farthest, last]);
    }
  }
  return keep.flatMap((kept, index) => kept ? [points[index * 2] ?? 0, points[index * 2 + 1] ?? 0] : []);
}

const COLOR = /^#[0-9a-f]{6}$/;
const CONTROL = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/;

function coordinate(value: number): boolean {
  return Number.isFinite(value) && value >= 0 && value <= MAX_COORDINATE;
}

function strokeWidthAllowed(width: number): boolean {
  return Number.isFinite(width) && width >= STROKE_WIDTH_RANGE[0] && width <= STROKE_WIDTH_RANGE[1];
}

function pointsAllowed(points: readonly number[], stride: number): boolean {
  if (points.length === 0 || points.length % stride !== 0 || points.length / stride > MAX_STROKE_POINTS) return false;
  return points.every((value, index) => index % stride === 2
    ? Number.isFinite(value) && value >= 0 && value <= 1
    : coordinate(value));
}

function rectsAllowed(rects: readonly number[] | undefined): boolean {
  if (!rects || rects.length === 0 || rects.length % 4 !== 0 || rects.length / 4 > MAX_TEXT_RECTS) return false;
  for (let index = 0; index < rects.length; index += 4) {
    const [x = NaN, y = NaN, width = NaN, height = NaN] = rects.slice(index, index + 4);
    if (!coordinate(x) || !coordinate(y) || !(width > 0) || !(height > 0)
      || x + width > MAX_COORDINATE || y + height > MAX_COORDINATE) return false;
  }
  return true;
}

function quotedTextAllowed(text: string | undefined): boolean {
  return text !== undefined && text.trim().length > 0 && [...text].length <= MAX_TEXT_CHARS && !/\p{Cc}/u.test(text);
}

/** Whether the native owner will accept the shape, so a save never fails on it. */
export function isStorable(shape: AnnotationShape): boolean {
  if (!COLOR.test(shape.color)) return false;
  switch (shape.kind) {
    case "ink":
      return pointsAllowed(shape.points, 3) && strokeWidthAllowed(shape.strokeWidth);
    case "highlight":
      if (shape.rects === undefined && shape.text === undefined) {
        return pointsAllowed(shape.points, 2) && strokeWidthAllowed(shape.strokeWidth);
      }
      return shape.points.length === 0 && rectsAllowed(shape.rects) && quotedTextAllowed(shape.text)
        && strokeWidthAllowed(shape.strokeWidth);
    case "line":
    case "arrow":
      return [shape.x1, shape.y1, shape.x2, shape.y2].every(coordinate)
        && (shape.x1 !== shape.x2 || shape.y1 !== shape.y2)
        && strokeWidthAllowed(shape.strokeWidth);
    case "rectangle":
    case "ellipse":
      return coordinate(shape.x) && coordinate(shape.y)
        && shape.width > 0 && shape.height > 0
        && shape.x + shape.width <= MAX_COORDINATE && shape.y + shape.height <= MAX_COORDINATE
        && strokeWidthAllowed(shape.strokeWidth);
    case "text":
      return coordinate(shape.x) && coordinate(shape.y)
        && shape.width > 0 && shape.x + shape.width <= MAX_COORDINATE
        && shape.text.trim().length > 0
        && (shape.sticky === undefined || shape.sticky === true)
        && (shape.collapsed === undefined || (shape.collapsed === true && shape.sticky === true))
        && [...shape.text].length <= (shape.sticky ? MAX_STICKY_CHARS : MAX_TEXT_CHARS)
        && !CONTROL.test(shape.text)
        && Number.isFinite(shape.fontSize)
        && shape.fontSize >= FONT_SIZE_RANGE[0] && shape.fontSize <= FONT_SIZE_RANGE[1];
  }
}

/** Typed text as a note may hold it: line breaks and tabs stay, other control characters go. */
export function cleanNoteText(text: string): string {
  return text.replaceAll("\r\n", "\n").replace(new RegExp(CONTROL.source, "g"), "");
}

const KIND_NAMES: Readonly<Record<AnnotationShape["kind"], string>> = {
  ink: "Ink",
  highlight: "Highlight",
  line: "Line",
  arrow: "Arrow",
  rectangle: "Box",
  ellipse: "Ellipse",
  text: "Text note",
};

/** A short name for an annotation, for assistive technology. */
export function describeAnnotation(annotation: Annotation): string {
  const kind = isStickyNote(annotation.shape) ? "Sticky note" : KIND_NAMES[annotation.shape.kind];
  const name = `${kind} on page ${annotation.pageNumber}`;
  const quoted = annotation.shape.kind === "text" ? annotation.shape.text
    : annotation.shape.kind === "highlight" ? annotation.shape.text : undefined;
  if (quoted === undefined) return name;
  const text = quoted.replace(/\s+/g, " ").trim();
  return `${name}: ${text.length > 60 ? `${text.slice(0, 59)}…` : text}`;
}

export type { AnnotationShape, FrameShape, HighlightShape, InkShape, SegmentShape, TextShape };
