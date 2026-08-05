import { invoke } from "@tauri-apps/api/core";

export type RichDocument = Record<string, unknown>;
export type BuiltInCardType = "basic" | "cloze";
export type ClozeMode = "one_at_a_time" | "all_together";

export interface Deck {
  id: string;
  name: string;
  description: string;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableDueCount: number;
  createdAt: number;
  updatedAt: number;
  archivedAt: number | null;
  revision: number;
}


export interface DeckCardSummary {
  noteId: string;
  primaryCardId: string;
  questionExcerpt: string;
  answerExcerpt: string;
  generatedCardCount: number;
  imageCount: number;
  updatedAt: number;
}

export interface DeckCardPage {
  cards: DeckCardSummary[];
  totalCount: number;
}

export interface CardTemplateSummary {
  id: string;
  name: string;
  description: string;
  fieldCount: number;
  generatedCardRuleCount: number;
  imageCount: number;
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

export interface AssetDraft {
  id: string;
  purpose: "card" | "template" | "import";
  expiresAt: number;
}

export interface DeletionResult {
  affectedCount: number;
  undoToken: string;
  undoExpiresAt: number;
  subjectLabel: string;
}

export interface UndoDeletionResult {
  restoredCount: number;
  entityKind: "deck" | "cards";
  subjectLabel: string;
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

export interface CreateOrganizedCardInput {
  deckId: string;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  tags: string[];
  allowDuplicate: boolean;
  assetDraftId?: string;
}

export interface UpdateOrganizedCardInput {
  noteId: string;
  deckId: string;
  revision: number;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  tags: string[];
  allowDuplicate: boolean;
  assetDraftId?: string;
}

export interface CardTypeInfo {
  cardType: BuiltInCardType;
  reverseEnabled: boolean;
  clozeSource: string;
  clozeMode: ClozeMode;
  generatedCardCount: number;
}

export interface CreateGeneratedCardInput {
  deckId: string;
  cardType: BuiltInCardType;
  reverseEnabled: boolean;
  clozeSource: string;
  clozeMode: ClozeMode;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  frontText: string;
  backText: string;
  tags: string[];
  allowDuplicate: boolean;
  assetDraftId?: string;
}

export interface UpdateGeneratedCardInput extends CreateGeneratedCardInput {
  noteId: string;
  revision: number;
}

export interface CardTemplateField {
  id: string;
  name: string;
  description: string;
  position: number;
}

export interface CardTemplateRule {
  id: string;
  name: string;
  frontDoc: RichDocument;
  backDoc: RichDocument;
  position: number;
}

export interface CardTemplate {
  id: string;
  name: string;
  description: string;
  fields: CardTemplateField[];
  rules: CardTemplateRule[];
  assets: Asset[];
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface CardTemplateFieldInput {
  id?: string;
  name: string;
  description: string;
}

export interface CardTemplateRuleInput {
  id?: string;
  name: string;
  frontDoc: RichDocument;
  backDoc: RichDocument;
}

export interface CreateCardTemplateInput {
  name: string;
  description: string;
  fields: CardTemplateFieldInput[];
  rules: CardTemplateRuleInput[];
  assetDraftId?: string;
}

export interface UpdateCardTemplateInput extends CreateCardTemplateInput {
  templateId: string;
  revision: number;
}

export interface TemplateValueInput {
  fieldId: string;
  doc: RichDocument;
  text: string;
}

export interface TemplateCardPreview {
  ruleId: string;
  ruleName: string;
  questionDoc: RichDocument;
  answerDoc: RichDocument;
  questionText: string;
  answerText: string;
  assets: Asset[];
}

export interface PreviewTemplateCardsInput {
  templateId: string;
  templateRevision: number;
  values: TemplateValueInput[];
}

export interface GenerateTemplateCardsInput extends PreviewTemplateCardsInput {
  deckId: string;
  tags: string[];
  assetDraftId?: string;
}

export interface GenerateTemplateCardsResult {
  createdCount: number;
  skippedDuplicates: number;
  createdNoteIds: string[];
}

export interface TagSummary {
  id: string;
  name: string;
  sourceNoteCount: number;
}

export type DueFilter = "all" | "due" | "not_due";
export type CardStatusFilter =
  | "all"
  | "active"
  | "suspended"
  | "buried"
  | "needs_more_practice";
export type ReviewedFilter =
  | "all"
  | "never"
  | "last_1_day"
  | "last_7_days"
  | "last_30_days"
  | "older_30_days";
export type CardSort =
  | "relevance"
  | "updated_desc"
  | "question_asc"
  | "due_asc"
  | "last_reviewed_desc";

export interface CardSearchInput {
  deckIds: string[];
  query: string;
  tagIds: string[];
  dueFilter: DueFilter;
  statusFilter: CardStatusFilter;
  reviewedFilter: ReviewedFilter;
  sort: CardSort;
  limit: number;
  offset: number;
}

export interface CardBrowserRow {
  cardId: string;
  noteId: string;
  deckId: string;
  deckName: string;
  cardType: BuiltInCardType;
  cardLabel: string;
  frontText: string;
  backText: string;
  tags: string[];
  dueAt: number;
  lastReviewedAt: number | null;
  reviewCount: number;
  suspendedAt: number | null;
  buriedUntil: number | null;
  needsMorePractice: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface CardSearchPage {
  cards: CardBrowserRow[];
  totalCount: number;
}

export interface SelectionCard {
  cardId: string;
  noteId: string;
  deckId: string;
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

export interface AnalyzeBulkCardsInput {
  deckId: string;
  rows: BulkCardRowInput[];
}

export interface BulkAnalyzeResult {
  uniqueCount: number;
  duplicateCount: number;
  duplicateRowIndices: number[];
}

export interface BulkCreateCardsInput {
  deckId: string;
  rows: BulkCardRowInput[];
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

export type TagChangeMode = "add" | "remove";

export interface BulkTagCardsInput {
  noteIds: string[];
  tags: string[];
  mode: TagChangeMode;
}

export interface BulkActionResult {
  affectedCount: number;
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
  dailyNewLimit: number;
  dailyReviewLimit: number;
}

export interface AppInfo {
  appVersion: string;
  dataDirectory: string;
  databasePath: string;
  imageDirectory: string;
  databaseSchemaVersion: number;
}

export type PracticeMode = "scheduled" | "extra" | "custom";

export interface PracticeCard {
  card: Flashcard;
  deckName: string;
  dueAt: number;
  scheduleState: number;
  needsMorePractice: boolean;
}

export interface PracticeSession {
  id: string;
  cards: PracticeCard[];
  cardCount: number;
  hasMore: boolean;
  shuffle: boolean;
  mode: PracticeMode;
  startedAt: number;
}

export interface PracticeCardWindow {
  startPosition: number;
  cards: PracticeCard[];
  hasMore: boolean;
}

export interface PracticeWorkloadOverview {
  newLimit: number;
  reviewLimit: number;
  newCompleted: number;
  reviewsCompleted: number;
  newRemaining: number;
  reviewsRemaining: number;
  dueReviews: number;
  dueNew: number;
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

export type ReviewOutcome =
  | "wrong"
  | "not_confident"
  | "somewhat_confident"
  | "very_confident";

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

export type ScheduleAction =
  | "suspend"
  | "resume"
  | "bury"
  | "unbury"
  | "reset"
  | "reschedule";

export type RescheduleIntent =
  | { kind: "dueNow" }
  | { kind: "tomorrow" }
  | { kind: "calendarDays"; days: number }
  | { kind: "calendarDate"; date: string };

export interface ScheduleActionInput {
  mutationId: string;
  cardIds: string[];
  action: ScheduleAction;
  reschedule?: RescheduleIntent;
  sessionId?: string;
}

export interface ScheduleActionResult {
  affectedCount: number;
  action: ScheduleAction;
}

export interface ExportResult {
  destinationPath: string;
  generatedCardCount: number;
  assetCount: number;
}

export interface ImportResult {
  deck: Deck;
  generatedCardCount: number;
  assetCount: number;
}

export interface LibraryBackupResult {
  destinationPath: string;
  deckCount: number;
  templateCount: number;
  reviewCount: number;
  assetCount: number;
}

export interface LibraryRestoreResult {
  deckCount: number;
  templateCount: number;
  reviewCount: number;
  assetCount: number;
  restartRequired: boolean;
}

export interface CommandErrorPayload {
  code?: string;
  message?: string;
}

export async function listDecks(): Promise<Deck[]> {
  return invoke<Deck[]>("list_decks");
}

export async function listArchivedDecks(): Promise<Deck[]> {
  return invoke<Deck[]>("list_archived_decks");
}

export async function archiveDeck(deckId: string): Promise<Deck> {
  return invoke<Deck>("archive_deck", { deckId });
}

export async function unarchiveDeck(deckId: string): Promise<Deck> {
  return invoke<Deck>("unarchive_deck", { deckId });
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

export async function deleteDeck(deckId: string): Promise<DeletionResult> {
  return invoke<DeletionResult>("delete_deck", { deckId });
}

export async function listDeckCards(
  deckId: string,
  query = "",
  limit = 50,
  offset = 0,
): Promise<DeckCardPage> {
  return invoke<DeckCardPage>("list_deck_cards", { input: { deckId, query, limit, offset } });
}

export async function deleteCard(noteId: string): Promise<DeletionResult> {
  return invoke<DeletionResult>("delete_card", { noteId });
}

export async function undoDeletion(undoToken: string): Promise<UndoDeletionResult> {
  return invoke<UndoDeletionResult>("undo_deletion", { input: { undoToken } });
}

export async function beginAssetDraft(
  purpose: AssetDraft["purpose"],
  ownerId?: string,
): Promise<AssetDraft> {
  return invoke<AssetDraft>("begin_asset_draft", { input: { purpose, ownerId } });
}

export async function cancelAssetDraft(draftId: string): Promise<void> {
  return invoke<void>("cancel_asset_draft", { draftId });
}

export async function syncAssetDraft(draftId: string, assetIds: string[]): Promise<void> {
  return invoke<void>("sync_asset_draft", { input: { draftId, assetIds } });
}

export async function listTags(deckIds: string[] = []): Promise<TagSummary[]> {
  return invoke<TagSummary[]>("list_tags", { input: { deckIds } });
}

export async function searchCards(input: CardSearchInput): Promise<CardSearchPage> {
  return invoke<CardSearchPage>("search_cards", { input });
}

export async function listMatchingCards(input: CardSearchInput): Promise<SelectionCard[]> {
  return invoke<SelectionCard[]>("list_matching_cards", { input });
}

export async function getOrganizedCard(noteId: string): Promise<OrganizedCard> {
  return invoke<OrganizedCard>("get_organized_card", { noteId });
}

export async function createOrganizedCard(input: CreateOrganizedCardInput): Promise<OrganizedCard> {
  return invoke<OrganizedCard>("create_organized_card", { input });
}

export async function updateOrganizedCard(input: UpdateOrganizedCardInput): Promise<OrganizedCard> {
  return invoke<OrganizedCard>("update_organized_card", { input });
}

export async function getCardTypeInfo(noteId: string): Promise<CardTypeInfo> {
  return invoke<CardTypeInfo>("get_card_type_info", { noteId });
}

export async function createGeneratedCard(input: CreateGeneratedCardInput): Promise<OrganizedCard> {
  return invoke<OrganizedCard>("create_generated_card", { input });
}

export async function updateGeneratedCard(input: UpdateGeneratedCardInput): Promise<OrganizedCard> {
  return invoke<OrganizedCard>("update_generated_card", { input });
}

export async function listCardTemplateSummaries(): Promise<CardTemplateSummary[]> {
  return invoke<CardTemplateSummary[]>("list_card_template_summaries");
}

export async function getCardTemplate(templateId: string): Promise<CardTemplate> {
  return invoke<CardTemplate>("get_card_template", { templateId });
}

export async function createCardTemplate(input: CreateCardTemplateInput): Promise<CardTemplate> {
  return invoke<CardTemplate>("create_card_template", { input });
}

export async function updateCardTemplate(input: UpdateCardTemplateInput): Promise<CardTemplate> {
  return invoke<CardTemplate>("update_card_template", { input });
}

export async function deleteCardTemplate(templateId: string): Promise<void> {
  return invoke<void>("delete_card_template", { templateId });
}

export async function previewTemplateCards(input: PreviewTemplateCardsInput): Promise<TemplateCardPreview[]> {
  return invoke<TemplateCardPreview[]>("preview_template_cards", { input });
}

export async function generateCardsFromTemplate(input: GenerateTemplateCardsInput): Promise<GenerateTemplateCardsResult> {
  return invoke<GenerateTemplateCardsResult>("generate_cards_from_template", { input });
}

export async function findDuplicateCards(input: DuplicateCheckInput): Promise<DuplicateCard[]> {
  return invoke<DuplicateCard[]>("find_duplicate_cards", { input });
}

export async function analyzeBulkCards(input: AnalyzeBulkCardsInput): Promise<BulkAnalyzeResult> {
  return invoke<BulkAnalyzeResult>("analyze_bulk_cards", { input });
}

export async function bulkCreateCards(input: BulkCreateCardsInput): Promise<BulkCreateResult> {
  return invoke<BulkCreateResult>("bulk_create_cards", { input });
}

export async function transferCards(input: TransferCardsInput): Promise<TransferCardsResult> {
  return invoke<TransferCardsResult>("transfer_cards", { input });
}

export async function bulkTagCards(input: BulkTagCardsInput): Promise<BulkActionResult> {
  return invoke<BulkActionResult>("bulk_tag_cards", { input });
}

export async function deleteCards(noteIds: string[]): Promise<DeletionResult> {
  return invoke<DeletionResult>("delete_cards", { input: { noteIds } });
}

export async function importImage(filePath: string, draftId: string): Promise<Asset> {
  return invoke<Asset>("import_image", { input: { filePath, draftId } });
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

export async function getPracticeWorkload(
  deckIds: string[],
  tagIds: string[] = [],
): Promise<PracticeWorkloadOverview> {
  return invoke<PracticeWorkloadOverview>("get_practice_workload", { input: { deckIds, tagIds } });
}

export async function startSelectedPractice(input: StartPracticeInput): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_selected_practice", { input });
}

export async function startCardPractice(input: StartCardPracticeInput): Promise<PracticeSession> {
  return invoke<PracticeSession>("start_card_practice", { input });
}

export async function getPracticeCardWindow(
  sessionId: string,
  startPosition: number,
  limit = 50,
): Promise<PracticeCardWindow> {
  return invoke<PracticeCardWindow>("get_practice_card_window", {
    input: { sessionId, startPosition, limit },
  });
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

export async function applyScheduleAction(input: ScheduleActionInput): Promise<ScheduleActionResult> {
  return invoke<ScheduleActionResult>("apply_schedule_action", { input });
}

export async function exportDeck(deckId: string, destinationPath: string): Promise<ExportResult> {
  return invoke<ExportResult>("export_deck", { input: { deckId, destinationPath } });
}

export async function importDeck(
  sourcePath: string,
  operationId: string,
): Promise<ImportResult> {
  return invoke<ImportResult>("import_deck", {
    input: { sourcePath, operationId },
  });
}

export async function exportLibraryBackup(
  destinationPath: string,
): Promise<LibraryBackupResult> {
  return invoke<LibraryBackupResult>("export_library_backup", {
    input: { destinationPath },
  });
}

export async function prepareLibraryRestore(
  sourcePath: string,
): Promise<LibraryRestoreResult> {
  return invoke<LibraryRestoreResult>("prepare_library_restore", {
    input: { sourcePath },
  });
}

export function createOperationId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

export function createSubmissionId(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

export function createMutationId(): string {
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
