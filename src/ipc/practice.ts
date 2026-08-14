import { invoke } from "@tauri-apps/api/core";

import type { CardPresentation } from "./cards";

export type PracticeMode = "scheduled" | "extra" | "custom";
export type ReviewOutcome =
  | "wrong"
  | "not_confident"
  | "somewhat_confident"
  | "very_confident";

export interface PracticeCard {
  presentation: CardPresentation;
  deckName: string;
  dueAt: number;
  scheduleState: number;
  needsMorePractice: boolean;
}

export interface PracticeCardWindow {
  startPosition: number;
  cards: PracticeCard[];
  hasMore: boolean;
}

export interface PracticeSession {
  id: string;
  cardCount: number;
  firstWindow: PracticeCardWindow;
  shuffle: boolean;
  mode: PracticeMode;
  startedAt: number;
}

export interface ResumedPracticeSession {
  session: PracticeSession;
  resumePosition: number;
  revealed: boolean;
  ratedCount: number;
  correctCount: number;
  completed: boolean;
}

export interface PracticeWorkloadOverview {
  newLimit: number;
  reviewLimit: number;
  newCompleted: number;
  reviewsCompleted: number;
  newRemaining: number;
  reviewsRemaining: number;
  studyableDueReviews: number;
  studyableDueNew: number;
  scheduledReviews: number;
  scheduledNew: number;
  omittedDueReviews: number;
  omittedDueNew: number;
  needsMorePractice: number;
  extraAvailable: number;
}

export interface StartPracticeInput {
  operationId: string;
  deckIds: string[];
  tagIds: string[];
  shuffle: boolean;
  mode: "scheduled" | "extra";
}

export interface StartCardPracticeInput {
  operationId: string;
  cardIds: string[];
  shuffle: boolean;
}

export interface PracticeCardWindowInput {
  sessionId: string;
  startPosition: number;
  limit: number;
}

export interface RefreshPracticeAfterCardEditInput {
  sessionId: string;
  noteId: string;
  startPosition: number;
}

export interface PracticeEditRefresh {
  cardCount: number;
  startPosition: number;
  completed: boolean;
  window: PracticeCardWindow;
}

export interface IntervalPreview {
  outcome: ReviewOutcome;
  dueAt: number;
  intervalMs: number;
}

export interface ReviewPreviewSet {
  scheduleRevision: number;
  previewedAt: number;
  previews: IntervalPreview[];
}

export interface SubmitReviewInput {
  sessionId: string;
  cardId: string;
  submissionId: string;
  outcome: ReviewOutcome;
  responseTimeMs: number;
  scheduleRevision: number;
}

export interface ReviewResult {
  cardId: string;
  rating: 1 | 2 | 3 | 4;
  outcome: ReviewOutcome;
  dueAt: number;
  intervalMs: number;
  completed: boolean;
  ratedCount: number;
  correctCount: number;
}

export async function getPracticeWorkload(
  deckIds: string[],
  tagIds: string[] = [],
): Promise<PracticeWorkloadOverview> {
  return invoke<PracticeWorkloadOverview>("get_practice_workload", {
    input: { deckIds, tagIds },
  });
}

export async function startSelectedPractice(input: StartPracticeInput): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_selected_practice", { input });
}

export async function startCardPractice(input: StartCardPracticeInput): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_card_practice", { input });
}

export async function resumePracticeSession(sessionId: string): Promise<ResumedPracticeSession> {
  return invoke<ResumedPracticeSession>("resume_practice_session", {
    input: { sessionId },
  });
}

export async function getPracticeCardWindow(
  input: PracticeCardWindowInput,
): Promise<PracticeCardWindow> {
  return invoke<PracticeCardWindow>("get_practice_card_window", { input });
}

export async function refreshPracticeAfterCardEdit(
  input: RefreshPracticeAfterCardEditInput,
): Promise<PracticeEditRefresh> {
  return invoke<PracticeEditRefresh>("refresh_practice_after_card_edit", { input });
}

export async function markPracticeCardViewed(sessionId: string, cardId: string): Promise<void> {
  return invoke<void>("mark_practice_card_viewed", { input: { sessionId, cardId } });
}

export async function markPracticeCardRevealed(
  sessionId: string,
  cardId: string,
): Promise<ReviewPreviewSet> {
  return invoke<ReviewPreviewSet>("mark_practice_card_revealed", {
    input: { sessionId, cardId },
  });
}

export async function submitPracticeReview(input: SubmitReviewInput): Promise<ReviewResult> {
  return invoke<ReviewResult>("submit_practice_review", { input });
}

export async function completePracticeSession(sessionId: string): Promise<void> {
  return invoke<void>("complete_practice_session", { sessionId });
}
