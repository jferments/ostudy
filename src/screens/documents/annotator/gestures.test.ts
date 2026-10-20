import { describe, expect, it } from "vitest";

import {
  DEFAULT_STYLES,
  estimateTextHeight,
  type Annotation,
  type AnnotationShape,
} from "./annotationModel";
import {
  AnnotatorGestures,
  type GestureContext,
  type GestureEffect,
  type GestureSurface,
  type PointerKind,
  type PointerSample,
} from "./gestures";

// Two US Letter pages, one above the other, at one CSS pixel per point.
const PAGE = { width: 612, height: 792 };
const GAP = 16;
const pageTop = (pageNumber: number) => (pageNumber - 1) * (PAGE.height + GAP);

const surface: GestureSurface = {
  pageAt(x, y) {
    for (const pageNumber of [1, 2]) {
      const top = pageTop(pageNumber);
      if (x >= 0 && x <= PAGE.width && y >= top && y <= top + PAGE.height) return pageNumber;
    }
    return null;
  },
  nearestPage: (_x, y) => (y < PAGE.height + GAP / 2 ? 1 : 2),
  toPage: (pageNumber, x, y) => ({ x, y: y - pageTop(pageNumber) }),
  pageSize: (pageNumber) => (pageNumber === 1 || pageNumber === 2 ? PAGE : null),
};

function context(overrides: Partial<GestureContext> = {}, annotations: readonly Annotation[] = []): GestureContext {
  let ids = 0;
  let time = 100;
  return {
    tool: "pen",
    styles: DEFAULT_STYLES,
    penOnly: false,
    scale: 1,
    minScale: 0.25,
    maxScale: 4,
    selected: null,
    annotationsOn: (pageNumber) => annotations.filter((item) => item.pageNumber === pageNumber),
    textHeight: (item) => (item.shape.kind === "text" ? estimateTextHeight(item.shape) : 0),
    newId: () => `new${(ids += 1)}`,
    now: () => (time += 1),
    ...overrides,
  };
}

function at(id: number, kind: PointerKind, x: number, y: number, extra: Partial<PointerSample> = {}): PointerSample {
  return { id, kind, clientX: x, clientY: y, pressure: 0.5, button: 0, shiftKey: false, time: 0, ...extra };
}

function only<K extends GestureEffect["kind"]>(effects: readonly GestureEffect[], kind: K) {
  return effects.filter((effect): effect is Extract<GestureEffect, { kind: K }> => effect.kind === kind);
}

function created(effects: readonly GestureEffect[]): AnnotationShape[] {
  return only(effects, "create").map((effect) => effect.annotation.shape);
}

function annotation(id: string, shape: AnnotationShape, pageNumber = 1): Annotation {
  return { id, pageNumber, shape, createdAt: 1 };
}

const box = annotation("box", { kind: "rectangle", x: 100, y: 100, width: 50, height: 50, color: "#dc2626", strokeWidth: 3 });
const line = annotation("line", { kind: "line", x1: 300, y1: 100, x2: 400, y2: 100, color: "#dc2626", strokeWidth: 3 });
const note = annotation("note", { kind: "text", x: 300, y: 300, width: 120, text: "Check this", color: "#1f2328", fontSize: 14 });
const lowerBox = annotation("lower", { ...box.shape }, 2);

