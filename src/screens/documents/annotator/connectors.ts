import type { SegmentShape } from "../../../ipc/documentAnnotations";
import { applyEdits, type AnnotationEdit, type PageAnnotations } from "./annotationHistory";
import { clampToPage, round, shapeExtent, type Annotation, type Bounds, type Point, type Size } from "./annotationModel";
import { visibleMarks } from "./mindMap";

/**
 * Lines and arrows that stay attached, on a notebook page. An end drawn or
 * dropped on a box, an ellipse, a text or sticky note or a mind-map topic is
 * attached to that mark: it sits just outside the mark's edge, aimed at the
 * other end, and follows the mark wherever it goes. Deleting the mark lets the
 * end go where it was.
 */

export type Connector = Annotation & { shape: SegmentShape };

export function isSegment(annotation: Annotation | null | undefined): annotation is Connector {
  return annotation?.shape.kind === "line" || annotation?.shape.kind === "arrow";
}

/** How far outside a mark an end may land and still attach to it, in points. */
const ATTACH_SLOP = 6;
/** The space between an attached end and its mark's edge, so an arrowhead never hides the border. */
const EDGE_GAP = 4;

type Height = (annotation: Annotation) => number;

/** Marks an end can attach to: boxes, ellipses and notes. Pictures, ink and other lines are pointed at, not attached to. */
function canAttach(annotation: Annotation): boolean {
  const { kind } = annotation.shape;
  return kind === "rectangle" || kind === "ellipse" || kind === "text";
}

function extentOf(mark: Annotation, height: Height): Bounds {
  return shapeExtent(mark.shape, height(mark));
}

function holds(bounds: Bounds, point: Point, slop: number): boolean {
  return point.x >= bounds.left - slop && point.x <= bounds.right + slop
    && point.y >= bounds.top - slop && point.y <= bounds.bottom + slop;
}

function centre(bounds: Bounds): Point {
  return { x: (bounds.left + bounds.right) / 2, y: (bounds.top + bounds.bottom) / 2 };
}

/** The mark an end at `point` attaches to: the smallest one it is on, so a note inside a box wins over the box. */
export function attachableAt(page: readonly Annotation[], point: Point, height: Height): Annotation | null {
  let best: Annotation | null = null;
  let bestArea = Infinity;
  for (const mark of visibleMarks(page)) {
    if (!canAttach(mark)) continue;
    const bounds = extentOf(mark, height);
    if (!holds(bounds, point, ATTACH_SLOP)) continue;
    const area = (bounds.right - bounds.left) * (bounds.bottom - bounds.top);
    if (area < bestArea) {
      best = mark;
      bestArea = area;
    }
  }
  return best;
}

/** Where a line from a mark's centre toward `aim` leaves the mark, a little way out; none when `aim` is on the mark. */
function edgeToward(mark: Annotation, bounds: Bounds, aim: Point): Point | null {
  const middle = centre(bounds);
  const dx = aim.x - middle.x;
  const dy = aim.y - middle.y;
  const halfWidth = (bounds.right - bounds.left) / 2;
  const halfHeight = (bounds.bottom - bounds.top) / 2;
  const length = Math.hypot(dx, dy);
  if (length === 0 || halfWidth <= 0 || halfHeight <= 0) return null;
  // The fraction of the way to `aim` at which the edge lies.
  const edge = mark.shape.kind === "ellipse"
    ? 1 / Math.hypot(dx / halfWidth, dy / halfHeight)
    : Math.min(dx === 0 ? Infinity : halfWidth / Math.abs(dx), dy === 0 ? Infinity : halfHeight / Math.abs(dy));
  const out = edge + EDGE_GAP / length;
  if (!Number.isFinite(out) || out >= 1) return null;
  return { x: middle.x + dx * out, y: middle.y + dy * out };
}

/** The shape with its ends attached to these marks, or let go where none is named. */
function withEnds(shape: SegmentShape, from: string | undefined, to: string | undefined): SegmentShape {
  if (shape.from === from && shape.to === to) return shape;
  const { from: _from, to: _to, ...rest } = shape;
  return { ...rest, ...(from !== undefined ? { from } : {}), ...(to !== undefined ? { to } : {}) };
}

/**
 * The line or arrow with each attached end put on its mark's edge, aimed at
 * the other mark's centre or at the other end. A curve's control points move
 * with their ends. Marks that overlap leave the ends where they are.
 */
