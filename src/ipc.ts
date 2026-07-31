import { invoke } from "@tauri-apps/api/core";

export type RichDocument = Record<string, unknown>;

export interface Deck {
  id: string;
  name: string;
  description: string;
  cardCount: number;
  dueCount: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface CreateDeckInput {
  name: string;
  description?: string;
}

export interface UpdateDeckInput {
  deckId: string;
  name: string;
  description?: string;
  revision: number;
}

export interface Asset {
  id: string;
  originalFilename: string;
  mimeType: string;
  byteSize: number;
  filePath: string;
  createdAt: number;
}

export interface Flashcard {
  id: string;
  noteId: string;
  deckId: string;
  direction: string;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  assets: Asset[];
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface CreateCardInput {
  deckId: string;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
}

export interface UpdateCardInput {
  noteId: string;
  revision: number;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
}

export interface AppSettings {
  theme: "light" | "dark" | "console" | "synthwave";
  uiFont: string;
  cardFont: string;
  uiScale: number;
  cardBaseSize: number;
  reduceMotion: boolean;
  confirmDeletions: boolean;
  practiceShuffle: boolean;
  desiredRetention: number;
}

export interface AppInfo {
  appVersion: string;
  dataDirectory: string;
  databasePath: string;
  imageDirectory: string;
  databaseSchemaVersion: number;
}

export interface PracticeCard {
  card: Flashcard;
  deckName: string;
  dueAt: number;
}

export interface PracticeSession {
  id: string;
  cards: PracticeCard[];
  shuffle: boolean;
  startedAt: number;
}

export interface StartPracticeInput {
  deckIds: string[];
  shuffle: boolean;
}

export type ReviewOutcome =
  | "wrong"
  | "not_confident"
  | "somewhat_confident"
  | "very_confident";

export interface SubmitReviewInput {
  sessionId: string;
  cardId: string;
  submissionId: string;
  outcome: ReviewOutcome;
  responseTimeMs: number;
}

export interface ReviewResult {
  cardId: string;
  rating: 1 | 2 | 3 | 4;
  outcome: ReviewOutcome;
  dueAt: number;
  completed: boolean;
  ratedCount: number;
  correctCount: number;
}

export interface DeckAnalytics {
  deckId: string;
  deckName: string;
  cardCount: number;
  dueCount: number;
  reviewCount: number;
  correctCount: number;
  correctRate: number | null;
  averageResponseTimeMs: number | null;
}

export interface RatingDistribution {
  again: number;
  hard: number;
  good: number;
  easy: number;
}

export interface RecentReview {
  id: string;
  deckName: string;
  cardId: string;
  frontText: string;
  outcome: ReviewOutcome;
  rating: 1 | 2 | 3 | 4;
  responseTimeMs: number;
  reviewedAt: number;
  dueAfter: number;
}

export interface Analytics {
  deckCount: number;
  cardCount: number;
  dueNow: number;
  reviewCount: number;
  correctCount: number;
  correctRate: number | null;
  averageResponseTimeMs: number | null;
  ratingDistribution: RatingDistribution;
  deckStats: DeckAnalytics[];
  recentReviews: RecentReview[];
}

export interface ExportResult {
  destinationPath: string;
  cardCount: number;
  assetCount: number;
}

export interface ImportResult {
  deck: Deck;
  cardCount: number;
  assetCount: number;
}

export interface CommandErrorPayload {
  code?: string;
  message?: string;
}

export async function listDecks(): Promise<Deck[]> {
  return invoke<Deck[]>("list_decks");
}

export async function getDeck(deckId: string): Promise<Deck> {
  return invoke<Deck>("get_deck", { deckId });
}

export async function createDeck(input: CreateDeckInput): Promise<Deck> {
  return invoke<Deck>("create_deck", { input });
}

export async function updateDeck(input: UpdateDeckInput): Promise<Deck> {
  return invoke<Deck>("update_deck", { input });
}

export async function deleteDeck(deckId: string): Promise<void> {
  return invoke<void>("delete_deck", { deckId });
}

export async function listCards(deckId: string): Promise<Flashcard[]> {
  return invoke<Flashcard[]>("list_cards", { deckId });
}

export async function getCard(noteId: string): Promise<Flashcard> {
  return invoke<Flashcard>("get_card", { noteId });
}

export async function createCard(input: CreateCardInput): Promise<Flashcard> {
  return invoke<Flashcard>("create_card", { input });
}

export async function updateCard(input: UpdateCardInput): Promise<Flashcard> {
  return invoke<Flashcard>("update_card", { input });
}

export async function deleteCard(noteId: string): Promise<void> {
  return invoke<void>("delete_card", { noteId });
}

export async function importImage(filePath: string): Promise<Asset> {
  return invoke<Asset>("import_image", { filePath });
}

export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

export async function saveSettings(input: AppSettings): Promise<AppSettings> {
  return invoke<AppSettings>("save_settings", { input });
}

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}

export async function startPractice(
  input: StartPracticeInput,
): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_practice", { input });
}

export async function markPracticeCardViewed(
  sessionId: string,
  cardId: string,
): Promise<void> {
  return invoke<void>("mark_practice_card_viewed", {
    input: { sessionId, cardId },
  });
}

export async function markPracticeCardRevealed(
  sessionId: string,
  cardId: string,
): Promise<void> {
  return invoke<void>("mark_practice_card_revealed", {
    input: { sessionId, cardId },
  });
}

export async function submitPracticeReview(
  input: SubmitReviewInput,
): Promise<ReviewResult> {
  return invoke<ReviewResult>("submit_practice_review", { input });
}

export async function completePracticeSession(
  sessionId: string,
): Promise<void> {
  return invoke<void>("complete_practice_session", { sessionId });
}

export async function getAnalytics(): Promise<Analytics> {
  return invoke<Analytics>("get_analytics");
}

export async function exportDeck(
  deckId: string,
  destinationPath: string,
): Promise<ExportResult> {
  return invoke<ExportResult>("export_deck", {
    input: { deckId, destinationPath },
  });
}

export async function importDeck(sourcePath: string): Promise<ImportResult> {
  return invoke<ImportResult>("import_deck", { sourcePath });
}

export function createSubmissionId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

export function getErrorMessage(error: unknown): string {
  if (typeof error === "string" && error.trim() !== "") {
    return error;
  }

  if (typeof error === "object" && error !== null) {
    const payload = error as CommandErrorPayload;
    if (typeof payload.message === "string" && payload.message.trim() !== "") {
      return payload.message;
    }
  }

  if (error instanceof Error && error.message.trim() !== "") {
    return error.message;
  }

  return "An unexpected error occurred.";
}
