import type { RichDocument } from "../../ipc/common";
import { createOperationId } from "../../ipc/common";
import type { ImageMask, OcclusionQuestion, QuestionBody, QuestionDefinition } from "../../ipc/questions";
import { cloneDocument, collectAssetIds, EMPTY_RICH_DOCUMENT, isDocumentEmpty } from "../../editor/richText";
import { validateRectangle, type Size } from "../../editor/occlusion/geometry";
import {
  commitGeometry, EMPTY_GEOMETRY_HISTORY, stepGeometryHistory,
  type GeometryCommand, type GeometryHistory,
} from "../../editor/occlusion/history";
import type { RegionMutation } from "../../editor/occlusion/gestures";
import { hasCustomOcclusionContent, withOcclusionDefaults } from "./occlusionQuestionDefaults";

/** Recovery-only import state. Neither this receipt nor editor history enters a saved definition. */
export interface PendingOcclusionImport {
  operationId: string;
  assetDraftId: string;
  stagedFileId: string | null;
}

export interface OcclusionEditorDraft {
  kind: "image_occlusion";
  imageAssetId: string | null;
  imageSize: Size | null;
  masks: ImageMask[];
  questions: OcclusionQuestion[];
  selectedMaskId: string | null;
  selectedQuestionId: string | null;
  history: GeometryHistory;
  pendingImport: PendingOcclusionImport | null;
}
export type OcclusionDraftUpdate = (draft: OcclusionEditorDraft) => OcclusionEditorDraft;

export function newOcclusionDraft(): OcclusionEditorDraft {
  return {
    kind: "image_occlusion", imageAssetId: null, imageSize: null,
    masks: [], questions: [], selectedMaskId: null, selectedQuestionId: null,
    history: EMPTY_GEOMETRY_HISTORY, pendingImport: null,
  };
}

export function occlusionDraftFromQuestion(question: Extract<QuestionBody, { kind: "image_occlusion" }>): OcclusionEditorDraft {
  return {
    ...newOcclusionDraft(), imageAssetId: question.imageAssetId,
    masks: question.masks.map((mask) => ({ ...mask, rect: { ...mask.rect } })),
    questions: question.questions.map((item) => ({
      ...item, maskIds: [...item.maskIds], prompt: cloneDocument(item.prompt), answer: cloneDocument(item.answer),
      explanation: item.explanation ? cloneDocument(item.explanation) : null,
    })),
    selectedMaskId: question.masks[0]?.id ?? null,
    selectedQuestionId: question.questions[0]?.id ?? null,
  };
}

/** Undo may retain a default-only question with no rectangle. It is not an
 * incomplete study card and must not prevent the remaining rectangles saving. */
export function occlusionStudyQuestions(draft: OcclusionEditorDraft): OcclusionQuestion[] {
  return draft.questions.filter((question) => question.maskIds.length > 0 || hasCustomOcclusionContent(question));
}

export function occlusionDraftReady(draft: OcclusionEditorDraft): boolean {
  const questions = occlusionStudyQuestions(draft);
  if (!draft.imageAssetId || draft.pendingImport || draft.masks.length < 1 || draft.masks.length > 128
    || questions.length < 1 || questions.length > 64) return false;
  const ids = new Set<string>();
  const acceptId = (id: string): boolean => {
    if (!/^[0-9a-f]{32}$/u.test(id) || ids.has(id)) return false;
    ids.add(id);
    return true;
  };
  if (!/^[0-9a-f]{32}$/u.test(draft.imageAssetId)) return false;
  for (const mask of draft.masks) {
    if (!acceptId(mask.id)) return false;
    try { validateRectangle(mask.rect); } catch { return false; }
    // Match native admission exactly; floating-point tolerance is for geometry tests only.
    if (mask.rect.x + mask.rect.width > 1 || mask.rect.y + mask.rect.height > 1) return false;
  }
  const masks = new Set(draft.masks.map((mask) => mask.id));
  return questions.every((question) => acceptId(question.id)
    && question.maskIds.length > 0 && new Set(question.maskIds).size === question.maskIds.length
    && question.maskIds.every((id) => masks.has(id))
    && (question.mode === "hide_all" || question.mode === "hide_target"));
}

export function occlusionDefinition(draft: OcclusionEditorDraft): QuestionDefinition {
  if (!occlusionDraftReady(draft) || !draft.imageAssetId) {
    throw new Error("Choose an image and draw at least one valid region. Custom questions must each have a target region.");
  }
  return {
    schemaVersion: 1,
    question: {
      kind: "image_occlusion", imageAssetId: draft.imageAssetId,
      masks: draft.masks.map((mask) => ({ ...mask, rect: { ...mask.rect } })),
      questions: occlusionStudyQuestions(draft).map((question) => ({ ...withOcclusionDefaults(question), maskIds: [...question.maskIds] })),
    },
  };
}

export function collectOcclusionDraftAssets(draft: OcclusionEditorDraft): string[] {
  const ids = new Set<string>(draft.imageAssetId ? [draft.imageAssetId] : []);
  for (const question of draft.questions) {
    for (const document of [question.prompt, question.answer, question.explanation]) {
      if (document) for (const id of collectAssetIds(document)) ids.add(id);
    }
  }
  return [...ids].sort();
}