describe("drawing", () => {
  it("draws a pen stroke with the pen's pressure and saves it when lifted", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context();
    const down = gestures.down(at(1, "pen", 100, 100, { pressure: 0.3 }), surface, ctx);
    expect(only(down, "pen-seen")).toHaveLength(1);
    expect(only(down, "draft")[0]?.draft).toMatchObject({ pageNumber: 1, complete: false, shape: { points: [100, 100, 0.3] } });

    gestures.move(at(1, "pen", 110, 105, { pressure: 0.8 }), surface, ctx);
    // Too close to the last point to show.
    expect(gestures.move(at(1, "pen", 110.2, 105.2, { pressure: 0.8 }), surface, ctx)).toEqual([]);
    expect(gestures.busy()).toBe(true);

    const up = gestures.up(at(1, "pen", 110, 105), surface, ctx);
    expect(up[0]).toEqual({ kind: "draft", draft: null });
    expect(only(up, "create")[0]?.annotation).toMatchObject({
      id: "new1",
      pageNumber: 1,
      createdAt: 101,
      shape: { kind: "ink", points: [100, 100, 0.3, 110, 105, 0.8], simulatePressure: false, color: "#1f2328", strokeWidth: 3 },
    });
    expect(gestures.busy()).toBe(false);
  });

  it("shapes finger and mouse strokes from their speed, and starts a pen at medium pressure when it reports none", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context();
    gestures.down(at(1, "touch", 100, 100, { pressure: 1 }), surface, ctx);
    gestures.move(at(1, "touch", 120, 100, { pressure: 1 }), surface, ctx);
    expect(created(gestures.up(at(1, "touch", 120, 100), surface, ctx))[0])
      .toMatchObject({ points: [100, 100, 0.5, 120, 100, 0.5], simulatePressure: true });

    gestures.down(at(2, "pen", 100, 200, { pressure: 0 }), surface, ctx);
    gestures.move(at(2, "pen", 120, 200, { pressure: 0.6 }), surface, ctx);
    expect(created(gestures.up(at(2, "pen", 120, 200), surface, ctx))[0])
      .toMatchObject({ points: [100, 200, 0.5, 120, 200, 0.6], simulatePressure: false });
  });

  it("keeps a stroke on the page it started on", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context();
    gestures.down(at(1, "mouse", 600, 780), surface, ctx);
    gestures.move(at(1, "mouse", 700, 900), surface, ctx);
    expect(created(gestures.up(at(1, "mouse", 700, 900), surface, ctx))[0])
      .toMatchObject({ points: [600, 780, 0.5, 612, 792, 0.5] });
  });

  it("stores a straight highlight by its ends and drops a highlighter click", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context({ tool: "highlighter" });
    gestures.down(at(1, "mouse", 100, 100), surface, ctx);
    for (const x of [110, 120, 130]) gestures.move(at(1, "mouse", x, 100), surface, ctx);
    expect(created(gestures.up(at(1, "mouse", 130, 100), surface, ctx))[0])
      .toMatchObject({ kind: "highlight", points: [100, 100, 130, 100], color: "#fde047", strokeWidth: 16 });

    gestures.down(at(2, "mouse", 100, 100), surface, ctx);
    expect(gestures.up(at(2, "mouse", 100, 100), surface, ctx)).toEqual([{ kind: "draft", draft: null }]);
  });

  it("snaps a highlight dragged across the page's text to its lines, and keeps the words", () => {
    const characters = Array.from("heart rate output");
    const boxes = Float64Array.from(characters.flatMap((_, index) => index < 11
      ? [72 + index * 6, 100, 6, 10]
      : [72 + (index - 11) * 6, 114, 6, 10]));
    const gestures = new AnnotatorGestures();
    const ctx = context({ tool: "highlighter", pageText: (pageNumber) => (pageNumber === 1 ? { characters, boxes } : null) });

    // Nothing shows for a press, and a tap on the text marks nothing.
    expect(gestures.down(at(1, "mouse", 105, 105), surface, ctx)).toEqual([]);
    expect(gestures.up(at(1, "mouse", 105, 105), surface, ctx)).toEqual([{ kind: "draft", draft: null }]);

    gestures.down(at(2, "mouse", 111, 105), surface, ctx);
    const moved = gestures.move(at(2, "mouse", 400, 118), surface, ctx);
    expect(only(moved, "draft")[0]?.draft?.shape).toMatchObject({ kind: "highlight", points: [], text: "rate output" });
    expect(created(gestures.up(at(2, "mouse", 400, 118), surface, ctx))[0]).toEqual({
      kind: "highlight",
      points: [],
      rects: [108, 100, 30, 10, 72, 114, 36, 10],
      text: "rate output",
      color: "#fde047",
      strokeWidth: 16,
    });

    // Away from the text, and on a page without text, the highlighter draws freely.
    gestures.down(at(3, "mouse", 300, 400), surface, ctx);
    expect(only(gestures.move(at(3, "mouse", 340, 400), surface, ctx), "draft")[0]?.draft?.shape).toMatchObject({ points: [300, 400, 340, 400] });
    gestures.up(at(3, "mouse", 340, 400), surface, ctx);
  });

  it("draws boxes and arrows by dragging, squares them with Shift and drops slips", () => {
    const gestures = new AnnotatorGestures();
    const boxes = context({ tool: "rectangle" });
    expect(gestures.down(at(1, "mouse", 100, 100), surface, boxes)).toEqual([]);
    expect(only(gestures.move(at(1, "mouse", 150, 180, { shiftKey: true }), surface, boxes), "draft")[0]?.draft?.shape)
      .toMatchObject({ kind: "rectangle", x: 100, y: 100, width: 80, height: 80 });
    expect(created(gestures.up(at(1, "mouse", 150, 180, { shiftKey: true }), surface, boxes))[0])
      .toMatchObject({ kind: "rectangle", width: 80, height: 80, color: "#dc2626", strokeWidth: 3 });

    const arrows = context({ tool: "arrow" });
    gestures.down(at(2, "touch", 100, 100), surface, arrows);
    gestures.move(at(2, "touch", 300, 140), surface, arrows);
    expect(created(gestures.up(at(2, "touch", 300, 140), surface, arrows))[0])
      .toMatchObject({ kind: "arrow", x1: 100, y1: 100, x2: 300, y2: 140 });

    gestures.down(at(3, "mouse", 100, 100), surface, arrows);
    expect(gestures.move(at(3, "mouse", 101, 100), surface, arrows)).toEqual([{ kind: "draft", draft: null }]);
    expect(created(gestures.up(at(3, "mouse", 101, 100), surface, arrows))).toEqual([]);
  });

  it("erases everything the eraser passes over, when it lifts", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context({ tool: "eraser" }, [box, line, lowerBox]);
    expect(only(gestures.down(at(1, "mouse", 100, 125), surface, ctx), "erasing")[0]?.annotations).toEqual([box]);
    expect(only(gestures.move(at(1, "mouse", 350, 101), surface, ctx), "erasing")[0]?.annotations).toEqual([box, line]);
    expect(gestures.up(at(1, "mouse", 350, 101), surface, ctx)).toEqual([
      { kind: "erasing", annotations: [] },
      { kind: "erase", annotations: [box, line] },
    ]);
  });

  it("erases with a stylus's eraser end whichever tool is chosen", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context({ tool: "pen" }, [box, line]);
    const down = gestures.down(at(1, "pen", 100, 125, { button: 5 }), surface, ctx);
    expect(only(down, "draft")).toEqual([]);
    expect(only(down, "erasing")[0]?.annotations).toEqual([box]);
    expect(gestures.up(at(1, "pen", 100, 125), surface, ctx)).toEqual([
      { kind: "erasing", annotations: [] },
      { kind: "erase", annotations: [box] },
    ]);

    // The writing end draws again.
    gestures.down(at(2, "pen", 300, 300), surface, ctx);
    expect(created(gestures.up(at(2, "pen", 300, 300), surface, ctx))).toMatchObject([{ kind: "ink" }]);
  });
});

