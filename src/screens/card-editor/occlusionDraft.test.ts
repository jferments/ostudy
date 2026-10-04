import { describe, expect, it } from "vitest";

import { requiredTestValue as required } from "../../test/factories";
import type { RichDocument } from "../../ipc/common";
import {
  addOcclusionQuestion, applyOcclusionGeometry, collectOcclusionDraftAssets, mutateOcclusionRegion,
  newOcclusionDraft, occlusionDefinition, occlusionDraftFromQuestion, occlusionDraftReady, occlusionStudyQuestions,
  removeOcclusionQuestion, removeOcclusionRegion, selectOcclusionRegion, setOcclusionMode, undoOcclusionGeometry, updateOcclusionQuestion,
} from "./occlusionDraft";
import { definitionFromDraft, draftFromQuestionNote, isQuestionEditorKind, newQuestionDraft } from "./questionDraft";

const imageId = "a".repeat(32);
const rich = (text: string): RichDocument => ({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] });
const rectangle = { x: 0.1, y: 0.2, width: 0.2, height: 0.1 };
function quickFixture() {
  const empty = { ...newOcclusionDraft(), imageAssetId: imageId, imageSize: { width: 1000, height: 800 } };
  return mutateOcclusionRegion(empty, { kind: "create", rectangle }, false);
}
function fixture() {
  const created = quickFixture();
  return updateOcclusionQuestion(created, required(created.questions[0], "created question 1").id, { answer: rich("First answer") });
}

