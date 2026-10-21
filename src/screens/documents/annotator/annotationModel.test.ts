import { describe, expect, it } from "vitest";

import type { InkShape } from "../../../ipc/documentAnnotations";

import {
  annotationsInLoop,
  frameAround,
  isCollapsedSticky,
  rectangleLoop,
  shapeBounds,
  cleanNoteText,
  insideLoop,
  moveGroupWithinPage,
  clampSize,
  cutStroke,
  dashArray,
  isCuttable,
  describeAnnotation,
  distanceToShape,
  estimateTextHeight,
  isDeliberateShape,
  handlesFor,
  isStorable,
  MAX_STICKY_CHARS,
  MAX_STROKE_POINTS,
  MAX_TEXT_CHARS,
  MIN_TEXT_WIDTH,
  moveWithinPage,
  resizeShape,
  restyle,
  shapeFromDrag,
  simplifyPairs,
  snapAngle,
  topmostAt,
  type Annotation,
  type AnnotationShape,
  type FrameShape,
  type TextShape,
} from "./annotationModel";

const page = { width: 612, height: 792 };
const red = "#dc2626";

function annotation(id: string, shape: AnnotationShape, createdAt: number): Annotation {
  return { id, pageNumber: 1, shape, createdAt };
}

function textHeight(item: Annotation): number {
  return item.shape.kind === "text" ? estimateTextHeight(item.shape) : 0;
}

const box: FrameShape = { kind: "rectangle", x: 100, y: 100, width: 200, height: 100, color: red, strokeWidth: 3 };
const note: TextShape = { kind: "text", x: 400, y: 400, width: 100, text: "Hello", color: "#1f2328", fontSize: 14 };

describe("annotation hit testing", () => {
  const annotations = [
    annotation("note", note, 1),
    annotation("box", box, 2),
    annotation("ink", { kind: "ink", points: [100, 300, 0.5, 200, 300, 0.5], color: red, strokeWidth: 3, simulatePressure: true }, 3),
    annotation("line", { kind: "line", x1: 380, y1: 410, x2: 520, y2: 410, color: red, strokeWidth: 2 }, 4),
  ];

  it("finds text notes first, because they are drawn above strokes and shapes", () => {
    expect(topmostAt(annotations, { x: 450, y: 410 }, 2, textHeight)?.id).toBe("note");
    expect(topmostAt(annotations, { x: 450, y: 410 }, 2, textHeight, (item) => item.shape.kind !== "text")?.id)
      .toBe("line");
  });

  it("hits a box on its outline, not inside it", () => {
    expect(topmostAt(annotations, { x: 101, y: 150 }, 2, textHeight)?.id).toBe("box");
    expect(topmostAt(annotations, { x: 150, y: 150 }, 2, textHeight)).toBeNull();
    expect(topmostAt(annotations, { x: 150, y: 302 }, 2, textHeight)?.id).toBe("ink");
  });

  it("measures how far a point is from what a shape paints", () => {
    const line: AnnotationShape = { kind: "line", x1: 0, y1: 0, x2: 100, y2: 0, color: red, strokeWidth: 2 };
    expect(distanceToShape(line, { x: 50, y: 10 }, 0)).toBe(9);
    expect(distanceToShape(line, { x: 50, y: 0.5 }, 0)).toBe(0);
    const ellipse: AnnotationShape = { kind: "ellipse", x: 0, y: 0, width: 200, height: 100, color: red, strokeWidth: 2 };
    expect(distanceToShape(ellipse, { x: 200, y: 50 }, 0)).toBe(0);
    expect(distanceToShape(ellipse, { x: 100, y: 50 }, 0)).toBe(49);
  });
});

