import { getRecentPracticeReviews, type RecentReview } from "../ipc/analytics";
import {
  getPracticeCardWindow,
  getPracticeWorkload,
  type PracticeCardWindow,
  type PracticeCardWindowInput,
  type PracticeWorkloadOverview,
} from "../ipc/practice";
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

  recentReviews(): Promise<RecentReview[] | undefined> {
    return latestResult(this.#recent, getRecentPracticeReviews);
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
