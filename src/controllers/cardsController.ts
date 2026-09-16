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
import type { AuthoringNote } from "../ipc/authoringNotes";
import { getQuestionNote } from "../ipc/questions";
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

  note(noteId: string, cardType: string): Promise<AuthoringNote | undefined> {
    return latestResult(this.#note, () => isInteractiveType(cardType)
      ? getQuestionNote(noteId)
      : getCardNote(noteId));
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

function isInteractiveType(cardType: string): boolean {
  return [
    "single_choice",
    "select_all",
    "true_false",
    "matching",
    "grouping",
    "ordering",
    "image_occlusion",
  ].includes(cardType);
}
