import { invoke } from "@tauri-apps/api/core";

import type { DeletionResult } from "./common";
import type { PreparedExport } from "./externalFiles";

export interface DeckSummary {
  id: string;
  name: string;
  description: string;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableDueCount: number;
  archivedAt: number | null;
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

export interface SetDeckArchivedInput {
  deckId: string;
  revision: number;
  archived: boolean;
}

export interface DeckCardSummaryQuery {
  deckId: string;
  query: string;
  sort: "created_desc" | "practiced_desc" | "practiced_asc";
  limit: number;
  offset: number;
}

export interface DeckCardSummary {
  noteId: string;
  primaryCardId: string;
  questionExcerpt: string;
  answerExcerpt: string;
  generatedCardCount: number;
  imageCount: number;
  reversible: boolean;
  updatedAt: number;
}

export interface DeckCardSummaryPage {
  cards: DeckCardSummary[];
  totalCount: number;
  offset: number;
  hasMore: boolean;
}

export interface PreparedDeckExport {
  preparedExport: PreparedExport;
  cardCount: number;
  assetCount: number;
}

export interface ImportResult {
  deck: DeckSummary;
  cardCount: number;
  assetCount: number;
}

export async function listDecks(): Promise<DeckSummary[]> {
  return invoke<DeckSummary[]>("list_decks");
}

export async function getDeck(deckId: string): Promise<DeckSummary> {
  return invoke<DeckSummary>("get_deck", { deckId });
}

export async function createDeck(input: CreateDeckInput): Promise<DeckSummary> {
  return invoke<DeckSummary>("create_deck", { input });
}

export async function updateDeck(input: UpdateDeckInput): Promise<DeckSummary> {
  return invoke<DeckSummary>("update_deck", { input });
}

export async function setDeckArchived(input: SetDeckArchivedInput): Promise<DeckSummary> {
  return invoke<DeckSummary>("set_deck_archived", { input });
}

export async function deleteDeck(deckId: string): Promise<DeletionResult> {
  return invoke<DeletionResult>("delete_deck", { deckId });
}

export async function listDeckCardSummaries(
  input: DeckCardSummaryQuery,
): Promise<DeckCardSummaryPage> {
  return invoke<DeckCardSummaryPage>("list_deck_card_summaries", { input });
}

export async function prepareDeckExport(
  deckId: string,
  operationId: string,
): Promise<PreparedDeckExport> {
  return invoke<PreparedDeckExport>("prepare_deck_export", {
    input: { deckId, operationId },
  });
}

export async function importDeck(
  stagedFileId: string,
  operationId: string,
): Promise<ImportResult> {
  return invoke<ImportResult>("import_deck", {
    input: { stagedFileId, operationId },
  });
}
