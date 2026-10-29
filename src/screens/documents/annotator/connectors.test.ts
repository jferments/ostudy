import { describe, expect, it } from "vitest";

import type { SegmentShape } from "../../../ipc/documentAnnotations";
import { applyEdits, groupByPage, type AnnotationEdit, type PageAnnotations } from "./annotationHistory";
import { estimateTextHeight, type Annotation } from "./annotationModel";
import { connectedPreview, connectMarks, snappedDraft } from "./connectors";
import { retopic, visibleMarks, type Topic } from "./mindMap";

const PAGE = { width: 1000, height: 1000 };
const height = (annotation: Annotation) => (annotation.shape.kind === "text" ? estimateTextHeight(annotation.shape) : 0);
const pageSize = () => PAGE;

let clock = 0;
function mark(id: string, shape: Annotation["shape"]): Annotation {
  clock += 1;
  return { id, pageNumber: 1, createdAt: clock, shape };
}
const box = (id: string, x: number, y: number) =>
  mark(id, { kind: "rectangle", x, y, width: 100, height: 60, color: "#1f2328", strokeWidth: 2 });
const arrow = (id: string, x1: number, y1: number, x2: number, y2: number, more: Partial<SegmentShape> = {}) =>
  mark(id, { kind: "arrow", x1, y1, x2, y2, color: "#1f2328", strokeWidth: 2, ...more });

function settle(edits: readonly AnnotationEdit[], pages: PageAnnotations): PageAnnotations {
  return applyEdits(pages, connectMarks(edits, pages, height, pageSize));
}
function add(annotations: readonly Annotation[], pages: PageAnnotations = new Map()): PageAnnotations {
  return settle(annotations.map((annotation) => ({ id: annotation.id, before: null, after: annotation })), pages);
}
function segment(pages: PageAnnotations, id: string): SegmentShape {
  const found = pages.get(1)?.find((annotation) => annotation.id === id)?.shape;
  if (found?.kind !== "arrow" && found?.kind !== "line") throw new Error(`no line ${id}`);
  return found;
}
function moveBy(pages: PageAnnotations, id: string, dx: number, dy: number): AnnotationEdit {
  const before = pages.get(1)?.find((annotation) => annotation.id === id);
  if (!before || before.shape.kind !== "rectangle") throw new Error(`no box ${id}`);
  return { id, before, after: { ...before, shape: { ...before.shape, x: before.shape.x + dx, y: before.shape.y + dy } } };
}

