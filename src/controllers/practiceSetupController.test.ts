import { describe, expect, it } from "vitest";

import type { PracticeWorkloadOverview } from "../ipc/practice";
import type { TagSummary } from "../ipc/tags";
import { deferred } from "../test/factories";
import { PracticeSetupController } from "./practiceSetupController";

const workload: PracticeWorkloadOverview = {
  newLimit: 20,
  reviewLimit: 200,
  newCompleted: 0,
  reviewsCompleted: 0,
  newRemaining: 20,
  reviewsRemaining: 200,
  studyableDueReviews: 2,
  studyableDueNew: 1,
  scheduledReviews: 2,
  scheduledNew: 1,
  omittedDueReviews: 0,
  omittedDueNew: 0,
  needsMorePractice: 0,
  extraAvailable: 3,
};

describe("PracticeSetupController request lanes", () => {
  it("allows only the latest workload response to win", async () => {
    const first = deferred<PracticeWorkloadOverview>();
    const second = deferred<PracticeWorkloadOverview>();
    let call = 0;
    const controller = new PracticeSetupController({
      getWorkload: async () => (call++ === 0 ? first.promise : second.promise),
      getTags: async () => [],
    });
    const obsolete = controller.workload(["a"], []);
    const current = controller.workload(["a"], ["tag"]);
    second.resolve({ ...workload, scheduledReviews: 9 });
    first.resolve(workload);
    await expect(current).resolves.toEqual({ ...workload, scheduledReviews: 9 });
    await expect(obsolete).resolves.toBeUndefined();
  });

  it("allows only the latest constrained-tag response to win", async () => {
    const first = deferred<TagSummary[]>();
    const second = deferred<TagSummary[]>();
    let call = 0;
    const controller = new PracticeSetupController({
      getWorkload: async () => workload,
      getTags: async () => (call++ === 0 ? first.promise : second.promise),
    });
    const obsolete = controller.tags(["a"]);
    const current = controller.tags(["b"]);
    const latestTags = [
      {
        id: "tag-b",
        name: "Biology",
        sourceNoteCount: 1,
        generatedCardCount: 1,
        studyableDueCount: 1,
      },
    ];
    second.resolve(latestTags);
    first.resolve([]);
    await expect(current).resolves.toEqual(latestTags);
    await expect(obsolete).resolves.toBeUndefined();
  });

  it("keeps tag and workload reads independent and invalidates both on disposal", async () => {
    const tagRead = deferred<TagSummary[]>();
    const workloadRead = deferred<PracticeWorkloadOverview>();
    const controller = new PracticeSetupController({
      getWorkload: async () => workloadRead.promise,
      getTags: async () => tagRead.promise,
    });
    const pendingTags = controller.tags(["a"]);
    const pendingWorkload = controller.workload(["a"], []);
    controller.invalidate();
    tagRead.resolve([]);
    workloadRead.resolve(workload);
    await expect(pendingTags).resolves.toBeUndefined();
    await expect(pendingWorkload).resolves.toBeUndefined();
  });

  it("ignores an obsolete error just like an obsolete success", async () => {
    const first = deferred<PracticeWorkloadOverview>();
    const controller = new PracticeSetupController({
      getWorkload: async () => first.promise,
      getTags: async () => [],
    });
    const obsolete = controller.workload(["a"], []);
    controller.invalidateWorkload();
    first.reject(new Error("obsolete failure"));
    await expect(obsolete).resolves.toBeUndefined();
  });
});
