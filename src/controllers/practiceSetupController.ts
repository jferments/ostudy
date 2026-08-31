import { getPracticeWorkload, type PracticeWorkloadOverview } from "../ipc/practice";
import { listTags, type TagSummary } from "../ipc/tags";
import { latestResult, RequestLane } from "./requestLane";

export interface PracticeSetupControllerDependencies {
  getWorkload(deckIds: string[], tagIds: string[]): Promise<PracticeWorkloadOverview>;
  getTags(deckIds: string[]): Promise<TagSummary[]>;
}

const defaultDependencies: PracticeSetupControllerDependencies = {
  getWorkload: getPracticeWorkload,
  getTags: listTags,
};

/** Owns the independent latest-read lanes used by the selected-deck setup. */
export class PracticeSetupController {
  readonly #workload = new RequestLane();
  readonly #tags = new RequestLane();

  constructor(
    private readonly dependencies: PracticeSetupControllerDependencies = defaultDependencies,
  ) {}

  workload(
    deckIds: string[],
    tagIds: string[],
  ): Promise<PracticeWorkloadOverview | undefined> {
    return latestResult(this.#workload, () =>
      this.dependencies.getWorkload(deckIds, tagIds),
    );
  }

  tags(deckIds: string[]): Promise<TagSummary[] | undefined> {
    return latestResult(this.#tags, () => this.dependencies.getTags(deckIds));
  }

  invalidateWorkload(): void {
    this.#workload.invalidate();
  }

  invalidate(): void {
    this.#workload.invalidate();
    this.#tags.invalidate();
  }
}
