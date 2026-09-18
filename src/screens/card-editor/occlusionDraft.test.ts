import { describe, expect, it } from "vitest";

import type { RichDocument } from "../../ipc/common";
import {
  addOcclusionQuestion, applyOcclusionGeometry, collectOcclusionDraftAssets, mutateOcclusionRegion,
  newOcclusionDraft, occlusionDefinition, occlusionDraftFromQuestion, occlusionDraftReady,
  removeOcclusionQuestion, removeOcclusionRegion, undoOcclusionGeometry, updateOcclusionQuestion,
} from "./occlusionDraft";
import { definitionFromDraft, draftFromQuestionNote, isQuestionEditorKind, newQuestionDraft } from "./questionDraft";

const imageId = "a".repeat(32);
const rich = (text: string): RichDocument => ({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] });
function fixture() {
  const empty = { ...newOcclusionDraft(), imageAssetId: imageId, imageSize: { width: 1000, height: 800 } };
  const created = mutateOcclusionRegion(empty, { kind: "create", rectangle: { x: 0.1, y: 0.2, width: 0.2, height: 0.1 } }, false);
  return updateOcclusionQuestion(created, created.questions[0].id, { answer: rich("First answer") });
}

describe("image occlusion authored draft", () => {
  it("dispatches the new type without changing objective draft ownership", () => {
    expect(isQuestionEditorKind("image_occlusion")).toBe(true);
    expect(newQuestionDraft("image_occlusion")).toEqual(newOcclusionDraft());
    expect(newQuestionDraft("single_choice").kind).toBe("single_choice");
    expect(occlusionDraftReady(newOcclusionDraft())).toBe(false);
    expect(() => definitionFromDraft(newOcclusionDraft())).toThrow();
  });

  it("creates independent stable mask and question identities and requires an answer", () => {
    const draft = fixture();
    expect(draft.masks).toHaveLength(1);
    expect(draft.questions).toHaveLength(1);
    expect(draft.questions[0].id).not.toBe(draft.masks[0].id);
    expect(draft.questions[0].maskIds).toEqual([draft.masks[0].id]);
    expect(occlusionDraftReady(draft)).toBe(true);
    const missing = updateOcclusionQuestion(draft, draft.questions[0].id, { answer: rich("") });
    expect(occlusionDraftReady(missing)).toBe(false);
  });

  it("supports several questions per region and several regions per question", () => {
    const first = fixture();
    const second = addOcclusionQuestion(first);
    expect(second.questions[1].maskIds).toEqual(first.questions[0].maskIds);
    expect(second.questions[1].id).not.toBe(first.questions[0].id);
    const multi = mutateOcclusionRegion(second, {
      kind: "create", rectangle: { x: 0.5, y: 0.5, width: 0.2, height: 0.2 },
    }, true);
    expect(multi.questions).toHaveLength(2);
    expect(multi.questions[0].maskIds).toHaveLength(1);
    expect(multi.questions[1].maskIds).toHaveLength(2);
    expect(multi.questions[0].answer).toEqual(first.questions[0].answer);
  });

  it("does not reset question identity when geometry is moved and undone", () => {
    const first = fixture();
    const mask = first.masks[0];
    const moved = applyOcclusionGeometry(first, {
      kind: "change", maskId: mask.id, before: mask.rect, after: { ...mask.rect, x: 0.6 },
    });
    const edited = updateOcclusionQuestion(moved, first.questions[0].id, { answer: rich("Newer answer text") });
    const restored = undoOcclusionGeometry(edited, "undo");
    expect(restored.masks).toEqual(first.masks);
    expect(restored.questions[0].id).toBe(first.questions[0].id);
    expect(restored.questions[0].answer).toEqual(rich("Newer answer text"));
    expect(undoOcclusionGeometry(restored, "redo").masks).toEqual(moved.masks);
  });

  it("removing a region preserves question text and undo restores its bindings", () => {
    const first = fixture();
    const removed = removeOcclusionRegion(first, first.masks[0].id);
    expect(removed.questions[0].answer).toEqual(first.questions[0].answer);
    expect(removed.questions[0].maskIds).toEqual([]);
    expect(occlusionDraftReady(removed)).toBe(false);
    const restored = undoOcclusionGeometry(removed, "undo");
    expect(restored.questions).toEqual(first.questions);
    expect(occlusionDraftReady(restored)).toBe(true);
  });

  it("removing a question never removes shared regions or restores deleted prose through geometry undo", () => {
    const first = addOcclusionQuestion(fixture());
    const moved = applyOcclusionGeometry(first, {
      kind: "change", maskId: first.masks[0].id, before: first.masks[0].rect,
      after: { ...first.masks[0].rect, x: 0.6 },
    });
    const removed = removeOcclusionQuestion(moved, first.questions[0].id);
    expect(removed.questions).toHaveLength(1);
    expect(removed.masks).toHaveLength(1);
    expect(undoOcclusionGeometry(removed, "undo").questions).toEqual(removed.questions);
  });

  it("excludes editor/import state from canonical source and round-trips source identities", () => {
    const draft = fixture();
    const definition = occlusionDefinition(draft);
    expect(Object.keys(definition.question).sort()).toEqual(["imageAssetId", "kind", "masks", "questions"]);
    if (definition.question.kind !== "image_occlusion") throw new Error("Wrong authored kind");
    const reopened = occlusionDraftFromQuestion(definition.question);
    expect(reopened.imageSize).toBeNull();
    expect(reopened.history.undo).toEqual([]);
    expect(reopened.pendingImport).toBeNull();
    expect(occlusionDefinition(reopened)).toEqual(definition);
    const note = { noteId: "1".repeat(32), deckId: "2".repeat(32), definition,
      assets: [], tags: [], cardIds: [], createdAt: 1, updatedAt: 1, revision: 1 };
    expect(draftFromQuestionNote(note)).toEqual(reopened);
  });

  it("retains source and rich-answer assets and blocks publication during an unresolved import", () => {
    let draft = fixture();
    const answer = { type: "doc", content: [{ type: "image", attrs: { assetId: "b".repeat(32), src: `study-asset://${"b".repeat(32)}` } }] };
    draft = updateOcclusionQuestion(draft, draft.questions[0].id, { answer });
    expect(collectOcclusionDraftAssets(draft)).toEqual([imageId, "b".repeat(32)]);
    const pending = { ...draft, pendingImport: { operationId: "c".repeat(32), assetDraftId: "d".repeat(32), stagedFileId: "e".repeat(32) } };
    expect(occlusionDraftReady(pending)).toBe(false);
    expect(JSON.parse(JSON.stringify(pending)).pendingImport).toEqual(pending.pendingImport);
  });

  it("rejects missing targets, duplicate identities and non-finite or out-of-image regions", () => {
    const draft = fixture();
    expect(occlusionDraftReady({ ...draft, masks: [{ ...draft.masks[0], rect: { x: NaN, y: 0, width: 1, height: 1 } }] })).toBe(false);
    expect(occlusionDraftReady({ ...draft, masks: [{ ...draft.masks[0], rect: { x: 0.8, y: 0, width: 0.4, height: 1 } }] })).toBe(false);
    expect(occlusionDraftReady({ ...draft, questions: [{ ...draft.questions[0], id: draft.masks[0].id }] })).toBe(false);
    expect(occlusionDraftReady(updateOcclusionQuestion(draft, draft.questions[0].id, { maskIds: ["f".repeat(32)] }))).toBe(false);
  });
});
