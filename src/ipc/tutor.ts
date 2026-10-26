import { invoke } from "./transport";

import type { FigureBox } from "./documentFigures";
import type { SourceCitation, StudyScope } from "./knowledge";
import type { NarrationSegment } from "./lessons";

export type TutorMode = "explain" | "hint" | "quiz" | "answer";

export type TutorFocus =
  | { kind: "card"; sessionId: string | null; cardId: string; noteRevision: number }
  | {
      kind: "guide";
      documentId: string;
      guideId: string;
      topicId: string | null;
      contentHash: string | null;
    };

export interface TutorTarget {
  scope: StudyScope;
  focus: TutorFocus | null;
  title: string;
  mode: "explain" | "hint";
}

export interface TutorProject {
  id: string;
  title: string;
  description: string;
  instructions: string;
  defaultScope: StudyScope;
  revision: number;
  archivedAt: number | null;
  createdAt: number;
  updatedAt: number;
}

export interface ConversationSummary {
  id: string;
  title: string;
  scope: StudyScope;
  projectId: string | null;
  revision: number;
  organizationRevision: number;
  archivedAt: number | null;
  createdAt: number;
  updatedAt: number;
}

export interface QuizFeedback {
  outcome: "correct" | "partial" | "incorrect" | "unclear";
  explanation: string;
  criteriaFeedback: string[];
  /** Whether the answer met each criterion, in order; empty for older feedback and choose-one answers. */
  criteriaMet: boolean[];
  /** The option a choose-one answer picked. */
  chosen: number | null;
}

/** A quiz question's public part. What it expected arrives once it is answered. */
export interface TutorQuestion {
  number: number;
  total: number | null;
  /** A choose-one question's options; empty for a short answer. */
  choices: string[];
  answered: boolean;
  expectedAnswer?: string;
  explanation?: string;
  correctChoice?: number;
}

export interface TutorTurn {
  id: string;
  operationId: string;
  sequence: number;
  mode: TutorMode;
  userText: string;
  assistantText: string;
  state: "running" | "complete" | "failed" | "interrupted" | "canceled";
  citations: SourceCitation[];
  evidenceCurrent: boolean;
  pendingQuestion: boolean;
  feedback: QuizFeedback | null;
  question: TutorQuestion | null;
  /** The question an answer answers. */
  questionId: string | null;
  provider: string;
  model: string;
  errorCode: string;
  errorMessage: string;
  totalTokens: number | null;
  createdAt: number;
  /** The newer version that replaced this turn; the chat carries on from it. */
  replacedByTurnId: string | null;
}

export interface ConversationDetail {
  conversation: ConversationSummary;
  turns: TutorTurn[];
  hasEarlierTurns: boolean;
}

export interface TutorTurnInput {
  operationId: string;
  conversationId: string;
  expectedRevision: number;
  mode: TutorMode;
  text: string;
  focus: TutorFocus | null;
  profileId: string | null;
  includeConversations: boolean;
  questionId: string | null;
  /** The conversation's last answer this one replaces: Regenerate, or the question edited and sent again. */
  replacesTurnId?: string;
  /** The question a quiz's Next question follows. */
  afterQuestionId?: string;
  /** The option picked for a choose-one question. */
  choice?: number;
}

export async function listTutorProjects(): Promise<TutorProject[]> {
  return invoke<TutorProject[]>("list_tutor_projects");
}

export async function createTutorProject(input: {
  operationId: string;
  title: string;
  description: string;
  instructions: string;
  defaultScope: StudyScope;
}): Promise<TutorProject> {
  return invoke<TutorProject>("create_tutor_project", { input });
}

export async function updateTutorProject(input: {
  operationId: string;
  projectId: string;
  expectedRevision: number;
  title: string;
  description: string;
  instructions: string;
  defaultScope: StudyScope;
}): Promise<TutorProject> {
  return invoke<TutorProject>("update_tutor_project", { input });
}

export async function setTutorProjectArchived(input: {
  operationId: string;
  projectId: string;
  expectedRevision: number;
  archived: boolean;
}): Promise<TutorProject> {
  return invoke<TutorProject>("set_tutor_project_archived", { input });
}