describe("lasso groups", () => {
  const square = [0, 0, 100, 0, 100, 100, 0, 100];
  const at = (id: string, x: number, y: number) => ({
    id, pageNumber: 1, createdAt: 1,
    shape: { kind: "rectangle" as const, x, y, width: 20, height: 20, color: "#dc2626", strokeWidth: 2 },
  });

  it("takes annotations whose middle the loop surrounds", () => {
    expect(insideLoop({ x: 50, y: 50 }, square)).toBe(true);
    expect(insideLoop({ x: 150, y: 50 }, square)).toBe(false);
    const inside = at("in", 10, 10);
    const halfway = at("half", 85, 40);
    const outside = at("out", 95, 40);
    expect(annotationsInLoop([inside, halfway, outside], square, () => 0).map((annotation) => annotation.id))
      .toEqual(["in", "half"]);
    expect(annotationsInLoop([inside], [0, 0, 100, 100], () => 0)).toEqual([]);
  });

  it("moves a group only as far as all of it stays on the page", () => {
    const moved = moveGroupWithinPage([at("a", 10, 10), at("b", 80, 50)], 50, -30, { width: 120, height: 200 }, () => 0);
    expect(moved.map((annotation) => annotation.shape.kind === "rectangle" ? [annotation.shape.x, annotation.shape.y] : null))
      .toEqual([[30, 0], [100, 40]]);
  });
});

describe("erasing part of a stroke", () => {
  const ink: InkShape = { kind: "ink", points: [0, 0, 0.2, 100, 0, 0.6], color: "#1f2328", strokeWidth: 2, simulatePressure: false };

  it("cuts a stroke where the eraser crosses it, even between two far-apart points", () => {
    expect(cutStroke(ink, { x: 50, y: 0 }, 9)).toEqual([
      { ...ink, points: [0, 0, 0.2, 40, 0, 0.36] },
      { ...ink, points: [60, 0, 0.44, 100, 0, 0.6] },
    ]);
  });

  it("leaves a stroke the eraser misses, and trims one it reaches the end of", () => {
    expect(cutStroke(ink, { x: 50, y: 30 }, 9)).toBeNull();
    // The eraser's edge reaches the stroke's edge: its half width counts.
    expect(cutStroke(ink, { x: 50, y: 9.5 }, 9)).not.toBeNull();
    expect(cutStroke(ink, { x: 100, y: 0 }, 9)).toEqual([{ ...ink, points: [0, 0, 0.2, 90, 0, 0.56] }]);
    expect(cutStroke({ ...ink, points: [5, 5, 0.5] }, { x: 5, y: 5 }, 9)).toEqual([]);
  });

  it("cuts pen and free highlighter strokes, and leaves highlights of the page's text whole", () => {
    expect(isCuttable(ink)).toBe(true);
    expect(isCuttable({ kind: "highlight", points: [0, 0, 10, 0], color: "#fde047", strokeWidth: 16 })).toBe(true);
    expect(isCuttable({ kind: "highlight", points: [], color: "#fde047", strokeWidth: 14, rects: [0, 0, 10, 10], text: "SA node" })).toBe(false);
    expect(isCuttable({ kind: "line", x1: 0, y1: 0, x2: 10, y2: 0, color: "#dc2626", strokeWidth: 3 })).toBe(false);
  });
});

