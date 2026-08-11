import { invoke } from "@tauri-apps/api/core";

import type { Asset } from "./assets";
import type { DeletionResult, RichDocument, UndoDeletionResult } from "./common";

export type BuiltInCardType = "basic" | "cloze";
export type ClozeMode = "one_at_a_time" | "all_together";

interface CardNoteCommon {
  noteId: string;
  deckId: string;
  tags: string[];
  generatedCardCount: number;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export type CardNote =
  | (CardNoteCommon & {
      cardType: "basic";
      frontDoc: RichDocument;
      backDoc: RichDocument;
      frontText: string;
      backText: string;
      reverseEnabled: boolean;
      assets: Asset[];
    })
  | (CardNoteCommon & {
      cardType: "cloze";
      clozeSource: string;
      clozeMode: ClozeMode;
    });

export interface CardPresentation {
  cardId: string;
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
  noteRevision: number;
}

interface CardNoteWriteCommon {
  deckId: string;
  tags: string[];
  allowDuplicate: boolean;
  assetDraftId?: string;
}

export type CreateCardNoteInput =
  | (CardNoteWriteCommon & {
      cardType: "basic";
      reverseEnabled: boolean;
      frontDoc: RichDocument;
      backDoc: RichDocument;
    })
  | (CardNoteWriteCommon & {
      cardType: "cloze";
      clozeSource: string;
      clozeMode: ClozeMode;
    });

export type UpdateCardNoteInput = CreateCardNoteInput & {
  noteId: string;
  revision: number;
};

export interface TagSummary {
  id: string;
  name: string;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableDueCount: number;
}

export type AvailabilityFilter = "all" | "ready_now" | "scheduled_later";
export type CardStatusFilter =
  | "all"
  | "active"
  | "suspended"
  | "buried"
  | "needs_more_practice";
export type ReviewedFilter =
  | "all"
  | "never"
  | "today"
  | "past_7_local_dates"
  | "past_30_local_dates"
  | "before_past_30_local_dates";
export type CardSort = "relevance" | "updated_desc" | "due_asc" | "last_reviewed_desc";

export interface CardSearchInput {
  deckIds: string[];
  query: string;
  tagIds: string[];
  availabilityFilter: AvailabilityFilter;
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

export interface ListTagsInput {
  deckIds: string[];
}

export interface TransferCardsInput {
  noteIds: string[];
  targetDeckId: string;
  mode: "move" | "copy";
  skipDuplicates: boolean;
}

export interface TransferCardsResult {
  affectedCount: number;
  skippedDuplicates: number;
}

export interface BulkTagCardsInput {
  noteIds: string[];
  tags: string[];
  mode: "add" | "remove";
}

export interface BulkActionResult {
  affectedCount: number;
}

export async function getCardNote(noteId: string): Promise<CardNote> {
  return invoke<CardNote>("get_card_note", { noteId });
}

export async function createCardNote(input: CreateCardNoteInput): Promise<CardNote> {
  return invoke<CardNote>("create_card_note", { input });
}

export async function updateCardNote(input: UpdateCardNoteInput): Promise<CardNote> {
  return invoke<CardNote>("update_card_note", { input });
}

export async function deleteCardNote(noteId: string): Promise<DeletionResult> {
  return invoke<DeletionResult>("delete_card_note", { noteId });
}

export async function checkDuplicate(input: DuplicateCheckInput): Promise<DuplicateCard[]> {
  return invoke<DuplicateCard[]>("check_duplicate", { input });
}

export async function listTags(input: ListTagsInput): Promise<TagSummary[]> {
  return invoke<TagSummary[]>("list_tags", { input });
}

export async function searchCards(input: CardSearchInput): Promise<CardSearchPage> {
  return invoke<CardSearchPage>("search_cards", { input });
}

export async function selectMatchingCards(input: CardSearchInput): Promise<SelectionCard[]> {
  return invoke<SelectionCard[]>("select_matching_cards", { input });
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

export async function undoDeletion(undoToken: string): Promise<UndoDeletionResult> {
  return invoke<UndoDeletionResult>("undo_deletion", { undoToken });
}
