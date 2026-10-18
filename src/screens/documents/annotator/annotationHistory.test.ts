import { describe, expect, it } from "vitest";

import {
  applyEdits,
  EMPTY_HISTORY,
  groupByPage,
  recordStep,
  redoStep,
  undoStep,
  type AnnotationEdit,
} from "./annotationHistory";
import type { Annotation } from "./annotationModel";

function box(id: string, pageNumber: number, createdAt: number, x = 10): Annotation {
  return {
    id,
    pageNumber,
    createdAt,
    shape: { kind: "rectangle", x, y: 10, width: 20, height: 20, color: "#dc2626", strokeWidth: 3 },
  };
}

describe("annotation pages", () => {
  it("groups annotations by page in drawing order", () => {
    const pages = groupByPage([box("c", 2, 5), box("b", 1, 3), box("a", 1, 3), box("d", 1, 1)]);
    expect(pages.get(1)?.map((item) => item.id)).toEqual(["d", "a", "b"]);
    expect(pages.get(2)?.map((item) => item.id)).toEqual(["c"]);
  });

  it("changes only the pages an edit touches, and moves annotations between pages", () => {
    const pages = groupByPage([box("a", 1, 1), box("b", 2, 2), box("c", 3, 3)]);
    const moved = { ...box("a", 2, 1), shape: box("a", 2, 1, 50).shape };
    const next = applyEdits(pages, [
      { id: "a", before: box("a", 1, 1), after: moved },
      { id: "n", before: null, after: box("n", 3, 4) },
    ]);
    expect(next.has(1)).toBe(false);
    expect(next.get(2)?.map((item) => item.id)).toEqual(["a", "b"]);
    expect(next.get(3)?.map((item) => item.id)).toEqual(["c", "n"]);
    expect(applyEdits(pages, [])).toBe(pages);
    const untouched = applyEdits(pages, [{ id: "c", before: box("c", 3, 3), after: null }]);
    expect(untouched.get(2)).toBe(pages.get(2));
    expect(untouched.has(3)).toBe(false);
  });
});

describe("undo history", () => {
  const first: AnnotationEdit[] = [{ id: "a", before: null, after: box("a", 1, 1) }];
  const second: AnnotationEdit[] = [
    { id: "a", before: box("a", 1, 1), after: null },
    { id: "b", before: null, after: box("b", 1, 2) },
  ];

  it("takes back the latest step by applying its edits in reverse", () => {
    const history = recordStep(recordStep(EMPTY_HISTORY, first), second);
    const undone = undoStep(history);
    expect(undone?.edits).toEqual([
      { id: "b", before: box("b", 1, 2), after: null },
      { id: "a", before: null, after: box("a", 1, 1) },
    ]);
    const redone = undone ? redoStep(undone.history) : null;
    expect(redone?.edits).toEqual(second);
    expect(redone?.history.undo).toHaveLength(2);
    expect(redone?.history.redo).toHaveLength(0);
  });

  it("forgets what could be redone once a new step is taken", () => {
    const undone = undoStep(recordStep(EMPTY_HISTORY, first));
    expect(undone?.history.redo).toHaveLength(1);
    const next = recordStep(undone?.history ?? EMPTY_HISTORY, second);
    expect(next.redo).toHaveLength(0);
    expect(redoStep(next)).toBeNull();
    expect(undoStep(EMPTY_HISTORY)).toBeNull();
    expect(recordStep(next, [])).toBe(next);
  });

  it("keeps the most recent 200 steps", () => {
    let history = EMPTY_HISTORY;
    for (let index = 0; index < 205; index += 1) {
      history = recordStep(history, [{ id: `n${index}`, before: null, after: box(`n${index}`, 1, index) }]);
    }
    expect(history.undo).toHaveLength(200);
    expect(history.undo[0]?.[0]?.id).toBe("n5");
  });
});
