import type { QuestionDefinition, QuestionNote } from "../../ipc/questions";
import * as objective from "./objectiveQuestionDraft";
import {
  collectOcclusionDraftAssets, newOcclusionDraft, occlusionDefinition,
  occlusionDraftFromQuestion, occlusionDraftReady, type OcclusionEditorDraft,
} from "./occlusionDraft";

// Objective editors keep their established adapters. This boundary dispatches
// between authored question families rather than duplicating their field models.
export * from "./objectiveQuestionDraft";
export type QuestionEditorKind = objective.QuestionEditorKind | "image_occlusion";
export type QuestionEditorDraft = objective.QuestionEditorDraft | OcclusionEditorDraft;
export type QuestionDraftChange = QuestionEditorDraft | ((current: QuestionEditorDraft) => QuestionEditorDraft);

export function isQuestionEditorKind(value: string): value is QuestionEditorKind {
  return value === "image_occlusion" || objective.isQuestionEditorKind(value);
}

export function questionKindForNote(note: QuestionNote): QuestionEditorKind | null {
  const kind = note.definition.question.kind;
  return isQuestionEditorKind(kind) ? kind : null;
}

export function newQuestionDraft(kind: QuestionEditorKind): QuestionEditorDraft {
  return kind === "image_occlusion" ? newOcclusionDraft() : objective.newQuestionDraft(kind);
}

export function draftFromQuestionNote(note: QuestionNote): QuestionEditorDraft | null {
  return note.definition.question.kind === "image_occlusion"
    ? occlusionDraftFromQuestion(note.definition.question)
    : objective.draftFromQuestionNote(note);
}

export function questionDraftReady(draft: QuestionEditorDraft): boolean {
  return draft.kind === "image_occlusion" ? occlusionDraftReady(draft) : objective.questionDraftReady(draft);
}

export function definitionFromDraft(draft: QuestionEditorDraft): QuestionDefinition {
  return draft.kind === "image_occlusion" ? occlusionDefinition(draft) : objective.definitionFromDraft(draft);
}

export function collectQuestionDraftAssetIds(draft: QuestionEditorDraft): string[] {
  return draft.kind === "image_occlusion" ? collectOcclusionDraftAssets(draft) : objective.collectQuestionDraftAssetIds(draft);
}