describe("moving and reshaping", () => {
  it("moves a shape as far as it can go without leaving the page", () => {
    const nearCorner: FrameShape = { ...box, x: 500, y: 700, width: 100, height: 80 };
    expect(moveWithinPage(nearCorner, 50, 50, page, 0)).toMatchObject({ x: 512, y: 712 });
    expect(moveWithinPage(nearCorner, -1000, -1000, page, 0)).toMatchObject({ x: 0, y: 0 });
  });

  it("moves stroke points and keeps their pressure", () => {
    const ink: AnnotationShape = { kind: "ink", points: [10, 10, 0.3, 20, 20, 0.9], color: red, strokeWidth: 3, simulatePressure: false };
    expect(moveWithinPage(ink, 5, 7.5, page, 0)).toMatchObject({ points: [15, 17.5, 0.3, 25, 27.5, 0.9] });
  });

  it("draws shapes from a drag, turning lines to 45 degrees and boxes to squares with Shift", () => {
    const tip = snapAngle({ x: 0, y: 0 }, { x: 100, y: 10 });
    expect(tip.y).toBeCloseTo(0);
    expect(tip.x).toBeCloseTo(Math.hypot(100, 10));
    const style = { color: red, size: 3 };
    expect(shapeFromDrag("arrow", { x: 100, y: 100 }, { x: 200, y: 195 }, true, style, page))
      .toMatchObject({ kind: "arrow", x1: 100, y1: 100, x2: 197.53, y2: 197.53, strokeWidth: 3 });
    expect(shapeFromDrag("rectangle", { x: 100, y: 100 }, { x: 50, y: 160 }, true, style, page))
      .toMatchObject({ kind: "rectangle", x: 40, y: 100, width: 60, height: 60 });
    expect(shapeFromDrag("ellipse", { x: 600, y: 780 }, { x: 700, y: 900 }, false, style, page))
      .toMatchObject({ x: 600, y: 780, width: 12, height: 12 });
  });

  it("keeps only shapes big enough to be meant", () => {
    const style = { color: red, size: 3 };
    expect(isDeliberateShape(shapeFromDrag("line", { x: 10, y: 10 }, { x: 11, y: 11 }, false, style, page))).toBe(false);
    expect(isDeliberateShape(shapeFromDrag("arrow", { x: 10, y: 10 }, { x: 14, y: 10 }, false, style, page))).toBe(true);
    // A box with no width would not be stored, so it is not kept either.
    expect(isDeliberateShape(shapeFromDrag("rectangle", { x: 10, y: 10 }, { x: 10, y: 80 }, false, style, page))).toBe(false);
    expect(isDeliberateShape(shapeFromDrag("rectangle", { x: 10, y: 10 }, { x: 10.5, y: 80 }, false, style, page))).toBe(true);
  });

  it("reshapes boxes from any corner, even past the opposite one", () => {
    expect(resizeShape(box, "se", { x: 350, y: 260 }, false, page)).toMatchObject({ x: 100, y: 100, width: 250, height: 160 });
    expect(resizeShape(box, "nw", { x: 400, y: 300 }, false, page)).toMatchObject({ x: 300, y: 200, width: 100, height: 100 });
    // Dragging onto the opposite corner would leave no box, so nothing changes.
    expect(resizeShape(box, "nw", { x: 300, y: 200 }, false, page)).toBe(box);
  });

  it("moves either end of a line and widens text notes no narrower than a word", () => {
    const line: AnnotationShape = { kind: "line", x1: 10, y1: 10, x2: 100, y2: 10, color: red, strokeWidth: 2 };
    expect(resizeShape(line, "start", { x: 50, y: 60 }, false, page)).toMatchObject({ x1: 50, y1: 60, x2: 100, y2: 10 });
    expect(resizeShape(line, "end", { x: 10, y: 10 }, false, page)).toBe(line);
    expect(resizeShape(note, "width", { x: 405, y: 0 }, false, page)).toMatchObject({ width: MIN_TEXT_WIDTH });
    expect(resizeShape(note, "width", { x: 900, y: 0 }, false, page)).toMatchObject({ width: 212 });
  });

  it("restyles text by font size and everything else by line width", () => {
    expect(restyle(note, { size: 20, color: red })).toMatchObject({ fontSize: 20, color: red });
    expect(restyle(box, { size: 6 })).toMatchObject({ strokeWidth: 6, color: red });
  });
});

