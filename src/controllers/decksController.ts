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
  prepareDeckExport,
  getDeck,
  importDeck,
  listDeckCardSummaries,
  setDeckArchived,
  updateDeck,
  type CreateDeckInput,
  type DeckCardSummaryPage,
  type DeckCardSummaryQuery,
  type DeckSummary,
  type PreparedDeckExport,
  type ImportResult,
  type SetDeckArchivedInput,
  type UpdateDeckInput,
} from "../ipc/decks";
import {
  importGeneratedFlashcards,
  type FlashcardImportResult,
} from "../ipc/generated-import";
import { latestResult, RequestLane } from "./requestLane";

export type SequentialDeckMutationPolicy = "continue-after-failure" | "stop-after-failure";

export interface SequentialDeckMutationSuccess<T> {
  deckId: string;
  value: T;
}

export interface SequentialDeckMutationFailure {
  deckId: string;
  error: unknown;
}

export interface SequentialDeckMutationOutcome<T> {
  policy: SequentialDeckMutationPolicy;
  requestedIds: string[];
  successful: SequentialDeckMutationSuccess<T>[];
  failed: SequentialDeckMutationFailure[];
  unattemptedIds: string[];
}

/**
 * Deck mutations are deliberately sequential and explicitly non-atomic. Use
 * continue-after-failure only when each deck has an independent revision and
 * transaction. Organization moves share one advancing revision and therefore
 * use stop-after-failure.
 */
export async function runSequentialDeckMutations<T>(
  deckIds: readonly string[],
  policy: SequentialDeckMutationPolicy,
  mutate: (deckId: string) => Promise<T>,
): Promise<SequentialDeckMutationOutcome<T>> {
  const requestedIds = [...new Set(deckIds)];
  const successful: SequentialDeckMutationSuccess<T>[] = [];
  const failed: SequentialDeckMutationFailure[] = [];

  for (let index = 0; index < requestedIds.length; index += 1) {
    const deckId = requestedIds[index];
    try {
      successful.push({ deckId, value: await mutate(deckId) });
    } catch (error: unknown) {
      failed.push({ deckId, error });
      if (policy === "stop-after-failure") {
        return {
          policy,
          requestedIds,
          successful,
          failed,
          unattemptedIds: requestedIds.slice(index + 1),
        };
      }
    }
  }

  return { policy, requestedIds, successful, failed, unattemptedIds: [] };
}

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

  prepareExport(deckId: string, operationId: string): Promise<PreparedDeckExport> {
    return prepareDeckExport(deckId, operationId);
  }

  importStudydeck(stagedFileId: string, operationId: string): Promise<ImportResult> {
    return importDeck(stagedFileId, operationId);
  }

  importGenerated(stagedFileId: string): Promise<FlashcardImportResult> {
    return importGeneratedFlashcards(stagedFileId);
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
