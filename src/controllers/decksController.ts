import { getCardNote, type CardNote } from "../ipc/cards";
import {
  getDeck,
  listDeckCardSummaries,
  type DeckCardSummaryPage,
  type DeckCardSummaryQuery,
  type DeckSummary,
} from "../ipc/decks";
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

  invalidateCards(): void {
    this.#cards.invalidate();
  }

  invalidate(): void {
    this.#deck.invalidate();
    this.#cards.invalidate();
    this.#note.invalidate();
  }
}
