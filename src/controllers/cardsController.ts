import {
  getCardNote,
  listTags,
  searchCards,
  type CardNote,
  type CardSearchInput,
  type CardSearchPage,
  type TagSummary,
} from "../ipc/cards";
import { latestResult, RequestLane } from "./requestLane";

export class CardsController {
  readonly #search = new RequestLane();
  readonly #tags = new RequestLane();
  readonly #note = new RequestLane();

  search(input: CardSearchInput): Promise<CardSearchPage | undefined> {
    return latestResult(this.#search, () => searchCards(input));
  }

  tags(deckIds: string[]): Promise<TagSummary[] | undefined> {
    return latestResult(this.#tags, () => listTags(deckIds));
  }

  note(noteId: string): Promise<CardNote | undefined> {
    return latestResult(this.#note, () => getCardNote(noteId));
  }

  invalidateSearch(): void {
    this.#search.invalidate();
  }

  invalidate(): void {
    this.#search.invalidate();
    this.#tags.invalidate();
    this.#note.invalidate();
  }
}