export function routeConnector(shape: SegmentShape, from: Annotation | null, to: Annotation | null, height: Height, page: Size): SegmentShape {
  if (!from && !to) return shape;
  const start = { x: shape.x1, y: shape.y1 };
  const end = { x: shape.x2, y: shape.y2 };
  const fromBounds = from ? extentOf(from, height) : null;
  const toBounds = to ? extentOf(to, height) : null;
  const placed = (mark: Annotation | null, bounds: Bounds | null, aim: Point, point: Point): Point => {
    const edge = mark && bounds ? edgeToward(mark, bounds, aim) : null;
    if (!edge) return point;
    const kept = clampToPage(edge, page);
    return { x: round(kept.x), y: round(kept.y) };
  };
  const first = placed(from, fromBounds, toBounds ? centre(toBounds) : end, start);
  const second = placed(to, toBounds, fromBounds ? centre(fromBounds) : start, end);
  if (first.x === second.x && first.y === second.y) return shape;
  if (first.x === start.x && first.y === start.y && second.x === end.x && second.y === end.y) return shape;
  const curve = shape.curve ? (() => {
    const c1 = clampToPage({ x: shape.curve[0] + first.x - start.x, y: shape.curve[1] + first.y - start.y }, page);
    const c2 = clampToPage({ x: shape.curve[2] + second.x - end.x, y: shape.curve[3] + second.y - end.y }, page);
    return [round(c1.x), round(c1.y), round(c2.x), round(c2.y)] as [number, number, number, number];
  })() : undefined;
  return { ...shape, x1: first.x, y1: first.y, x2: second.x, y2: second.y, ...(curve ? { curve } : {}) };
}

function find(page: readonly Annotation[], id: string | undefined): Annotation | null {
  return id === undefined ? null : page.find((annotation) => annotation.id === id) ?? null;
}

/** Which marks a drawn, moved or reshaped line's ends are on. */
function attachedEnds(
  after: Connector,
  before: Connector | null,
  page: readonly Annotation[],
  changed: ReadonlySet<string>,
  height: Height,
): { from: string | undefined; to: string | undefined } {
  const shape = after.shape;
  const previous = before?.shape;
  if (previous) {
    // Moved along with a mark it is attached to, as part of a group: it keeps hold of both ends.
    const dx = shape.x1 - previous.x1;
    const dy = shape.y1 - previous.y1;
    const translated = (dx !== 0 || dy !== 0) && shape.x2 - previous.x2 === dx && shape.y2 - previous.y2 === dy;
    const withMark = [previous.from, previous.to].some((id) => id !== undefined && changed.has(id));
    if (translated && withMark) {
      return { from: find(page, previous.from)?.id, to: find(page, previous.to)?.id };
    }
  }
  const end = (point: Point, was: Point | null, held: string | undefined): string | undefined => {
    const mark = find(page, held);
    // An end that stayed put, or landed back on its mark, keeps hold of it.
    if (mark && canAttach(mark) && ((was && was.x === point.x && was.y === point.y) || holds(extentOf(mark, height), point, ATTACH_SLOP))) {
      return mark.id;
    }
    return attachableAt(page, point, height)?.id;
  };
  const from = end({ x: shape.x1, y: shape.y1 }, previous ? { x: previous.x1, y: previous.y1 } : null, shape.from);
  const to = end({ x: shape.x2, y: shape.y2 }, previous ? { x: previous.x2, y: previous.y2 } : null, shape.to);
  // Both ends on one mark is a line drawn inside it.
  return from === to ? { from: undefined, to: undefined } : { from, to };
}

function sameShape(a: SegmentShape, b: SegmentShape): boolean {
  return a === b || JSON.stringify(a) === JSON.stringify(b);
}

/**
 * Attaches the lines and arrows the edits draw, move or reshape to the marks
 * their ends are on, and brings along every line attached to a mark the edits
 * move, reshape or delete, in the same step. Edits on pages without lines pass
 * through as they are.
 */
