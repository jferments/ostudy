import {
  getCardNote,
  listMatchingCards,
  listTags,
  searchCards,
  type CardNote,
  type CardSearchInput,
  type CardSearchPage,
  type SelectionCard,
  type TagSummary,
} from "../ipc/cards";
import {
  applyScheduleAction,
  type ScheduleActionInput,
  type ScheduleActionResult,
} from "../ipc/scheduling";
import { latestResult, RequestLane } from "./requestLane";

export class CardsController {
  readonly #search = new RequestLane();
  readonly #tags = new RequestLane();
  readonly #note = new RequestLane();
  readonly #matching = new RequestLane();

  search(input: CardSearchInput): Promise<CardSearchPage | undefined> {
    return latestResult(this.#search, () => searchCards(input));
  }

  tags(deckIds: string[]): Promise<TagSummary[] | undefined> {
    return latestResult(this.#tags, () => listTags(deckIds));
  }

  note(noteId: string): Promise<CardNote | undefined> {
    return latestResult(this.#note, () => getCardNote(noteId));
  }

  matching(input: CardSearchInput): Promise<SelectionCard[] | undefined> {
    return latestResult(this.#matching, () => listMatchingCards(input));
  }

  /** Mutations are never stale-cancelled; the caller reconciles after committed success. */
  schedule(input: ScheduleActionInput): Promise<ScheduleActionResult> {
    return applyScheduleAction(input);
  }

  invalidateSearch(): void {
    this.#search.invalidate();
  }

  invalidate(): void {
    this.#search.invalidate();
    this.#tags.invalidate();
    this.#note.invalidate();
    this.#matching.invalidate();
  }
}