describe("stored shapes", () => {
  it("drops highlight points that do not change its path, keeping both ends and corners", () => {
    const straight = Array.from({ length: 50 }, (_, index) => [index * 2, 10]).flat();
    expect(simplifyPairs(straight, 0.4)).toEqual([0, 10, 98, 10]);
    const vee = [0, 0, 5, 5, 10, 10, 15, 5, 20, 0];
    expect(simplifyPairs(vee, 0.4)).toEqual([0, 0, 10, 10, 20, 0]);
    expect(simplifyPairs([1, 2, 3], 0.4)).toEqual([1, 2]);
  });

  it("accepts only shapes the native owner stores", () => {
    const ink = (points: number[]): AnnotationShape => ({ kind: "ink", points, color: red, strokeWidth: 3, simulatePressure: false });
    expect(isStorable(ink([1, 2, 0.5]))).toBe(true);
    expect(isStorable(ink([1, 2, 1.2]))).toBe(false);
    expect(isStorable(ink([1, 2]))).toBe(false);
    expect(isStorable(ink([-1, 2, 0.5]))).toBe(false);
    expect(isStorable(ink(Array.from({ length: (MAX_STROKE_POINTS + 1) * 3 }, () => 0.5)))).toBe(false);
    expect(isStorable({ ...box, color: "#DC2626" })).toBe(false);
    expect(isStorable({ ...box, strokeWidth: 0.2 })).toBe(false);
    expect(isStorable({ ...box, strokeWidth: 64 })).toBe(true);
    expect(isStorable({ ...box, x: 14_350 })).toBe(false);
    expect(isStorable({ kind: "arrow", x1: 5, y1: 5, x2: 5, y2: 5, color: red, strokeWidth: 2 })).toBe(false);
    expect(isStorable({ ...note, text: " \n " })).toBe(false);
    expect(isStorable({ ...note, text: "one\ttwo\nthree" })).toBe(true);
    expect(isStorable({ ...note, text: "bell\u0007" })).toBe(false);
    expect(isStorable({ ...note, text: "x".repeat(MAX_TEXT_CHARS + 1) })).toBe(false);
    expect(isStorable({ ...note, fontSize: 5 })).toBe(false);
  });

  it("hits, moves and checks a text highlight by its boxes", () => {
    const marked: AnnotationShape = {
      kind: "highlight", points: [], rects: [100, 100, 60, 10, 72, 114, 30, 10], text: "heart rate", color: "#fde047", strokeWidth: 16,
    };
    expect(distanceToShape(marked, { x: 80, y: 120 }, 0)).toBe(0);
    expect(distanceToShape(marked, { x: 130, y: 120 }, 0)).toBeCloseTo(10);
    expect(moveWithinPage(marked, 10, -200, page, 0)).toMatchObject({ rects: [110, 0, 60, 10, 82, 14, 30, 10] });

    expect(isStorable(marked)).toBe(true);
    expect(isStorable({ ...marked, points: [1, 2] })).toBe(false);
    expect(isStorable({ ...marked, rects: [100, 100, 0, 10] })).toBe(false);
    expect(isStorable({ ...marked, rects: [100, 100, 60] })).toBe(false);
    expect(isStorable({ ...marked, text: " " })).toBe(false);
    expect(isStorable({ ...marked, text: "a\nb" })).toBe(false);
    expect(isStorable({ ...marked, text: undefined })).toBe(false);
    expect(describeAnnotation(annotation("m", marked, 1))).toBe("Highlight on page 1: heart rate");
  });

  it("cleans typed text and names annotations for assistive technology", () => {
    expect(cleanNoteText("a\r\nb\u0007c\td")).toBe("a\nbc\td");
    expect(describeAnnotation(annotation("box", box, 1))).toBe("Box on page 1");
    const long = annotation("note", { ...note, text: `Remember   ${"the heart ".repeat(10)}` }, 1);
    const description = describeAnnotation(long);
    expect(description.startsWith("Text note on page 1: Remember the heart")).toBe(true);
    expect(description.endsWith("…")).toBe(true);
  });

  it("keeps sticky notes a fixed size and lets them hold long text", () => {
    const sticky: TextShape = { ...note, width: 120, fontSize: 10, color: "#fde047", sticky: true, text: "x".repeat(5_000) };
    expect(isStorable(sticky)).toBe(true);
    expect(isStorable({ ...sticky, text: "x".repeat(MAX_STICKY_CHARS + 1) })).toBe(false);
    expect(isStorable({ ...sticky, sticky: false } as unknown as TextShape)).toBe(false);
    expect(estimateTextHeight(sticky)).toBe(72);
    expect(handlesFor(sticky, 72)).toEqual([]);
    expect(describeAnnotation(annotation("sticky", { ...sticky, text: "Ask in office hours" }, 1)))
      .toBe("Sticky note on page 1: Ask in office hours");
  });

  it("estimates a note's height from its lines before it is laid out", () => {
    expect(estimateTextHeight({ ...note, text: "a\nb", fontSize: 10 })).toBe(30);
  });
});