export function connectMarks(
  edits: readonly AnnotationEdit[],
  pages: PageAnnotations,
  height: Height,
  pageSize: (pageNumber: number) => Size | null,
): AnnotationEdit[] {
  const touched = new Set<number>();
  for (const edit of edits) {
    if (edit.before) touched.add(edit.before.pageNumber);
    if (edit.after) touched.add(edit.after.pageNumber);
  }
  const hasLines = [...touched].some((pageNumber) => (pages.get(pageNumber) ?? []).some(isSegment))
    || edits.some((edit) => isSegment(edit.after));
  if (!hasLines) return [...edits];

  const result = [...edits];
  const changed = new Set(edits.map((edit) => edit.id));
  let state = applyEdits(pages, edits);

  result.forEach((edit, index) => {
    const after = edit.after;
    if (!isSegment(after)) return;
    const size = pageSize(after.pageNumber);
    if (!size) return;
    const page = state.get(after.pageNumber) ?? [];
    const { from, to } = attachedEnds(after, isSegment(edit.before) ? edit.before : null, page, changed, height);
    const shape = routeConnector(withEnds(after.shape, from, to), find(page, from), find(page, to), height, size);
    if (sameShape(shape, after.shape)) return;
    const routed: Connector = { ...after, shape };
    result[index] = { ...edit, after: routed };
    state = applyEdits(state, [{ id: after.id, before: after, after: routed }]);
  });

  for (const pageNumber of touched) {
    const size = pageSize(pageNumber);
    if (!size) continue;
    const page = state.get(pageNumber) ?? [];
    for (const line of page) {
      if (!isSegment(line) || changed.has(line.id)) continue;
      const { from, to } = line.shape;
      if (!(from !== undefined && changed.has(from)) && !(to !== undefined && changed.has(to))) continue;
      const fromMark = find(page, from);
      const toMark = find(page, to);
      const shape = routeConnector(withEnds(line.shape, fromMark?.id, toMark?.id), fromMark, toMark, height, size);
      if (!sameShape(shape, line.shape)) result.push({ id: line.id, before: line, after: { ...line, shape } });
    }
  }
  return result;
}

/** The middle of a line or curve, where its label is written. */
export function labelPoint(shape: SegmentShape): Point {
  if (!shape.curve) return { x: (shape.x1 + shape.x2) / 2, y: (shape.y1 + shape.y2) / 2 };
  const [c1x, c1y, c2x, c2y] = shape.curve;
  return { x: (shape.x1 + 3 * c1x + 3 * c2x + shape.x2) / 8, y: (shape.y1 + 3 * c1y + 3 * c2y + shape.y2) / 8 };
}

/** The most characters a line's label holds. */
export const MAX_LABEL_CHARS = 120;

/**
 * A curved line linking two marks, from the middle of one to the middle of
 * the other, bowed a little to one side so that it reads as a link rather
 * than as a branch. Its ends are put on the marks' edges when it is added.
 */
export function linkBetween(from: Annotation, to: Annotation, height: Height, color: string, createdAt: number, id: string): Connector {
  const a = centre(extentOf(from, height));
  const b = centre(extentOf(to, height));
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  // A quarter of the way along, pushed out sideways by a fifth of the length.
  const bow = 0.2;
  const c1 = { x: a.x + dx / 4 - dy * bow, y: a.y + dy / 4 + dx * bow };
  const c2 = { x: a.x + dx * 3 / 4 - dy * bow, y: a.y + dy * 3 / 4 + dx * bow };
  return {
    id,
    pageNumber: from.pageNumber,
    createdAt,
    shape: {
      kind: "arrow",
      x1: round(a.x), y1: round(a.y), x2: round(b.x), y2: round(b.y),
      color,
      strokeWidth: 2,
      dash: "dashed",
      curve: [round(c1.x), round(c1.y), round(c2.x), round(c2.y)],
      from: from.id,
      to: to.id,
    },
  };
}

/** Whether a mark can be linked to, as an attached line's end. */
export function isLinkable(annotation: Annotation): boolean {
  return canAttach(annotation);
}

/** A page as it looks mid-gesture: the marks being moved or drawn, and the lines attached to them following. */
export function connectedPreview(
  page: readonly Annotation[],
  edits: readonly AnnotationEdit[],
  height: Height,
  size: Size,
): readonly Annotation[] {
  const pageNumber = edits[0]?.after?.pageNumber ?? edits[0]?.before?.pageNumber;
  if (pageNumber === undefined) return page;
  const settled = connectMarks(edits, new Map([[pageNumber, page]]), height, () => size);
  return applyEdits(new Map([[pageNumber, page]]), settled).get(pageNumber) ?? [];
}

/** A line or arrow being drawn, as it will be attached when it is let go. */
export function snappedDraft(page: readonly Annotation[], shape: SegmentShape, pageNumber: number, height: Height, size: Size): SegmentShape {
  const draft: Connector = { id: "draft", pageNumber, shape, createdAt: Infinity };
  const [settled] = connectMarks([{ id: draft.id, before: null, after: draft }], new Map([[pageNumber, page]]), height, () => size);
  return isSegment(settled?.after) ? settled.after.shape : shape;
}
