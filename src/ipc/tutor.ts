import { invoke } from "@tauri-apps/api/core";

import type { SourceCitation, StudyScope } from "./knowledge";

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

export interface ConversationSummary {
  id: string;
  title: string;
  scope: StudyScope;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export interface QuizFeedback {
  outcome: "correct" | "partial" | "incorrect" | "unclear";
  explanation: string;
  criteriaFeedback: string[];
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
  provider: string;
  model: string;
  errorCode: string;
  errorMessage: string;
  totalTokens: number | null;
  createdAt: number;
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

export async function getTutorConversation(input: {
  conversationId: string;
  beforeSequence: number | null;
}): Promise<ConversationDetail> {
  return invoke<ConversationDetail>("get_tutor_conversation", { input });
}

export async function deleteTutorConversation(input: {
  conversationId: string;
  expectedRevision: number;
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
