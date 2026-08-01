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

export interface OrganizedCard extends Flashcard {
  tags: string[];
  dueAt: number;
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

export interface CreateOrganizedCardInput extends CreateCardInput {
  tags: string[];
  allowDuplicate: boolean;
}

export interface UpdateOrganizedCardInput extends UpdateCardInput {
  tags: string[];
  allowDuplicate: boolean;
}

export interface TagSummary {
  id: string;
  name: string;
  cardCount: number;
}

export type DueFilter = "all" | "due" | "not_due";
export type CardSort = "relevance" | "updated_desc" | "question_asc" | "due_asc";

export interface CardSearchInput {
  deckId?: string;
  query: string;
  tagIds: string[];
  dueFilter: DueFilter;
  sort: CardSort;
  limit: number;
  offset: number;
}

export interface CardBrowserRow {
  cardId: string;
  noteId: string;
  deckId: string;
  deckName: string;
  frontText: string;
  backText: string;
  tags: string[];
  dueAt: number;
  createdAt: number;
  updatedAt: number;
}

export interface CardSearchPage {
  cards: CardBrowserRow[];
  totalCount: number;
}

export interface DuplicateCheckInput {
  deckId: string;
  frontText: string;
  backText: string;
  excludeNoteId?: string;
}

export interface DuplicateCard {
  noteId: string;
  deckId: string;
  frontText: string;
  backText: string;
}

export interface BulkCardRowInput {
  question: string;
  answer: string;
  tags: string[];
}

export interface BulkCreateCardsInput {
  deckId: string;
  rows: BulkCardRowInput[];
  skipDuplicates: boolean;
}

export interface BulkCreateResult {
  createdCount: number;
  skippedDuplicates: number;
}

export type TransferCardsMode = "move" | "copy";

export interface TransferCardsInput {
  noteIds: string[];
  targetDeckId: string;
  mode: TransferCardsMode;
  skipDuplicates: boolean;
}

export interface TransferCardsResult {
  affectedCount: number;
  skippedDuplicates: number;
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

export async function listTags(deckId?: string): Promise<TagSummary[]> {
  return invoke<TagSummary[]>("list_tags", { input: { deckId } });
}

export async function searchCards(input: CardSearchInput): Promise<CardSearchPage> {
  return invoke<CardSearchPage>("search_cards", { input });
}

export async function getOrganizedCard(noteId: string): Promise<OrganizedCard> {
  return invoke<OrganizedCard>("get_organized_card", { noteId });
}

export async function createOrganizedCard(
  input: CreateOrganizedCardInput,
): Promise<OrganizedCard> {
  return invoke<OrganizedCard>("create_organized_card", { input });
}

export async function updateOrganizedCard(
  input: UpdateOrganizedCardInput,
): Promise<OrganizedCard> {
  return invoke<OrganizedCard>("update_organized_card", { input });
}

export async function findDuplicateCards(
  input: DuplicateCheckInput,
): Promise<DuplicateCard[]> {
  return invoke<DuplicateCard[]>("find_duplicate_cards", { input });
}

export async function bulkCreateCards(
  input: BulkCreateCardsInput,
): Promise<BulkCreateResult> {
  return invoke<BulkCreateResult>("bulk_create_cards", { input });
}

export async function transferCards(
  input: TransferCardsInput,
): Promise<TransferCardsResult> {
  return invoke<TransferCardsResult>("transfer_cards", { input });
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

/** Starts a strictly scheduled session containing only cards already due. */
export async function startPractice(
  input: StartPracticeInput,
): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_practice", { input });
}

/**
 * Starts the user-selected session used by the interface. Each deck contributes
 * due cards first, or all active cards when that deck has nothing due.
 */
export async function startSelectedPractice(
  input: StartPracticeInput,
): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_selected_practice", { input });
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
  return invoke<ExportResult>("export_tagged_deck", {
    input: { deckId, destinationPath },
  });
}

export async function importDeck(sourcePath: string): Promise<ImportResult> {
  return invoke<ImportResult>("import_tagged_deck", { sourcePath });
}

export function createSubmissionId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

export function getErrorCode(error: unknown): string | null {
  if (typeof error === "object" && error !== null) {
    const payload = error as CommandErrorPayload;
    return typeof payload.code === "string" ? payload.code : null;
  }
  return null;
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
