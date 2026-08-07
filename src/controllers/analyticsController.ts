import {
  getAnalytics,
  type Analytics,
  type AnalyticsQuery,
} from "../ipc/analytics";
import { latestResult, RequestLane } from "./requestLane";

export class AnalyticsController {
  readonly #analytics = new RequestLane();

  load(input: AnalyticsQuery): Promise<Analytics | undefined> {
    return latestResult(this.#analytics, () => getAnalytics(input));
  }

  invalidate(): void {
    this.#analytics.invalidate();
  }
}
