import { describe, expect, it } from "vitest";

import {
  cleanNoteText,
  describeAnnotation,
  distanceToShape,
  estimateTextHeight,
  isDeliberateShape,
  isStorable,
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

  it("estimates a note's height from its lines before it is laid out", () => {
    expect(estimateTextHeight({ ...note, text: "a\nb", fontSize: 10 })).toBe(30);
  });
});
