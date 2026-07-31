import { invoke } from "@tauri-apps/api/core";

export type RichDocument = Record<string, unknown>;
export type NoteMode = "basic" | "reverse" | "fill_blank" | "template";
export type CardBrowserStatus =
  | "all"
  | "active"
  | "due"
  | "new"
  | "suspended"
  | "buried"
  | "struggling";
export type CardBrowserSort =
  | "updated_desc"
  | "created_asc"
  | "due_asc"
  | "question_asc";
export type ReviewScore = 0 | 1 | 2 | 3;

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

/** One generated review card returned by the Rust renderer. */
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
  fillBlankText?: string | null;
  templateId?: string | null;
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
  suspended: boolean;
  buriedUntil: number | null;
  struggling: boolean;
  strugglingReason: string | null;
  hasImages: boolean;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface BrowseCardsInput {
  deckId: string;
  query?: string;
  tags?: string[];
  status?: CardBrowserStatus;
  mode?: "all" | NoteMode;
  sort?: CardBrowserSort;
  limit?: number;
  offset?: number;
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

export interface BulkEntryRow {
  question: string;
  answer: string;
}

export interface BulkCreateInput {
  deckId: string;
  entries: BulkEntryRow[];
  tags: string[];
  createReverse: boolean;
  skipDuplicates: boolean;
}

export interface BulkCreateResult {
  createdCount: number;
  skippedDuplicates: string[];
}

export interface EditorDraft {
  draftKey: string;
  deckId: string;
  noteId: string | null;
  payload: unknown;
  updatedAt: number;
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
  newCardsPerDay: number;
  reviewsPerDay: number;
  strugglingCardThreshold: number;
  autoSuspendStrugglingCards: boolean;
  buryRelatedCards: boolean;
}

export interface AppInfo {
  appVersion: string;
  dataDirectory: string;
  databasePath: string;
  imageDirectory: string;
  databaseSchemaVersion: number;
}

export interface StudyCard {
  card: Flashcard;
  deckName: string;
  dueAt: number;
  isNew: boolean;
}

export interface StudySession {
  id: string;
  cards: StudyCard[];
  shuffle: boolean;
  startedAt: number;
  newCount: number;
  reviewCount: number;
  newRemainingToday: number;
  reviewsRemainingToday: number;
}

export interface StartStudyInput {
  deckIds: string[];
  shuffle: boolean;
  dayStartMs: number;
  nextDayStartMs: number;
}

export interface IntervalPreview {
  score: ReviewScore;
  label: string;
  dueAt: number;
  intervalMs: number;
}

export interface SubmitStudyReviewInput {
  sessionId: string;
  cardId: string;
  submissionId: string;
  score: ReviewScore;
  responseTimeMs: number;
  nextDayStartMs: number;
}

export interface StudyReviewResult {
  cardId: string;
  score: ReviewScore;
  label: string;
  dueAt: number;
  intervalMs: number;
  completed: boolean;
  ratedCount: number;
  correctCount: number;
  struggling: boolean;
  autoSuspended: boolean;
}

export interface UndoStudyReviewResult {
  cardId: string;
  position: number;
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
  strugglingCount: number;
}

export interface RatingDistribution {
  wrong: number;
  hard: number;
  ok: number;
  easy: number;
}

export interface RecentDeckPractice {
  deckId: string;
  deckName: string;
  sessionId: string;
  practicedAt: number;
  reviewCount: number;
  ratingDistribution: RatingDistribution;
}

export interface RecentReview {
  id: string;
  deckName: string;
  cardId: string;
  frontText: string;
  score: ReviewScore;
  label: string;
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
  strugglingCount: number;
  suspendedCount: number;
  ratingDistribution: RatingDistribution;
  deckStats: DeckAnalytics[];
  recentDeckPractice: RecentDeckPractice[];
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

/** Legacy basic-card APIs retained for migration compatibility and tests. */
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
    excludeNoteId: excludeNoteId ?? null,
  });
}

export async function renderGeneratedCard(cardId: string): Promise<Flashcard> {
  return invoke<Flashcard>("render_generated_card", { cardId });
}

export async function setNoteSuspended(noteId: string, value: boolean): Promise<void> {
  return invoke<void>("set_note_suspended", { input: { noteId, value } });
}

export async function buryNote(noteId: string, buriedUntil: number | null): Promise<void> {
  return invoke<void>("bury_note", { input: { noteId, buriedUntil } });
}

export async function resetNoteSchedule(noteId: string): Promise<void> {
  return invoke<void>("reset_note_schedule", { noteId });
}

export async function rescheduleNote(noteId: string, dueAt: number): Promise<void> {
  return invoke<void>("reschedule_note", { input: { noteId, dueAt } });
}

export async function moveOrCopyNote(
  noteId: string,
  destinationDeckId: string,
  operation: "move" | "copy",
): Promise<MoveCopyResult> {
  return invoke<MoveCopyResult>("move_or_copy_note", {
    input: { noteId, destinationDeckId, operation },
  });
}

export async function bulkCreateNotes(input: BulkCreateInput): Promise<BulkCreateResult> {
  return invoke<BulkCreateResult>("bulk_create_notes", { input });
}

export async function saveEditorDraft(
  draftKey: string,
  deckId: string,
  noteId: string | null,
  payload: unknown,
): Promise<EditorDraft> {
  return invoke<EditorDraft>("save_editor_draft", {
    input: { draftKey, deckId, noteId, payload },
  });
}

export async function getEditorDraft(draftKey: string): Promise<EditorDraft | null> {
  return invoke<EditorDraft | null>("get_editor_draft", { draftKey });
}

export async function deleteEditorDraft(draftKey: string): Promise<void> {
  return invoke<void>("delete_editor_draft", { draftKey });
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

export async function startStudy(input: StartStudyInput): Promise<StudySession> {
  return invoke<StudySession>("start_study_session", { input });
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

export async function previewReviewIntervals(
  cardId: string,
  reviewedAt = Date.now(),
): Promise<IntervalPreview[]> {
  return invoke<IntervalPreview[]>("preview_review_intervals", {
    input: { cardId, reviewedAt },
  });
}

export async function submitStudyReview(
  input: SubmitStudyReviewInput,
): Promise<StudyReviewResult> {
  return invoke<StudyReviewResult>("submit_study_review", { input });
}

export async function undoLastStudyReview(
  sessionId: string,
): Promise<UndoStudyReviewResult> {
  return invoke<UndoStudyReviewResult>("undo_last_study_review", {
    input: { sessionId },
  });
}

export async function completePracticeSession(sessionId: string): Promise<void> {
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
  if (typeof error === "object" && error !== null) {
    const payload = error as CommandErrorPayload;
    return typeof payload.code === "string" ? payload.code : undefined;
  }
  return undefined;
}

export function getErrorMessage(error: unknown): string {
  if (typeof error === "string" && error.trim() !== "") return error;
  if (typeof error === "object" && error !== null) {
    const payload = error as CommandErrorPayload;
    if (typeof payload.message === "string" && payload.message.trim() !== "") {
      return payload.message;
    }
  }
  if (error instanceof Error && error.message.trim() !== "") return error.message;
  return "An unexpected error occurred.";
}