describe("lines and arrows that stay attached", () => {
  it("attaches an arrow drawn from one box to another, its ends just outside their edges", () => {
    const pages = add([box("a", 100, 100), box("b", 500, 100)]);
    const after = add([arrow("link", 150, 130, 550, 130)], pages);
    const shape = segment(after, "link");
    expect(shape.from).toBe("a");
    expect(shape.to).toBe("b");
    expect(shape.x1).toBe(204);
    expect(shape.x2).toBe(496);
    expect(shape.y1).toBe(130);
  });

  it("follows a box that moves, in the same step", () => {
    const pages = add([arrow("link", 150, 130, 550, 130)], add([box("a", 100, 100), box("b", 500, 100)]));
    const edits = connectMarks([moveBy(pages, "b", 0, 300)], pages, height, pageSize);
    expect(edits.map((edit) => edit.id)).toEqual(["b", "link"]);
    const shape = segment(applyEdits(pages, edits), "link");
    expect(shape.to).toBe("b");
    // Aimed from box a's centre to box b's centre, both ends on the edges.
    expect(shape.y2).toBeGreaterThan(300);
    expect(shape.y2).toBeLessThan(400);
    expect(shape.y1).toBeGreaterThan(130);
  });

  it("lets go of a mark that is deleted, leaving the end where it was", () => {
    const pages = add([arrow("link", 150, 130, 550, 130)], add([box("a", 100, 100), box("b", 500, 100)]));
    const b = pages.get(1)?.find((annotation) => annotation.id === "b") ?? null;
    const after = settle([{ id: "b", before: b, after: null }], pages);
    const shape = segment(after, "link");
    expect(shape.to).toBeUndefined();
    expect(shape.from).toBe("a");
    expect(shape.x2).toBe(496);
  });

  it("leaves a line drawn on empty paper, or inside one box, unattached", () => {
    const pages = add([box("a", 100, 100)]);
    expect(segment(add([arrow("free", 300, 300, 400, 400)], pages), "free")).not.toHaveProperty("from");
    const inside = segment(add([arrow("inside", 110, 110, 190, 150)], pages), "inside");
    expect(inside).not.toHaveProperty("from");
    expect(inside).not.toHaveProperty("to");
  });

  it("attaches to the smallest mark under an end, so a note inside a box wins", () => {
    const pages = add([
      mark("frame", { kind: "rectangle", x: 0, y: 0, width: 400, height: 400, color: "#1f2328", strokeWidth: 2 }),
      mark("note", { kind: "text", x: 50, y: 50, width: 100, text: "Inside", color: "#1f2328", fontSize: 16 }),
    ]);
    expect(segment(add([arrow("link", 80, 60, 700, 700)], pages), "link").from).toBe("note");
  });

  it("lets go of an end dragged off its mark, and keeps the other", () => {
    const pages = add([arrow("link", 150, 130, 550, 130)], add([box("a", 100, 100), box("b", 500, 100)]));
    const before = pages.get(1)?.find((annotation) => annotation.id === "link");
    if (!before || before.shape.kind !== "arrow") throw new Error("no arrow");
    const dragged = { ...before, shape: { ...before.shape, x2: 800, y2: 800 } };
    const shape = segment(settle([{ id: "link", before, after: dragged }], pages), "link");
    expect(shape.from).toBe("a");
    expect(shape.to).toBeUndefined();
    expect([shape.x2, shape.y2]).toEqual([800, 800]);
  });

  it("keeps hold when moved together with a mark it is attached to", () => {
    const pages = add([arrow("link", 150, 130, 550, 130)], add([box("a", 100, 100), box("b", 500, 100)]));
    const link = pages.get(1)?.find((annotation) => annotation.id === "link");
    if (!link || link.shape.kind !== "arrow") throw new Error("no arrow");
    const moved = { ...link, shape: { ...link.shape, x1: link.shape.x1 + 20, x2: link.shape.x2 + 20 } };
    const shape = segment(settle([moveBy(pages, "a", 20, 0), { id: "link", before: link, after: moved }], pages), "link");
    expect(shape.from).toBe("a");
    expect(shape.to).toBe("b");
    expect(shape.x2).toBe(496);
  });

  it("moves a curve's control points with their ends", () => {
    const pages = add([arrow("link", 150, 130, 550, 130, { curve: [250, 50, 450, 50] })], add([box("a", 100, 100), box("b", 500, 100)]));
    const before = segment(pages, "link");
    const shape = segment(settle([moveBy(pages, "b", 100, 0)], pages), "link");
    expect(shape.curve?.[2]).toBe((before.curve?.[2] ?? 0) + (shape.x2 - before.x2));
    expect(shape.curve?.[0]).toBe(before.curve?.[0]);
  });

  it("hides a line attached to a folded-away topic", () => {
    const topic = (id: string, parent?: string): Topic => ({
      ...mark(id, { kind: "text", x: 0, y: 0, width: 100, text: id, color: "#2563eb", fontSize: 16 }),
      shape: { kind: "text", x: 0, y: 0, width: 100, text: id, color: "#2563eb", fontSize: 16, topic: { map: "root", order: 0, ...(parent ? { parent } : {}) } },
    } as Topic);
    const root = retopic(topic("root"), {});
    const page = [root, topic("a", "root"), topic("a1", "a"), arrow("link", 0, 0, 10, 10, { from: "a1" })];
    expect(visibleMarks(page).map((annotation) => annotation.id)).toContain("link");
    const folded = page.map((annotation) => (annotation.id === "a" ? retopic(annotation as Topic, { folded: true }) : annotation));
    expect(visibleMarks(folded).map((annotation) => annotation.id)).not.toContain("link");
  });

  it("shows a line being drawn attached, and lines following a mark being dragged", () => {
    const pages = add([arrow("link", 150, 130, 550, 130)], add([box("a", 100, 100), box("b", 500, 100)]));
    const page = pages.get(1) ?? [];
    const draft = snappedDraft(page, { kind: "line", x1: 150, y1: 300, x2: 550, y2: 130, color: "#1f2328", strokeWidth: 2 }, 1, height, PAGE);
    expect(draft.to).toBe("b");
    const shown = connectedPreview(page, [moveBy(pages, "a", 0, 200)], height, PAGE);
    const link = shown.find((annotation) => annotation.id === "link");
    expect(link?.shape.kind === "arrow" && link.shape.y1).toBeGreaterThan(130);
  });

  it("passes edits on pages without lines through as they are", () => {
    const pages = groupByPage([box("a", 0, 0)]);
    const edits = [moveBy(pages, "a", 10, 10)];
    expect(connectMarks(edits, pages, height, pageSize)).toEqual(edits);
  });
});
