import { invoke } from "@tauri-apps/api/core";

import type { Asset } from "./assets";
import type { RichDocument } from "./common";
import type { EditorDraftCommitRef } from "./drafts";
import type {
  PublicQuestion,
  QuestionEvaluation,
  QuestionResponse,
} from "./questionAttempts";

export interface QuestionItem {
  id: string;
  content: RichDocument;
}

export interface ChoiceOption extends QuestionItem {
  rationale: RichDocument | null;
}

export interface MatchingPair {
  left: QuestionItem;
  right: QuestionItem;
  rationale: RichDocument | null;
}

export interface GroupedItem {
  item: QuestionItem;
  categoryId: string;
  rationale: RichDocument | null;
}

export interface NormalizedRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ImageMask {
  id: string;
  rect: NormalizedRect;
}

export interface OcclusionQuestion {
  id: string;
  prompt: RichDocument;
  answer: RichDocument;
  explanation: RichDocument | null;
  maskIds: string[];
  mode: "hide_target" | "hide_all";
}

interface QuestionStem {
  prompt: RichDocument;
  explanation: RichDocument | null;
}

/** Canonical native definition, not an incomplete editor draft or study view. */
export type QuestionBody =
  | (QuestionStem & {
      kind: "single_choice";
      options: ChoiceOption[];
      correctOptionId: string;
    })
  | (QuestionStem & {
      kind: "select_all";
      options: ChoiceOption[];
      correctOptionIds: string[];
    })
  | {
      kind: "true_false";
      statement: RichDocument;
      correct: boolean;
      explanation: RichDocument | null;
      correctedStatement: RichDocument | null;
    }
  | (QuestionStem & {
      kind: "matching";
      pairs: MatchingPair[];
    })
  | (QuestionStem & {
      kind: "grouping";
      categories: QuestionItem[];
      items: GroupedItem[];
    })
  | (QuestionStem & {
      kind: "ordering";
      items: QuestionItem[];
    })
  | {
      kind: "image_occlusion";
      imageAssetId: string;
      masks: ImageMask[];
      questions: OcclusionQuestion[];
    };

export type QuestionKind = QuestionBody["kind"];
export type ObjectiveQuestionKind = Exclude<QuestionKind, "image_occlusion">;

export interface QuestionDefinition {
  schemaVersion: 1;
  question: QuestionBody;
}

export interface SaveQuestionNoteInput {
  operationId: string;
  noteId: string | null;
  revision: number | null;
  deckId: string;
  definitionJson: string;
  tags: string[];
  allowDuplicate: boolean;
  assetDraftId?: string;
  editorDraft?: EditorDraftCommitRef;
}

/** A receipt is proof of a committed operation, not a current-note snapshot. */
export interface QuestionWriteReceipt {
  operationId: string;
  noteId: string;
  noteRevision: number;
  cardIds: string[];
  committedAt: number;
}

export interface QuestionNote {
  noteId: string;
  deckId: string;
  definition: QuestionDefinition;
  assets: Asset[];
  tags: string[];
  cardIds: string[];
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface PreviewQuestionDraftInput {
  definitionJson: string;
  response: QuestionResponse | null;
}

export interface QuestionDraftPreview {
  question: PublicQuestion;
  evaluation: QuestionEvaluation | null;
}

export async function previewQuestionDraft(
  input: PreviewQuestionDraftInput,
): Promise<QuestionDraftPreview> {
  return invoke<QuestionDraftPreview>("preview_question_draft", { input });
}

export async function saveQuestionNote(
  input: SaveQuestionNoteInput,
): Promise<QuestionWriteReceipt> {
  return invoke<QuestionWriteReceipt>("save_question_note", { input });
}

export async function getQuestionNote(noteId: string): Promise<QuestionNote> {
  return invoke<QuestionNote>("get_question_note", { noteId });
}