describe("fingers, pens and palms", () => {
  it("turns a finger stroke into a pinch when a second finger lands, and draws nothing", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context();
    gestures.down(at(1, "touch", 100, 100), surface, ctx);
    gestures.move(at(1, "touch", 120, 100), surface, ctx);
    expect(gestures.down(at(2, "touch", 300, 100), surface, ctx)).toEqual([{ kind: "draft", draft: null }]);

    expect(gestures.move(at(2, "touch", 500, 100), surface, ctx)).toEqual([{
      kind: "zoom",
      scale: 380 / 180,
      anchor: { pageNumber: 1, point: { x: 210, y: 100 } },
      clientX: 310,
      clientY: 100,
    }]);
    // Fingers far apart stop at the largest zoom.
    expect(only(gestures.move(at(2, "touch", 2_000, 100), surface, ctx), "zoom")[0]?.scale).toBe(4);

    // The finger left behind neither draws nor scrolls.
    expect(gestures.up(at(1, "touch", 120, 100), surface, ctx)).toEqual([]);
    expect(gestures.move(at(2, "touch", 400, 300), surface, ctx)).toEqual([]);
    expect(gestures.up(at(2, "touch", 400, 300), surface, ctx)).toEqual([]);
    expect(gestures.busy()).toBe(false);
  });

  it("lets a pen take over from a resting palm, and ignores fingers while the pen draws", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context();
    gestures.down(at(1, "touch", 200, 400), surface, ctx);
    const pen = gestures.down(at(2, "pen", 100, 100, { pressure: 0.4 }), surface, ctx);
    expect(pen.map((effect) => effect.kind)).toEqual(["pen-seen", "draft", "draft"]);
    expect(only(pen, "draft")[0]?.draft).toBeNull();

    expect(gestures.move(at(1, "touch", 260, 400), surface, ctx)).toEqual([]);
    expect(gestures.down(at(3, "touch", 50, 50), surface, ctx)).toEqual([]);
    gestures.move(at(2, "pen", 140, 100, { pressure: 0.6 }), surface, ctx);
    expect(gestures.up(at(3, "touch", 50, 50), surface, ctx)).toEqual([]);
    expect(created(gestures.up(at(2, "pen", 140, 100), surface, ctx))[0])
      .toMatchObject({ kind: "ink", points: [100, 100, 0.4, 140, 100, 0.6] });
    expect(gestures.up(at(1, "touch", 260, 400), surface, ctx)).toEqual([]);
  });

  it("scrolls and flings with a finger when only the pen draws", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context({ penOnly: true });
    expect(gestures.down(at(1, "touch", 100, 100, { time: 0 }), surface, ctx)).toEqual([]);
    expect(gestures.move(at(1, "touch", 100, 80, { time: 16 }), surface, ctx)).toEqual([{ kind: "scroll-by", dx: 0, dy: 20 }]);
    gestures.move(at(1, "touch", 100, 40, { time: 32 }), surface, ctx);
    expect(gestures.up(at(1, "touch", 100, 40, { time: 40 }), surface, ctx)).toEqual([
      { kind: "fling", velocityX: 0, velocityY: 60 / 32 },
    ]);

    // A finger still places text notes.
    const text = context({ penOnly: true, tool: "text" });
    gestures.down(at(2, "touch", 50, 60), surface, text);
    expect(gestures.up(at(2, "touch", 52, 61), surface, text)).toEqual([
      { kind: "new-text", pageNumber: 1, point: { x: 50, y: 60 }, sticky: false },
    ]);
  });

  it("scrolls with a finger that starts beside the pages, and pans with the middle mouse button", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context();
    gestures.down(at(1, "touch", 700, 100), surface, ctx);
    expect(gestures.move(at(1, "touch", 690, 130), surface, ctx)).toEqual([{ kind: "scroll-by", dx: 10, dy: -30 }]);
    gestures.up(at(1, "touch", 690, 130), surface, ctx);

    gestures.down(at(2, "mouse", 100, 100, { button: 1 }), surface, ctx);
    expect(gestures.move(at(2, "mouse", 90, 90), surface, ctx)).toEqual([{ kind: "scroll-by", dx: 10, dy: 10 }]);
    gestures.up(at(2, "mouse", 90, 90), surface, ctx);

    gestures.down(at(3, "mouse", 100, 100, { button: 2 }), surface, ctx);
    expect(gestures.move(at(3, "mouse", 150, 150), surface, ctx)).toEqual([]);
    expect(gestures.up(at(3, "mouse", 150, 150), surface, ctx)).toEqual([]);
  });

  it("commits nothing when the browser cancels the pointer or the tool changes", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context();
    gestures.down(at(1, "pen", 100, 100), surface, ctx);
    gestures.move(at(1, "pen", 150, 100), surface, ctx);
    expect(gestures.cancel(7)).toEqual([]);
    expect(gestures.cancel(1)).toEqual([{ kind: "draft", draft: null }]);
    expect(gestures.up(at(1, "pen", 150, 100), surface, ctx)).toEqual([]);

    gestures.down(at(2, "mouse", 100, 100), surface, ctx);
    gestures.move(at(2, "mouse", 150, 100), surface, ctx);
    expect(gestures.reset()).toEqual([{ kind: "draft", draft: null }]);
    expect(gestures.busy()).toBe(false);
    expect(gestures.move(at(2, "mouse", 200, 100), surface, ctx)).toEqual([]);
    expect(gestures.up(at(2, "mouse", 200, 100), surface, ctx)).toEqual([]);
  });
});

