import { describe, expect, it } from "vitest";

import type { MapTopic } from "../../../ipc/documentAnnotations";
import { applyEdits, groupByPage, type AnnotationEdit, type PageAnnotations } from "./annotationHistory";
import { estimateTextHeight, type Annotation } from "./annotationModel";
import {
  arrangeMaps,
  branchLines,
  detachEdits,
  foldBadges,
  freeEdits,
  isCentral,
  isTopic,
  lookDepth,
  mapAsList,
  mapFromList,
  mapsOn,
  newCentralTopic,
  newFreeTopic,
  newTopic,
  nextFreeColor,
  parseList,
  promote,
  retopic,
  tidyEdits,
  topicToward,
  visibleMarks,
  type Topic,
} from "./mindMap";

const BOARD = { width: 14_400, height: 14_400 };
const height = (annotation: Annotation) => (annotation.shape.kind === "text" ? estimateTextHeight(annotation.shape) : 0);
const pageSize = () => BOARD;

let clock = 0;
function topic(id: string, text: string, place: Omit<MapTopic, "map"> & { map?: string }, color = "#2563eb"): Topic {
  clock += 1;
  return {
    id,
    pageNumber: 1,
    createdAt: clock,
    shape: { kind: "text", x: 7000, y: 7000, width: 160, text, color, fontSize: 16, topic: { map: "root", ...place } },
  };
}

/** Adds the topics as one step, as the editor would, and returns the page after it. */
function build(topics: readonly Topic[], pages: PageAnnotations = new Map()): PageAnnotations {
  const edits = arrangeMaps(topics.map((item) => ({ id: item.id, before: null, after: item })), pages, height, pageSize);
  return applyEdits(pages, edits);
}

function find(pages: PageAnnotations, id: string): Topic {
  const found = pages.get(1)?.find((annotation) => annotation.id === id);
  if (!isTopic(found)) throw new Error(`no topic ${id}`);
  return found;
}

function overlaps(a: Topic, b: Topic): boolean {
  return a.shape.x < b.shape.x + b.shape.width && b.shape.x < a.shape.x + a.shape.width
    && a.shape.y < b.shape.y + height(b) && b.shape.y < a.shape.y + height(a);
}

const heart = () => [
  topic("root", "Heart", { order: 0 }, "#334155"),
  topic("a", "Systole", { parent: "root", order: 0 }, "#2563eb"),
  topic("b", "Diastole", { parent: "root", order: 1 }, "#dc2626"),
  topic("c", "Valves", { parent: "root", order: 2 }, "#16a34a"),
  topic("a1", "Isovolumetric contraction", { parent: "a", order: 0 }),
  topic("a2", "Ejection", { parent: "a", order: 1 }),
];

