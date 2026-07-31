import { invoke } from "@tauri-apps/api/core";

export type RichDocument = Record<string, unknown>;
export type NoteMode = "basic" | "reverse" | "fill_blank" | "template";
export type CardBrowserStatus = "all" | "due" | "new";
export type CardBrowserSort =
  | "updated_desc"
  | "created_asc"
  | "due_asc"
  | "question_asc";

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
  displayName: string;
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

export interface TemplateCardSpec {
  name: string;
  front: string;
  back: string;
}

export interface CardTemplate {
  id: string;
  name: string;
  description: string;
  fields: string[];
  cards: TemplateCardSpec[];
  isBuiltin: boolean;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface SaveCardTemplateInput {
  templateId?: string;
  revision?: number;
  name: string;
  description: string;
  fields: string[];
  cards: TemplateCardSpec[];
}

export interface NoteEditor {
  noteId: string;
  deckId: string;
  mode: NoteMode;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  tags: string[];
  createReverse: boolean;
  fillBlankText: string | null;
  templateId: string | null;
  fields: Record<string, string>;
  assets: Asset[];
  generatedCardCount: number;
  revision: number;
}

export interface SaveNoteInput {
  noteId?: string;
  revision?: number;
  deckId: string;
  mode: NoteMode;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  tags: string[];
  createReverse: boolean;
  fillBlankText: string | null;
  templateId: string | null;
  fields: Record<string, string>;
  allowDuplicate: boolean;
}

export interface BrowserCard {
  noteId: string;
  deckId: string;
  mode: NoteMode;
  frontText: string;
  backText: string;
  tags: string[];
  generatedCardCount: number;
  dueAt: number | null;
  isNew: boolean;
  hasImages: boolean;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface BrowseCardsInput {
  deckId: string;
  query: string;
  tags: string[];
  status: CardBrowserStatus;
  mode: "all" | NoteMode;
  sort: CardBrowserSort;
  limit: number;
  offset: number;
}

export interface BrowseCardsResult {
  cards: BrowserCard[];
  total: number;
}

export interface DuplicateNote {
  noteId: string;
  deckId: string;
  deckName: string;
  frontText: string;
  backText: string;
}

export interface MoveCopyResult {
  noteId: string;
  destinationDeckId: string;
  copied: boolean;
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
  templateCount?: number;
  tagCount?: number;
}

export interface ImportResult {
  deck: Deck;
  cardCount: number;
  assetCount: number;
  templateCount?: number;
  tagCount?: number;
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

export async function listCardTemplates(): Promise<CardTemplate[]> {
  return invoke<CardTemplate[]>("list_card_templates");
}

export async function saveCardTemplate(
  input: SaveCardTemplateInput,
): Promise<CardTemplate> {
  return invoke<CardTemplate>("save_card_template", { input });
}

export async function deleteCardTemplate(templateId: string): Promise<void> {
  return invoke<void>("delete_card_template", { templateId });
}

export async function saveAdvancedNote(input: SaveNoteInput): Promise<NoteEditor> {
  return invoke<NoteEditor>("save_advanced_note", { input });
}

export async function getNoteForEditing(noteId: string): Promise<NoteEditor> {
  return invoke<NoteEditor>("get_note_for_editing", { noteId });
}

export async function browseCards(input: BrowseCardsInput): Promise<BrowseCardsResult> {
  return invoke<BrowseCardsResult>("browse_cards", { input });
}

export async function findNoteDuplicates(
  deckId: string,
  frontText: string,
  backText: string,
  excludeNoteId?: string,
): Promise<DuplicateNote[]> {
  return invoke<DuplicateNote[]>("find_note_duplicates", {
    deckId,
    frontText,
    backText,
    excludeNoteId,
  });
}

export async function renderGeneratedCard(cardId: string): Promise<Flashcard> {
  return invoke<Flashcard>("render_generated_card", { cardId });
}

export async function moveOrCopyNote(
  noteId: string,
  destinationDeckId: string,
  operation: "move" | "copy",
  allowDuplicate = false,
): Promise<MoveCopyResult> {
  return invoke<MoveCopyResult>("move_or_copy_note", {
    input: { noteId, destinationDeckId, operation, allowDuplicate },
  });
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
  return invoke<ExportResult>("export_deck_v2", {
    input: { deckId, destinationPath },
  });
}

export async function importDeck(sourcePath: string): Promise<ImportResult> {
  try {
    return await invoke<ImportResult>("import_deck_v2", { sourcePath });
  } catch (error: unknown) {
    if (getErrorCode(error) !== "legacy_deck_package") throw error;
    return invoke<ImportResult>("import_deck", { sourcePath });
  }
}

export function createSubmissionId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

export function getErrorCode(error: unknown): string | undefined {
  if (typeof error !== "object" || error === null) return undefined;
  const payload = error as CommandErrorPayload;
  return typeof payload.code === "string" ? payload.code : undefined;
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