export function newOcclusionQuestion(maskIds: string[], mode: OcclusionQuestion["mode"] = "hide_target"): OcclusionQuestion {
  return withOcclusionDefaults({
    id: createOperationId(), maskIds: [...maskIds], mode,
    prompt: cloneDocument(EMPTY_RICH_DOCUMENT), answer: cloneDocument(EMPTY_RICH_DOCUMENT), explanation: null,
  });
}

export function updateOcclusionQuestion(draft: OcclusionEditorDraft, id: string, update: Partial<Omit<OcclusionQuestion, "id">>): OcclusionEditorDraft {
  return { ...draft, questions: draft.questions.map((question) => question.id === id ? { ...question, ...update } : question) };
}

export function setOcclusionMode(draft: OcclusionEditorDraft, mode: OcclusionQuestion["mode"]): OcclusionEditorDraft {
  return { ...draft, questions: draft.questions.map((question) => ({ ...question, mode })) };
}

export function selectOcclusionRegion(draft: OcclusionEditorDraft, id: string | null): OcclusionEditorDraft {
  const current = draft.questions.find((question) => question.id === draft.selectedQuestionId);
  const question = id && !current?.maskIds.includes(id)
    ? draft.questions.find((candidate) => candidate.maskIds.includes(id)) : current;
  return { ...draft, selectedMaskId: id, selectedQuestionId: question?.id ?? draft.selectedQuestionId };
}

export function addOcclusionQuestion(draft: OcclusionEditorDraft): OcclusionEditorDraft {
  if (occlusionStudyQuestions(draft).length >= 64) return draft;
  const selected = draft.masks.find((mask) => mask.id === draft.selectedMaskId) ?? draft.masks[0];
  if (!selected) return draft;
  const mode = draft.questions.find((question) => question.id === draft.selectedQuestionId)?.mode;
  const question = newOcclusionQuestion([selected.id], mode);
  return { ...draft, questions: [...draft.questions, question], selectedQuestionId: question.id };
}

export function removeOcclusionQuestion(draft: OcclusionEditorDraft, id: string): OcclusionEditorDraft {
  const questions = draft.questions.filter((question) => question.id !== id);
  return {
    ...draft, questions,
    selectedQuestionId: draft.selectedQuestionId === id ? questions[0]?.id ?? null : draft.selectedQuestionId,
  };
}

function retainQuestionHistory(draft: OcclusionEditorDraft): OcclusionEditorDraft {
  const retained = new Set([...draft.history.undo, ...draft.history.redo]
    .flatMap((command) => command.kind === "insert" ? command.restoreQuestionIds : []));
  const questions = draft.questions.filter((question) => question.maskIds.length > 0
    || hasCustomOcclusionContent(question) || retained.has(question.id));
  return { ...draft, questions, selectedQuestionId: questions.some((question) => question.id === draft.selectedQuestionId)
    ? draft.selectedQuestionId : questions[0]?.id ?? null };
}

export function applyOcclusionGeometry(draft: OcclusionEditorDraft, command: GeometryCommand): OcclusionEditorDraft {
  const transition = commitGeometry(draft, draft.history, command);
  return transition.applied ? retainQuestionHistory({ ...draft, ...transition.state, history: transition.history }) : draft;
}

/** One completed gesture is one geometry history entry; moves never replace question prose. */
export function mutateOcclusionRegion(draft: OcclusionEditorDraft, mutation: RegionMutation, addToCurrent: boolean,
  mode?: OcclusionQuestion["mode"]): OcclusionEditorDraft {
  if (mutation.kind === "change") return applyOcclusionGeometry(draft, mutation);
  if (draft.masks.length >= 128) return draft;
  const current = draft.questions.find((question) => question.id === draft.selectedQuestionId);
  if ((!addToCurrent || !current) && occlusionStudyQuestions(draft).length >= 64) return draft;
  const mask: ImageMask = { id: createOperationId(), rect: mutation.rectangle };
  let next = applyOcclusionGeometry(draft, { kind: "insert", mask, index: draft.masks.length,
    restoreQuestionIds: addToCurrent && current ? [current.id] : [] });
  if (next === draft) return draft;
  next = { ...next, selectedMaskId: mask.id };
  if (addToCurrent && current) return { ...next, selectedQuestionId: current.id };
  const question = newOcclusionQuestion([mask.id], mode ?? current?.mode);
  return { ...next, questions: [...next.questions, question], selectedQuestionId: question.id };
}

export function removeOcclusionRegion(draft: OcclusionEditorDraft, id: string): OcclusionEditorDraft {
  const mask = draft.masks.find((mask) => mask.id === id);
  if (!mask) return draft;
  const next = applyOcclusionGeometry(draft, { kind: "remove", maskId: id, expected: mask.rect });
  return selectOcclusionRegion(next, next.masks[0]?.id ?? null);
}

export function undoOcclusionGeometry(draft: OcclusionEditorDraft, direction: "undo" | "redo"): OcclusionEditorDraft {
  const transition = stepGeometryHistory(draft, draft.history, direction);
  const selectedMaskId = transition.state.masks.some((mask) => mask.id === draft.selectedMaskId)
    ? draft.selectedMaskId : transition.state.masks[0]?.id ?? null;
  return retainQuestionHistory({ ...draft, ...transition.state, history: transition.history, selectedMaskId });
}

export function optionalOcclusionField(document: RichDocument): RichDocument | null {
  return isDocumentEmpty("", document) ? null : document;
}
