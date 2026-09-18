import { describe, expect, it } from "vitest";

import { commitGeometry, EMPTY_GEOMETRY_HISTORY, stepGeometryHistory } from "./history";

const a = { id: "a", rect: { x: 0.1, y: 0.1, width: 0.2, height: 0.2 } };
const b = { id: "b", rect: { x: 0.5, y: 0.5, width: 0.1, height: 0.1 } };
const state = { masks: [a, b], questions: [
  { id: "q1", maskIds: ["a"], answer: "First answer" },
  { id: "q2", maskIds: ["a", "b"], answer: "Second answer" },
] };

describe("region geometry history", () => {
  it("undoes movement without overwriting a newer rich-text answer", () => {
    const moved = commitGeometry(state, EMPTY_GEOMETRY_HISTORY, { kind: "change", maskId: "a", before: a.rect, after: { ...a.rect, x: 0.3 } });
    const edited = { ...moved.state, questions: moved.state.questions.map((q) => ({ ...q, answer: "Newer answer" })) };
    const undone = stepGeometryHistory(edited, moved.history, "undo");
    expect(undone.state.masks).toEqual(state.masks);
    expect(undone.state.questions.map((q) => q.answer)).toEqual(["Newer answer", "Newer answer"]);
    expect(stepGeometryHistory(undone.state, undone.history, "redo").state.masks[0].rect.x).toBe(0.3);
  });

  it("removal clears references, preserves question drafts and restores only still-existing questions", () => {
    const removed = commitGeometry(state, EMPTY_GEOMETRY_HISTORY, { kind: "remove", maskId: "a", expected: a.rect });
    expect(removed.state.questions.map((q) => q.maskIds)).toEqual([[], ["b"]]);
    expect(removed.state.questions[0].answer).toBe("First answer");
    const edited = { ...removed.state, questions: [{ ...removed.state.questions[1], answer: "Updated" }] };
    const undone = stepGeometryHistory(edited, removed.history, "undo");
    expect(undone.state.masks).toEqual(state.masks);
    expect(undone.state.questions).toEqual([{ id: "q2", maskIds: ["b", "a"], answer: "Updated" }]);
    expect(stepGeometryHistory(undone.state, undone.history, "redo").state.questions[0].maskIds).toEqual(["b"]);
  });

  it("refuses stale mutations and never overwrites a conflicting region on undo", () => {
    expect(commitGeometry(state, EMPTY_GEOMETRY_HISTORY, { kind: "change", maskId: "a", before: b.rect, after: a.rect }).applied).toBe(false);
    const moved = commitGeometry(state, EMPTY_GEOMETRY_HISTORY, { kind: "change", maskId: "a", before: a.rect, after: { ...a.rect, x: 0.3 } });
    const externallyChanged = { ...moved.state, masks: [{ ...a, rect: { ...a.rect, x: 0.6 } }, b] };
    const undone = stepGeometryHistory(externallyChanged, moved.history, "undo");
    expect(undone.applied).toBe(false);
    expect(undone.state).toBe(externallyChanged);
    expect(undone.history).toEqual(EMPTY_GEOMETRY_HISTORY);
  });

  it("does not create duplicate region IDs or no-op undo entries", () => {
    expect(commitGeometry(state, EMPTY_GEOMETRY_HISTORY, { kind: "insert", mask: a, index: 0, restoreQuestionIds: [] }).applied).toBe(false);
    expect(commitGeometry(state, EMPTY_GEOMETRY_HISTORY, { kind: "change", maskId: "a", before: a.rect, after: a.rect }).applied).toBe(false);
    expect(state.questions[0].maskIds).toEqual(["a"]);
  });

  it("a new edit discards redo without dropping older undo operations", () => {
    const moved = commitGeometry(state, EMPTY_GEOMETRY_HISTORY, { kind: "change", maskId: "a", before: a.rect, after: { ...a.rect, x: 0.3 } });
    const undone = stepGeometryHistory(moved.state, moved.history, "undo");
    const different = commitGeometry(undone.state, undone.history, { kind: "remove", maskId: "b", expected: b.rect });
    expect(different.history.redo).toEqual([]);
    expect(different.history.undo).toHaveLength(1);
  });
});