describe("picking up and folding marks", () => {
  const frame = (kind: "rectangle" | "ellipse", id: string) => ({
    id, pageNumber: 1, createdAt: 1,
    shape: { kind, x: 0, y: 0, width: 100, height: 60, color: "#dc2626", strokeWidth: 2 },
  });

  it("finds the topmost box or ellipse around a point, but not outside an ellipse's curve", () => {
    const box = frame("rectangle", "box");
    const ring = frame("ellipse", "ring");
    expect(frameAround([box, ring], { x: 50, y: 30 })?.id).toBe("ring");
    expect(frameAround([box, ring], { x: 3, y: 3 })?.id).toBe("box");
    expect(frameAround([ring], { x: 3, y: 3 })).toBeNull();
    expect(frameAround([box], { x: 101, y: 30 })).toBeNull();
  });

  it("makes a rectangle loop from two corners", () => {
    expect(rectangleLoop({ x: 10, y: 20 }, { x: 5, y: 40 })).toEqual([10, 20, 5, 20, 5, 40, 10, 40]);
  });

  it("shows a collapsed sticky note as its small icon, and keeps only sticky notes collapsible", () => {
    const sticky = { kind: "text" as const, x: 10, y: 10, width: 120, text: "Folded", color: "#fde047", fontSize: 10, sticky: true as const };
    const folded = { ...sticky, collapsed: true as const };
    expect(isCollapsedSticky(folded)).toBe(true);
    expect(isCollapsedSticky(sticky)).toBe(false);
    expect(estimateTextHeight(folded)).toBe(18);
    expect(shapeBounds(folded, estimateTextHeight(folded))).toEqual({ left: 10, top: 10, right: 28, bottom: 28 });
    expect(isStorable(folded)).toBe(true);
    const { sticky: _sticky, ...plain } = folded;
    expect(isStorable(plain)).toBe(false);
  });
});

describe("shape looks", () => {
  const box: FrameShape = { kind: "rectangle", x: 10, y: 10, width: 40, height: 20, color: red, strokeWidth: 2 };

  it("adds and drops a fill, and keeps the border of an unfilled shape", () => {
    const filled = restyle(box, { fill: "#fef3c7", border: false });
    expect(filled).toEqual({ ...box, fill: "#fef3c7", border: false });
    expect(isStorable(filled)).toBe(true);
    expect(restyle(filled, { fill: null })).toEqual(box);
    expect(restyle(box, { border: false })).toEqual(box);
    expect(isStorable({ ...box, border: false })).toBe(false);
  });

  it("dashes any shape but puts a second head only on an arrow", () => {
    const line = { kind: "line" as const, x1: 0, y1: 0, x2: 40, y2: 0, color: red, strokeWidth: 2 };
    expect(restyle(line, { line: "dotted", bothEnds: true })).toEqual({ ...line, dash: "dotted" });
    const arrow = restyle({ ...line, kind: "arrow" }, { line: "dashed", bothEnds: true });
    expect(arrow).toMatchObject({ dash: "dashed", bothEnds: true });
    expect(restyle(arrow, { line: "solid", bothEnds: false })).toEqual({ ...line, kind: "arrow" });
    expect(isStorable({ ...line, bothEnds: true })).toBe(false);
    expect(shapeFromDrag("ellipse", { x: 10, y: 10 }, { x: 60, y: 40 }, false,
      { color: red, size: 2, line: "dashed", bothEnds: true, fill: "#ffffff", border: true }, page))
      .toEqual({ kind: "ellipse", x: 10, y: 10, width: 50, height: 30, color: red, strokeWidth: 2, dash: "dashed", fill: "#ffffff" });
  });

  it("matches the PDF export's dash spacing and keeps typed sizes in range", () => {
    expect(dashArray(undefined, 3)).toBeUndefined();
    expect(dashArray("dashed", 3)).toBe("9 9");
    expect(dashArray("dotted", 0.5)).toBe("0 2");
    expect(clampSize("pen", 4.6)).toBe(4.5);
    expect(clampSize("pen", 500)).toBe(64);
    expect(clampSize("text", 2)).toBe(6);
  });

  it("picks up a filled shape anywhere inside it", () => {
    expect(distanceToShape(box, { x: 30, y: 20 }, 0)).toBeGreaterThan(0);
    expect(distanceToShape({ ...box, fill: "#ffffff" }, { x: 30, y: 20 }, 0)).toBe(0);
  });
});