describe("mind maps", () => {
  it("builds each map's tree from its topics, branches in order", () => {
    const pages = build(heart());
    const tree = mapsOn(pages.get(1) ?? []).get("root");
    expect(tree?.root?.id).toBe("root");
    expect(tree?.children.get("root")?.map((child) => child.id)).toEqual(["a", "b", "c"]);
    expect(tree?.children.get("a")?.map((child) => child.id)).toEqual(["a1", "a2"]);
  });

  it("lays a map out both ways from its central topic with no two topics overlapping", () => {
    const pages = build(heart());
    const all = (pages.get(1) ?? []).filter(isTopic);
    for (const one of all) {
      for (const other of all) {
        if (one.id !== other.id) expect(overlaps(one, other), `${one.id} and ${other.id}`).toBe(false);
      }
    }
    const root = find(pages, "root");
    // The central topic stays where it was put; branches even out on both sides.
    expect(root.shape.x).toBe(7000);
    expect(find(pages, "a").shape.x).toBeGreaterThan(root.shape.x);
    expect(find(pages, "b").shape.x).toBeGreaterThan(root.shape.x);
    expect(find(pages, "c").shape.x).toBeLessThan(root.shape.x);
    // A branch's own branches go further out on its side.
    expect(find(pages, "a1").shape.x).toBeGreaterThan(find(pages, "a").shape.x);
  });

  it("lays a map out downward or to the right when its central topic says so", () => {
    const down = heart().map((item) => (item.id === "root" ? retopic(item, { layout: "down" }) : item));
    const pages = build(down);
    const root = find(pages, "root");
    for (const id of ["a", "b", "c"]) expect(find(pages, id).shape.y).toBeGreaterThan(root.shape.y);
    expect(find(pages, "a").shape.x).toBeLessThan(find(pages, "b").shape.x);

    const right = build(heart().map((item) => (item.id === "root" ? retopic(item, { layout: "right" }) : item)));
    for (const id of ["a", "b", "c"]) expect(find(right, id).shape.x).toBeGreaterThan(find(right, "root").shape.x);
  });

  it("sizes each topic to its words", () => {
    const pages = build(heart());
    expect(find(pages, "c").shape.width).toBeLessThan(find(pages, "a1").shape.width);
  });

  it("deletes a topic with its branches in one step", () => {
    const pages = build(heart());
    const a = find(pages, "a");
    const edits = arrangeMaps([{ id: "a", before: a, after: null }], pages, height, pageSize);
    const after = applyEdits(pages, edits);
    expect((after.get(1) ?? []).map((item) => item.id).sort()).toEqual(["b", "c", "root"]);
  });

  it("makes a topic dropped on another one of its branches, bringing its own along", () => {
    const pages = build(heart());
    const a = find(pages, "a");
    const c = find(pages, "c");
    const dropped: Topic = { ...a, shape: { ...a.shape, x: c.shape.x + 4, y: c.shape.y } };
    const after = applyEdits(pages, arrangeMaps([{ id: "a", before: a, after: dropped }], pages, height, pageSize));
    expect(find(after, "a").shape.topic.parent).toBe("c");
    expect(find(after, "a1").shape.topic.parent).toBe("a");
    expect(find(after, "a").shape.x).toBeLessThan(find(after, "c").shape.x);
  });

  it("moves a topic dropped on empty paper among its neighbours", () => {
    const pages = build(heart());
    const a2 = find(pages, "a2");
    const a1 = find(pages, "a1");
    const raised: Topic = { ...a2, shape: { ...a2.shape, y: a1.shape.y - 40 } };
    const after = applyEdits(pages, arrangeMaps([{ id: "a2", before: a2, after: raised }], pages, height, pageSize));
    const tree = mapsOn(after.get(1) ?? []).get("root");
    expect(tree?.children.get("a")?.map((child) => child.id)).toEqual(["a2", "a1"]);
  });

  it("keeps a topic where it was let go, with its branches, and moves the others out of its way", () => {
    const pages = build(heart());
    const a = find(pages, "a");
    const b = find(pages, "b");
    // Dragged a little to the right and down onto Diastole's row, missing Diastole itself.
    const dropped: Topic = { ...a, shape: { ...a.shape, x: b.shape.x + b.shape.width + 120, y: b.shape.y + 6 } };
    const after = applyEdits(pages, arrangeMaps([{ id: "a", before: a, after: dropped }], pages, height, pageSize));
    const moved = find(after, "a");
    expect(moved.shape.x).toBeCloseTo(dropped.shape.x, 1);
    expect(moved.shape.y).toBeCloseTo(dropped.shape.y, 1);
    expect(moved.shape.topic.offset).toBeDefined();
    // Its branches come along, further out.
    expect(find(after, "a1").shape.x).toBeGreaterThan(moved.shape.x);
    const all = (after.get(1) ?? []).filter(isTopic);
    for (const one of all) {
      for (const other of all) {
        if (one.id !== other.id) expect(overlaps(one, other), `${one.id} and ${other.id}`).toBe(false);
      }
    }

    // Let go half over its neighbour: the neighbour moves aside, the dropped one stays put.
    const b2 = find(after, "b");
    const a2 = find(after, "a2");
    const over: Topic = { ...b2, shape: { ...b2.shape, x: a2.shape.x + 10, y: a2.shape.y + height(a2) - 6 } };
    const crowded = applyEdits(after, arrangeMaps([{ id: "b", before: b2, after: over }], after, height, pageSize));
    expect(find(crowded, "b").shape.y).toBeCloseTo(over.shape.y, 1);
    expect(find(crowded, "b").shape.topic.parent).toBe("root");
    expect(find(crowded, "a2").shape.y).not.toBeCloseTo(a2.shape.y, 1);
    const marks = (crowded.get(1) ?? []).filter(isTopic);
    for (const one of marks) {
      for (const other of marks) {
        if (one.id !== other.id) expect(overlaps(one, other), `${one.id} and ${other.id}`).toBe(false);
      }
    }

    // Tidying puts it back in the map's own layout.
    const tree = mapsOn(after.get(1) ?? []).get("root");
    if (!tree) throw new Error("no map");
    const tidied = applyEdits(after, arrangeMaps(tidyEdits(tree), after, height, pageSize));
    expect(find(tidied, "a").shape.topic.offset).toBeUndefined();
    expect(find(tidied, "a").shape.x).toBeLessThan(moved.shape.x);
  });

  it("makes room for a topic added after another", () => {
    const pages = build(heart());
    const tree = mapsOn(pages.get(1) ?? []).get("root");
    const a = find(pages, "a");
    const added = tree ? newTopic(tree, { after: a }, 99) : null;
    if (!added) throw new Error("no topic");
    const withText: Topic = { ...added, shape: { ...added.shape, text: "Isovolumetric relaxation" } };
    const after = applyEdits(pages, arrangeMaps([{ id: added.id, before: null, after: withText }], pages, height, pageSize));
    const next = mapsOn(after.get(1) ?? []).get("root");
    expect(next?.children.get("root")?.map((child) => child.id)).toEqual(["a", added.id, "b", "c"]);
  });

  it("gives a branch a topic's new color where it had the old one", () => {
    const pages = build(heart());
    const a = find(pages, "a");
    const recolored: Topic = { ...a, shape: { ...a.shape, color: "#7c3aed" } };
    const after = applyEdits(pages, arrangeMaps([{ id: "a", before: a, after: recolored }], pages, height, pageSize));
    expect(find(after, "a1").shape.color).toBe("#7c3aed");
    expect(find(after, "b").shape.color).toBe("#dc2626");
  });

  it("hides folded branches and counts them on their topic", () => {
    const pages = build(heart());
    const a = find(pages, "a");
    const after = applyEdits(pages, arrangeMaps([{ id: "a", before: a, after: retopic(a, { folded: true }) }], pages, height, pageSize));
    const page = after.get(1) ?? [];
    expect(visibleMarks(page).map((item) => item.id).sort()).toEqual(["a", "b", "c", "root"]);
    expect(foldBadges(page, height)).toEqual([expect.objectContaining({ id: "a", count: 2 })]);
    expect(branchLines(visibleMarks(page), height).map((line) => line.id).sort()).toEqual(["a", "b", "c"]);
    // The page's other marks pass through as they are.
    expect(visibleMarks(page.filter((item) => item.id === "b"))).toHaveLength(1);
  });

  it("moves a topic up a level, after its parent", () => {
    const pages = build(heart());
    const tree = mapsOn(pages.get(1) ?? []).get("root");
    if (!tree) throw new Error("no map");
    const after = applyEdits(pages, arrangeMaps(promote(tree, find(pages, "a1")), pages, height, pageSize));
    const next = mapsOn(after.get(1) ?? []).get("root");
    expect(next?.children.get("root")?.map((child) => child.id)).toEqual(["a", "a1", "b", "c"]);
  });

  it("goes from topic to topic with the arrow keys as the map is drawn", () => {
    const pages = build(heart());
    const tree = mapsOn(pages.get(1) ?? []).get("root");
    if (!tree) throw new Error("no map");
    expect(topicToward(tree, find(pages, "root"), "ArrowRight")?.id).toBe("a");
    expect(topicToward(tree, find(pages, "root"), "ArrowLeft")?.id).toBe("c");
    expect(topicToward(tree, find(pages, "a"), "ArrowDown")?.id).toBe("b");
    expect(topicToward(tree, find(pages, "a"), "ArrowRight")?.id).toBe("a1");
    expect(topicToward(tree, find(pages, "a1"), "ArrowLeft")?.id).toBe("a");
    expect(topicToward(tree, find(pages, "c"), "ArrowRight")?.id).toBe("root");
  });

  it("writes a map as an indented list", () => {
    const pages = build(heart());
    const tree = mapsOn(pages.get(1) ?? []).get("root");
    expect(tree && mapAsList(tree)).toBe([
      "- Heart",
      "  - Systole",
      "    - Isovolumetric contraction",
      "    - Ejection",
      "  - Diastole",
      "  - Valves",
    ].join("\n"));
  });

  it("reads an outline's bullets, numbers, headings and indented lines as levels", () => {
    expect(parseList("## Heart\n### Valves\n1. Mitral\n2) Aortic\n\tSemilunar\n- [ ] Tricuspid\n## Vessels")).toEqual([
      { text: "Heart", depth: 0 },
      { text: "Valves", depth: 1 },
      { text: "Mitral", depth: 2 },
      { text: "Aortic", depth: 2 },
      { text: "Semilunar", depth: 3 },
      { text: "Tricuspid", depth: 2 },
      { text: "Vessels", depth: 0 },
    ]);
    // A line never sits more than one level under the one before it.
    expect(parseList("Heart\n        Ejection").map((item) => item.depth)).toEqual([0, 1]);
  });

  it("makes a map of a pasted outline, its one top line in the middle", () => {
    let time = 0;
    const made = mapFromList(parseList("- Heart\n  - Systole\n    - Ejection\n  - Diastole"), 1, { x: 7000, y: 7000 }, BOARD, () => ++time);
    if (!made) throw new Error("no map");
    expect(made.named).toBe(true);
    const pages = build(made.topics);
    const tree = mapsOn(pages.get(1) ?? []).get(made.topics[0]?.id ?? "");
    expect(tree && mapAsList(tree)).toBe("- Heart\n  - Systole\n    - Ejection\n  - Diastole");

    // Several top lines branch from a new central topic, to be named.
    const several = mapFromList(parseList("Systole\nDiastole"), 1, { x: 7000, y: 7000 }, BOARD, () => ++time);
    expect(several?.named).toBe(false);
    expect(several?.topics.map((topic) => topic.shape.topic.parent === several.topics[0]?.id)).toEqual([false, true, true]);
    expect(several?.topics[1]?.shape.color).not.toBe(several?.topics[2]?.shape.color);
  });

  it("starts a map around the point tapped, kept on the page", () => {
    const central = newCentralTopic(1, { x: 5, y: 5 }, { width: 600, height: 800 }, 1);
    expect(central.shape.topic).toEqual({ map: central.id, order: 0 });
    expect(central.shape.x).toBe(0);
    expect(central.shape.y).toBe(0);
  });

  it("starts a free topic, drawn as a branch rather than a central topic", () => {
    const free = newFreeTopic(1, { x: 7000, y: 7000 }, BOARD, 1, "#dc2626");
    const central = newCentralTopic(1, { x: 7000, y: 7000 }, BOARD, 1);
    expect(free.shape.topic).toEqual({ map: free.id, order: 0, free: true });
    expect(isCentral(free)).toBe(false);
    expect(free.shape.color).toBe("#dc2626");
    expect(free.shape.fontSize).toBeLessThan(central.shape.fontSize ?? 0);
    const tree = mapsOn([free]).get(free.id);
    if (!tree) throw new Error("no map");
    expect(lookDepth(tree, free)).toBe(1);
  });

  it("gives each new free topic a color the others use least", () => {
    const first = newFreeTopic(1, { x: 7000, y: 7000 }, BOARD, 1, nextFreeColor([]));
    expect(nextFreeColor([first])).not.toBe(first.shape.color);
  });

  it("detaches a branch with its own as a free map of their own", () => {
    const pages = build(heart());
    const before = mapsOn(pages.get(1) ?? []).get("root");
    if (!before) throw new Error("no map");
    const after = applyEdits(pages, arrangeMaps(detachEdits(before, find(pages, "a")), pages, height, pageSize));
    expect(find(after, "a").shape.topic).toMatchObject({ map: "a", order: 0, free: true });
    expect(find(after, "a").shape.topic.parent).toBeUndefined();
    expect(find(after, "a1").shape.topic).toMatchObject({ map: "a", parent: "a" });
    expect(mapsOn(after.get(1) ?? []).get("root")?.children.get("root")?.map((child) => child.id)).toEqual(["b", "c"]);
    expect(detachEdits(before, find(pages, "root"))).toEqual([]);
  });

  it("shows a map's first topic as free or central, sized to match", () => {
    const pages = build(heart());
    const root = find(pages, "root");
    const freed = applyEdits(pages, freeEdits(root, true));
    expect(find(freed, "root").shape.topic.free).toBe(true);
    expect(find(freed, "root").shape.fontSize).toBeLessThan(root.shape.fontSize ?? 0);
    const back = applyEdits(freed, freeEdits(find(freed, "root"), false));
    expect(find(back, "root").shape.topic.free).toBeUndefined();
    expect(find(back, "root").shape.fontSize).toBe(root.shape.fontSize);
    expect(freeEdits(find(pages, "a"), true)).toEqual([]);
  });

  it("joins a free topic dropped on another map's topic to it, with its branches", () => {
    const pages = build([
      ...heart(),
      topic("f", "Blood pressure", { map: "f", order: 0, free: true }),
      topic("f1", "Systolic", { map: "f", parent: "f", order: 0 }),
    ]);
    const f = find(pages, "f");
    const c = find(pages, "c");
    const dropped: Topic = { ...f, shape: { ...f.shape, x: c.shape.x + 4, y: c.shape.y } };
    const after = applyEdits(pages, arrangeMaps([{ id: "f", before: f, after: dropped }], pages, height, pageSize));
    expect(find(after, "f").shape.topic).toMatchObject({ map: "root", parent: "c" });
    expect(find(after, "f").shape.topic.free).toBeUndefined();
    expect(find(after, "f1").shape.topic).toMatchObject({ map: "root", parent: "f" });
  });

  it("passes other marks through untouched", () => {
    const ink: Annotation = { id: "ink", pageNumber: 1, createdAt: 1, shape: { kind: "ink", points: [1, 1, 0.5, 2, 2, 0.5], color: "#000000", strokeWidth: 2, simulatePressure: false } };
    const edits: AnnotationEdit[] = [{ id: "ink", before: null, after: ink }];
    expect(arrangeMaps(edits, groupByPage([]), height, pageSize)).toEqual(edits);
  });
});
