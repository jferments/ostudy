import { getRecentPracticeReviews, type RecentReview } from "../ipc/analytics";
import {
  completePracticeSession,
  getPracticeCardWindow,
  getPracticeWorkload,
  markPracticeCardRevealed,
  markPracticeCardViewed,
  refreshPracticeAfterCardEdit,
  startCardPractice,
  startSelectedPractice,
  submitPracticeReview,
  type PracticeCardWindow,
  type PracticeCardWindowInput,
  type PracticeEditRefresh,
  type PracticeSession,
  type PracticeWorkloadOverview,
  type RefreshPracticeAfterCardEditInput,
  type ReviewPreviewSet,
  type ReviewResult,
  type StartCardPracticeInput,
  type StartPracticeInput,
  type SubmitReviewInput,
} from "../ipc/practice";
import {
  applyScheduleAction,
  type ScheduleActionInput,
  type ScheduleActionResult,
} from "../ipc/scheduling";
import { latestResult, RequestLane } from "./requestLane";

export class PracticeController {
  readonly #workload = new RequestLane();
  readonly #window = new RequestLane();
  readonly #recent = new RequestLane();

  workload(deckIds: string[], tagIds: string[]): Promise<PracticeWorkloadOverview | undefined> {
    return latestResult(this.#workload, () => getPracticeWorkload(deckIds, tagIds));
  }

  window(input: PracticeCardWindowInput): Promise<PracticeCardWindow | undefined> {
    return latestResult(this.#window, () => getPracticeCardWindow(input));
  }

  refreshAfterEdit(input: RefreshPracticeAfterCardEditInput): Promise<PracticeEditRefresh> {
    return refreshPracticeAfterCardEdit(input);
  }

  recentReviews(): Promise<RecentReview[] | undefined> {
    return latestResult(this.#recent, getRecentPracticeReviews);
  }

  startSelected(input: StartPracticeInput): Promise<PracticeSession> {
    return startSelectedPractice(input);
  }

  startCards(input: StartCardPracticeInput): Promise<PracticeSession> {
    return startCardPractice(input);
  }

  markViewed(sessionId: string, cardId: string): Promise<void> {
    return markPracticeCardViewed(sessionId, cardId);
  }

  reveal(sessionId: string, cardId: string): Promise<ReviewPreviewSet> {
    return markPracticeCardRevealed(sessionId, cardId);
  }

  review(input: SubmitReviewInput): Promise<ReviewResult> {
    return submitPracticeReview(input);
  }

  schedule(input: ScheduleActionInput): Promise<ScheduleActionResult> {
    return applyScheduleAction(input);
  }

  complete(sessionId: string): Promise<void> {
    return completePracticeSession(sessionId);
  }

  invalidateWorkload(): void {
    this.#workload.invalidate();
  }

  invalidateWindow(): void {
    this.#window.invalidate();
  }

  invalidate(): void {
    this.#workload.invalidate();
    this.#window.invalidate();
    this.#recent.invalidate();
  }
}