describe("image occlusion authored draft", () => {
  it("dispatches the new type without changing objective draft ownership", () => {
    expect(isQuestionEditorKind("image_occlusion")).toBe(true);
    expect(newQuestionDraft("image_occlusion")).toEqual(newOcclusionDraft());
    expect(newQuestionDraft("single_choice").kind).toBe("single_choice");
    expect(occlusionDraftReady(newOcclusionDraft())).toBe(false);
    expect(() => definitionFromDraft(newOcclusionDraft())).toThrow();
  });

  it("creates independent stable mask and question identities without requiring written prose", () => {
    const draft = quickFixture();
    expect(draft.masks).toHaveLength(1);
    expect(draft.questions).toHaveLength(1);
    expect(required(draft.questions[0], "draft question 1").id).not.toBe(required(draft.masks[0], "draft mask 1").id);
    expect(required(draft.questions[0], "draft question 1").maskIds).toEqual([required(draft.masks[0], "draft mask 1").id]);
    expect(occlusionDraftReady(draft)).toBe(true);
    const blank = updateOcclusionQuestion(draft, required(draft.questions[0], "draft question 1").id, { prompt: rich(""), answer: rich("") });
    expect(occlusionDraftReady(blank)).toBe(true);
    expect(occlusionDefinition(blank)).toEqual(occlusionDefinition(draft));
    expect(draft.questions[0]?.mode).toBe("hide_target");
  });

  it("draws independent ready-to-study rectangles and switches question-side masks without replacing IDs or prose", () => {
    const first = fixture();
    const second = mutateOcclusionRegion(first, { kind: "create", rectangle: { ...rectangle, x: 0.5 } }, false);
    const before = occlusionDefinition(second);
    const all = setOcclusionMode(second, "hide_all");
    const third = mutateOcclusionRegion(all, { kind: "create", rectangle: { ...rectangle, y: 0.6 } }, false);
    expect(occlusionDraftReady(third)).toBe(true);
    expect(occlusionStudyQuestions(third)).toHaveLength(3);
    expect(third.questions.every((question) => question.mode === "hide_all")).toBe(true);
    expect(third.questions.slice(0, 2).map((question) => question.id)).toEqual(second.questions.map((question) => question.id));
    expect(third.questions[0]?.answer).toEqual(rich("First answer"));
    expect(occlusionDefinition(setOcclusionMode(all, "hide_target"))).toEqual(before);
    const selected = selectOcclusionRegion(third, required(third.masks[0], "first mask").id);
    expect(selected.selectedQuestionId).toBe(third.questions[0]?.id);
  });

  it("supports several questions per region and several regions per question", () => {
    const first = fixture();
    const second = addOcclusionQuestion(first);
    expect(required(second.questions[1], "second question 2").maskIds).toEqual(required(first.questions[0], "first question 1").maskIds);
    expect(required(second.questions[1], "second question 2").id).not.toBe(required(first.questions[0], "first question 1").id);
    const multi = mutateOcclusionRegion(second, {
      kind: "create", rectangle: { x: 0.5, y: 0.5, width: 0.2, height: 0.2 },
    }, true);
    expect(multi.questions).toHaveLength(2);
    expect(required(multi.questions[0], "multi question 1").maskIds).toHaveLength(1);
    expect(required(multi.questions[1], "multi question 2").maskIds).toHaveLength(2);
    expect(required(multi.questions[0], "multi question 1").answer).toEqual(required(first.questions[0], "first question 1").answer);
    expect(occlusionDefinition(undoOcclusionGeometry(undoOcclusionGeometry(multi, "undo"), "redo"))).toEqual(occlusionDefinition(multi));
  });

  it("undoes, deletes, and restores a quick rectangle without blocking remaining cards or changing question identity", () => {
    const first = quickFixture();
    const second = mutateOcclusionRegion(first, { kind: "create", rectangle: { ...rectangle, x: 0.5 } }, false);
    const undone = undoOcclusionGeometry(second, "undo");
    expect(occlusionDraftReady(undone)).toBe(true);
    expect(occlusionDefinition(undone)).toEqual(occlusionDefinition(first));
    expect(occlusionStudyQuestions(undone)).toHaveLength(1);
    expect(occlusionDefinition(undoOcclusionGeometry(undone, "redo"))).toEqual(occlusionDefinition(second));
    const removed = removeOcclusionRegion(second, required(second.masks[1], "second mask").id);
    expect(occlusionDefinition(removed)).toEqual(occlusionDefinition(first));
    expect(occlusionDefinition(undoOcclusionGeometry(removed, "undo"))).toEqual(occlusionDefinition(second));
    const redrawn = mutateOcclusionRegion(undone, { kind: "create", rectangle: { ...rectangle, y: 0.5 } }, false);
    expect(redrawn.questions).toHaveLength(2);
    expect(redrawn.history.redo).toEqual([]);
    expect(redrawn.questions.map((question) => question.id)).not.toContain(second.questions[1]?.id);
  });

  it("does not insert a stray mask at the standalone-card limit but still permits grouped regions", () => {
    let draft = quickFixture();
    for (let index = 1; index < 64; index++) draft = mutateOcclusionRegion(draft, { kind: "create", rectangle }, false);
    expect(occlusionDraftReady(draft)).toBe(true);
    expect(draft.masks).toHaveLength(64);
    expect(mutateOcclusionRegion(draft, { kind: "create", rectangle }, false)).toBe(draft);
    const grouped = mutateOcclusionRegion(draft, { kind: "create", rectangle }, true);
    expect(grouped.masks).toHaveLength(65);
    expect(occlusionStudyQuestions(grouped)).toHaveLength(64);
    expect(occlusionDraftReady(grouped)).toBe(true);
    expect(addOcclusionQuestion(newOcclusionDraft())).toEqual(newOcclusionDraft());
  });

  it("does not reset question identity when geometry is moved and undone", () => {
    const first = fixture();
    const mask = required(first.masks[0], "first mask 1");
    const moved = applyOcclusionGeometry(first, {
      kind: "change", maskId: mask.id, before: mask.rect, after: { ...mask.rect, x: 0.6 },
    });
    const edited = updateOcclusionQuestion(moved, required(first.questions[0], "first question 1").id, { answer: rich("Newer answer text") });
    const restored = undoOcclusionGeometry(edited, "undo");
    expect(restored.masks).toEqual(first.masks);
    expect(required(restored.questions[0], "restored question 1").id).toBe(required(first.questions[0], "first question 1").id);
    expect(required(restored.questions[0], "restored question 1").answer).toEqual(rich("Newer answer text"));
    expect(undoOcclusionGeometry(restored, "redo").masks).toEqual(moved.masks);
  });

  it("removing a region preserves custom question text and undo restores its bindings", () => {
    const first = fixture();
    const removed = removeOcclusionRegion(first, required(first.masks[0], "first mask 1").id);
    expect(required(removed.questions[0], "removed question 1").answer).toEqual(required(first.questions[0], "first question 1").answer);
    expect(required(removed.questions[0], "removed question 1").maskIds).toEqual([]);
    expect(occlusionDraftReady(removed)).toBe(false);
    const restored = undoOcclusionGeometry(removed, "undo");
    expect(restored.questions).toEqual(first.questions);
    expect(occlusionDraftReady(restored)).toBe(true);
  });

  it("removing a question never removes shared regions or restores deleted prose through geometry undo", () => {
    const first = addOcclusionQuestion(fixture());
    const moved = applyOcclusionGeometry(first, {
      kind: "change", maskId: required(first.masks[0], "first mask 1").id, before: required(first.masks[0], "first mask 1").rect,
      after: { ...required(first.masks[0], "first mask 1").rect, x: 0.6 },
    });
    const removed = removeOcclusionQuestion(moved, required(first.questions[0], "first question 1").id);
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
    draft = updateOcclusionQuestion(draft, required(draft.questions[0], "draft question 1").id, { answer });
    expect(collectOcclusionDraftAssets(draft)).toEqual([imageId, "b".repeat(32)]);
    const pending = { ...draft, pendingImport: { operationId: "c".repeat(32), assetDraftId: "d".repeat(32), stagedFileId: "e".repeat(32) } };
    expect(occlusionDraftReady(pending)).toBe(false);
    expect(JSON.parse(JSON.stringify(pending)).pendingImport).toEqual(pending.pendingImport);
  });

  it("rejects missing targets, duplicate identities and non-finite or out-of-image regions", () => {
    const draft = fixture();
    expect(occlusionDraftReady({ ...draft, masks: [{ ...required(draft.masks[0], "draft mask 1"), rect: { x: NaN, y: 0, width: 1, height: 1 } }] })).toBe(false);
    expect(occlusionDraftReady({ ...draft, masks: [{ ...required(draft.masks[0], "draft mask 1"), rect: { x: 0.8, y: 0, width: 0.4, height: 1 } }] })).toBe(false);
    expect(occlusionDraftReady({ ...draft, questions: [{ ...required(draft.questions[0], "draft question 1"), id: required(draft.masks[0], "draft mask 1").id }] })).toBe(false);
    expect(occlusionDraftReady(updateOcclusionQuestion(draft, required(draft.questions[0], "draft question 1").id, { maskIds: ["f".repeat(32)] }))).toBe(false);
  });
});
