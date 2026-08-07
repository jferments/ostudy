import {
  deleteCard,
  getCardNote,
  undoDeletion,
  type CardNote,
} from "../ipc/cards";
import type { DeletionResult, UndoDeletionResult } from "../ipc/common";
import {
  createDeck,
  deleteDeck,
  exportDeck,
  getDeck,
  importDeck,
  listDeckCardSummaries,
  setDeckArchived,
  updateDeck,
  type CreateDeckInput,
  type DeckCardSummaryPage,
  type DeckCardSummaryQuery,
  type DeckSummary,
  type ExportResult,
  type ImportResult,
  type SetDeckArchivedInput,
  type UpdateDeckInput,
} from "../ipc/decks";
import {
  importGeneratedFlashcards,
  type FlashcardImportResult,
} from "../ipc/generated-import";
import { latestResult, RequestLane } from "./requestLane";

export class DecksController {
  readonly #deck = new RequestLane();
  readonly #cards = new RequestLane();
  readonly #note = new RequestLane();

  deck(deckId: string): Promise<DeckSummary | undefined> {
    return latestResult(this.#deck, () => getDeck(deckId));
  }

  cards(input: DeckCardSummaryQuery): Promise<DeckCardSummaryPage | undefined> {
    return latestResult(this.#cards, () => listDeckCardSummaries(input));
  }

  note(noteId: string): Promise<CardNote | undefined> {
    return latestResult(this.#note, () => getCardNote(noteId));
  }

  create(input: CreateDeckInput): Promise<DeckSummary> {
    return createDeck(input);
  }

  update(input: UpdateDeckInput): Promise<DeckSummary> {
    return updateDeck(input);
  }

  setArchived(input: SetDeckArchivedInput): Promise<DeckSummary> {
    return setDeckArchived(input);
  }

  deleteDeck(deckId: string): Promise<DeletionResult> {
    return deleteDeck(deckId);
  }

  deleteNote(noteId: string): Promise<DeletionResult> {
    return deleteCard(noteId);
  }

  undo(undoToken: string): Promise<UndoDeletionResult> {
    return undoDeletion(undoToken);
  }

  export(deckId: string, destinationPath: string): Promise<ExportResult> {
    return exportDeck(deckId, destinationPath);
  }

  importStudydeck(sourcePath: string, operationId: string): Promise<ImportResult> {
    return importDeck(sourcePath, operationId);
  }

  importGenerated(sourcePath: string): Promise<FlashcardImportResult> {
    return importGeneratedFlashcards(sourcePath);
  }

  invalidateCards(): void {
    this.#cards.invalidate();
  }

  invalidate(): void {
    this.#deck.invalidate();
    this.#cards.invalidate();
    this.#note.invalidate();
  }
}