export async function deleteTutorProject(input: {
  operationId: string;
  projectId: string;
  expectedRevision: number;
}): Promise<void> {
  return invoke<void>("delete_tutor_project", { input });
}

export async function listTutorConversations(): Promise<ConversationSummary[]> {
  return invoke<ConversationSummary[]>("list_tutor_conversations");
}

export async function createTutorConversation(input: {
  operationId: string;
  scope: StudyScope;
  title: string;
}): Promise<ConversationSummary> {
  return invoke<ConversationSummary>("create_tutor_conversation", { input });
}

export async function createTutorProjectConversation(input: {
  operationId: string;
  projectId: string;
  title: string;
  scope: StudyScope | null;
}): Promise<ConversationSummary> {
  return invoke<ConversationSummary>("create_tutor_project_conversation", { input });
}

export async function renameTutorConversation(input: {
  operationId: string;
  conversationId: string;
  expectedOrganizationRevision: number;
  title: string;
}): Promise<ConversationSummary> {
  return invoke<ConversationSummary>("rename_tutor_conversation", { input });
}

export async function moveTutorConversation(input: {
  operationId: string;
  conversationId: string;
  expectedOrganizationRevision: number;
  projectId: string | null;
  applyProjectDefault: boolean;
}): Promise<ConversationSummary> {
  return invoke<ConversationSummary>("move_tutor_conversation", { input });
}

export async function setTutorConversationArchived(input: {
  operationId: string;
  conversationId: string;
  expectedOrganizationRevision: number;
  archived: boolean;
}): Promise<ConversationSummary> {
  return invoke<ConversationSummary>("set_tutor_conversation_archived", { input });
}

/** Where a figure an answer shows sits on its document's page. */
export interface TutorFigure {
  documentId: string;
  figureId: string;
  title: string;
  pageNumber: number;
  pageWidth: number;
  pageHeight: number;
  bounds: FigureBox;
}

/** A figure a finished answer shows; fails once the learner hides it. */
export async function getTutorFigure(turnId: string, documentId: string, figureId: string): Promise<TutorFigure> {
  return invoke<TutorFigure>("get_tutor_figure", { input: { turnId, documentId, figureId } });
}

export async function getTutorConversation(input: {
  conversationId: string;
  beforeSequence: number | null;
}): Promise<ConversationDetail> {
  return invoke<ConversationDetail>("get_tutor_conversation", { input });
}

export async function deleteTutorConversation(input: {
  operationId: string;
  conversationId: string;
  expectedRevision: number;
  expectedOrganizationRevision: number;
}): Promise<void> {
  return invoke<void>("delete_tutor_conversation", { input });
}

/**
 * Native admission saves a durable turn before provider work. Keep the complete
 * input after an ambiguous IPC result: replaying its operationId returns the
 * original state instead of paying for and appending another response.
 */
export async function sendTutorTurn(input: TutorTurnInput): Promise<TutorTurn> {
  return invoke<TutorTurn>("send_tutor_turn", { input });
}

/** One piece of a reply read aloud. */
export interface SpokenPiece {
  segment: NarrationSegment;
  /** How many pieces the whole reply is read in. */
  pieces: number;
  /** Whether the voice spoke this piece now rather than reusing kept audio. */
  made: boolean;
}

/**
 * Reads one piece of a finished reply aloud in the lesson voice, counted from
 * 0, and keeps the audio. Asking for the next piece while one plays keeps a
 * long reply from waiting on all of it.
 */
export async function speakTutorReply(turnId: string, piece: number): Promise<SpokenPiece> {
  return invoke<SpokenPiece>("speak_tutor_reply", { input: { turnId, piece } });
}

/** A flashcard the flashcard model drafted from a Tutor answer. */
export interface AnswerCardDraft {
  question: string;
  answer: string;
}

/**
 * Asks the flashcard model for a few cards from a finished answer. Nothing is
 * saved; cancel it with `cancelLearningTask(operationId)`.
 */
export async function draftTutorAnswerCards(operationId: string, turnId: string): Promise<AnswerCardDraft[]> {
  const drafted = await invoke<{ cards: AnswerCardDraft[] }>("draft_tutor_answer_cards", { input: { operationId, turnId } });
  return drafted.cards;
}