describe("selecting and text", () => {
  it("selects with a tap and clears the selection with a tap beside it", () => {
    const gestures = new AnnotatorGestures();
    gestures.down(at(1, "mouse", 101, 125), surface, context({ tool: "select" }, [box]));
    expect(gestures.up(at(1, "mouse", 101, 125), surface, context({ tool: "select" }, [box])))
      .toEqual([{ kind: "select", id: "box" }]);

    const selected = context({ tool: "select", selected: box }, [box]);
    gestures.down(at(2, "touch", 400, 600), surface, selected);
    expect(gestures.up(at(2, "touch", 400, 600), surface, selected)).toEqual([{ kind: "select", id: null }]);
  });

  it("moves a dragged annotation, keeping it on its page", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context({ tool: "select" }, [box]);
    gestures.down(at(1, "mouse", 101, 125), surface, ctx);
    const moving = gestures.move(at(1, "mouse", 131, 145), surface, ctx);
    expect(moving[0]).toEqual({ kind: "select", id: "box" });
    expect(only(moving, "preview")[0]?.annotation?.shape).toMatchObject({ x: 130, y: 120 });
    gestures.move(at(1, "mouse", -500, 145), surface, ctx);
    const up = gestures.up(at(1, "mouse", -500, 145), surface, ctx);
    expect(up[0]).toEqual({ kind: "preview", annotation: null });
    expect(only(up, "change")[0]).toMatchObject({ before: box, after: { id: "box", shape: { x: 0, y: 120 } } });
  });

  it("reshapes the selected box from a corner handle", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context({ tool: "select", selected: box }, [box]);
    gestures.down(at(1, "touch", 152, 148), surface, ctx);
    expect(only(gestures.move(at(1, "touch", 200, 210), surface, ctx), "preview")[0]?.annotation?.shape)
      .toMatchObject({ x: 100, y: 100, width: 100, height: 110 });
    expect(only(gestures.up(at(1, "touch", 200, 210), surface, ctx), "change")).toHaveLength(1);

    // Letting go where it started changes nothing.
    gestures.down(at(2, "mouse", 150, 150), surface, ctx);
    gestures.move(at(2, "mouse", 160, 160), surface, ctx);
    gestures.move(at(2, "mouse", 150, 150), surface, ctx);
    expect(gestures.up(at(2, "mouse", 150, 150), surface, ctx)).toEqual([{ kind: "preview", annotation: null }]);
  });

  it("opens a new note where the text tool taps, or the note under it", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context({ tool: "text" }, [box, note]);
    gestures.down(at(1, "mouse", 50, 60), surface, ctx);
    expect(gestures.up(at(1, "mouse", 50, 60), surface, ctx)).toEqual([
      { kind: "new-text", pageNumber: 1, point: { x: 50, y: 60 }, sticky: false },
    ]);
    gestures.down(at(2, "touch", 320, 305), surface, ctx);
    expect(gestures.up(at(2, "touch", 320, 305), surface, ctx)).toEqual([{ kind: "edit-text", annotation: note }]);
    // Text taps pass through other annotations to the page.
    gestures.down(at(3, "mouse", 101, 125), surface, ctx);
    expect(gestures.up(at(3, "mouse", 101, 125), surface, ctx)).toEqual([
      { kind: "new-text", pageNumber: 1, point: { x: 101, y: 125 }, sticky: false },
    ]);
    gestures.down(at(4, "mouse", 650, 60), surface, ctx);
    expect(gestures.up(at(4, "mouse", 650, 60), surface, ctx)).toEqual([]);
  });

  it("places a sticky note where the sticky tool taps, or opens the sticky note under it", () => {
    const gestures = new AnnotatorGestures();
    const sticky = annotation("sticky", {
      kind: "text", x: 40, y: 40, width: 120, text: "Ask about this", color: "#fde047", fontSize: 10, sticky: true,
    });
    const ctx = context({ tool: "sticky", penOnly: true }, [sticky, note]);
    gestures.down(at(1, "touch", 80, 60), surface, ctx);
    expect(gestures.up(at(1, "touch", 80, 60), surface, ctx)).toEqual([{ kind: "edit-text", annotation: sticky }]);
    // Plain text notes are not sticky notes, so a tap on one places a new sticky note.
    gestures.down(at(2, "mouse", 320, 305), surface, ctx);
    expect(gestures.up(at(2, "mouse", 320, 305), surface, ctx)).toEqual([
      { kind: "new-text", pageNumber: 1, point: { x: 320, y: 305 }, sticky: true },
    ]);
  });

  it("opens the selected note with another tap", () => {
    const gestures = new AnnotatorGestures();
    const ctx = context({ tool: "select", selected: note }, [note]);
    gestures.down(at(1, "touch", 320, 305), surface, ctx);
    expect(gestures.up(at(1, "touch", 320, 305), surface, ctx)).toEqual([{ kind: "edit-text", annotation: note }]);
  });
});
