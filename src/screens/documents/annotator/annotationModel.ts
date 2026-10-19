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
  | "select" | "pen" | "highlighter" | "text" | "line" | "arrow" | "rectangle" | "ellipse" | "eraser";
export type ShapeTool = "line" | "arrow" | "rectangle" | "ellipse";
export const SHAPE_TOOLS: readonly ShapeTool[] = ["line", "arrow", "rectangle", "ellipse"];

export function isShapeTool(tool: AnnotatorTool): tool is ShapeTool {
  return (SHAPE_TOOLS as readonly string[]).includes(tool);
}

// Limits the native owner enforces (document_annotations.rs); the annotator stays inside them.
const MAX_COORDINATE = 14_400;
export const MAX_STROKE_POINTS = 10_000;
export const MAX_TEXT_CHARS = 5_000;
const STROKE_WIDTH_RANGE = [0.25, 64] as const;
const FONT_SIZE_RANGE = [6, 96] as const;

/** Text notes: line height as a multiple of the font size, and inner padding in points. */
export const TEXT_LINE_HEIGHT = 1.3;
export const TEXT_PADDING = 2;
export const DEFAULT_TEXT_WIDTH = 200;
export const MIN_TEXT_WIDTH = 24;
/** Shapes smaller than this on both sides are taken as a slip and not kept. */
const MIN_SHAPE_EXTENT = 2;

export type StyleFamily = "pen" | "highlighter" | "shape" | "text";
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
};

export const DEFAULT_STYLES: ToolStyles = {
  pen: { color: "#1f2328", size: 3 },
  highlighter: { color: "#fde047", size: 16 },
  shape: { color: "#dc2626", size: 3 },
  text: { color: "#1f2328", size: 14 },
};

export function styleFamilyForTool(tool: AnnotatorTool): StyleFamily | null {
  if (tool === "pen" || tool === "highlighter" || tool === "text") return tool;
  return isShapeTool(tool) ? "shape" : null;
}

export function styleFamilyForShape(shape: AnnotationShape): StyleFamily {
  if (shape.kind === "ink") return "pen";
  if (shape.kind === "highlight") return "highlighter";
  return shape.kind === "text" ? "text" : "shape";
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
    case "highlight": return pointBounds(shape.points, 2);
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
      return { left: shape.x, top: shape.y, right: shape.x + shape.width, bottom: shape.y + textHeight };
  }
}

/** Everything the shape paints. A full-pressure ink stroke is half again as wide as its width. */
export function shapeBounds(shape: AnnotationShape, textHeight: number): Bounds {
  const extent = shapeExtent(shape, textHeight);
  switch (shape.kind) {
    case "ink": return pad(extent, shape.strokeWidth * 0.75);
    case "arrow": return pad(extent, Math.max(shape.strokeWidth / 2, arrowHeadLength(shape)));
    case "text": return extent;
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
    case "highlight":
      return Math.max(0, distanceToPolyline(point, shape.points, 2) - shape.strokeWidth / 2);
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
    case "highlight": return { ...shape, points: mapPoints(shape.points, 2, x, y) };
    case "line":
    case "arrow":
      return { ...shape, x1: round(shape.x1 + x), y1: round(shape.y1 + y), x2: round(shape.x2 + x), y2: round(shape.y2 + y) };
    default:
      return { ...shape, x: round(shape.x + x), y: round(shape.y + y) };
  }
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
      return [{ handle: "width", point: { x: shape.x + shape.width, y: shape.y + textHeight / 2 } }];
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

/** Whether the native owner will accept the shape, so a save never fails on it. */
export function isStorable(shape: AnnotationShape): boolean {
  if (!COLOR.test(shape.color)) return false;
  switch (shape.kind) {
    case "ink":
      return pointsAllowed(shape.points, 3) && strokeWidthAllowed(shape.strokeWidth);
    case "highlight":
      return pointsAllowed(shape.points, 2) && strokeWidthAllowed(shape.strokeWidth);
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
        && [...shape.text].length <= MAX_TEXT_CHARS
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
  const name = `${KIND_NAMES[annotation.shape.kind]} on page ${annotation.pageNumber}`;
  if (annotation.shape.kind !== "text") return name;
  const text = annotation.shape.text.replace(/\s+/g, " ").trim();
  return `${name}: ${text.length > 60 ? `${text.slice(0, 59)}…` : text}`;
}

export type { AnnotationShape, FrameShape, HighlightShape, InkShape, SegmentShape, TextShape };
