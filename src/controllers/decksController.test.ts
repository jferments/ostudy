import { describe, expect, it, vi } from "vitest";

import { runSequentialDeckMutations } from "./decksController";

describe("runSequentialDeckMutations", () => {
  it("continues independent per-deck mutations and reports exact partial outcomes", async () => {
    const mutate = vi.fn(async (deckId: string) => {
      if (deckId === "b") throw new Error("stale deck revision");
      return `undo-${deckId}`;
    });

    const outcome = await runSequentialDeckMutations(
      ["a", "b", "c", "a"],
      "continue-after-failure",
      mutate,
    );

    expect(outcome.policy).toBe("continue-after-failure");
    expect(outcome.requestedIds).toEqual(["a", "b", "c"]);
    expect(outcome.successful).toEqual([
      { deckId: "a", value: "undo-a" },
      { deckId: "c", value: "undo-c" },
    ]);
    expect(outcome.failed.map(({ deckId }) => deckId)).toEqual(["b"]);
    expect(outcome.unattemptedIds).toEqual([]);
  });

  it("stops shared-revision organization work after the first failure", async () => {
    const mutate = vi.fn(async (deckId: string) => {
      if (deckId === "b") throw new Error("organization changed");
      return deckId;
    });

    const outcome = await runSequentialDeckMutations(
      ["a", "b", "c"],
      "stop-after-failure",
      mutate,
    );

    expect(mutate).toHaveBeenCalledTimes(2);
    expect(outcome.successful.map(({ deckId }) => deckId)).toEqual(["a"]);
    expect(outcome.failed.map(({ deckId }) => deckId)).toEqual(["b"]);
    expect(outcome.unattemptedIds).toEqual(["c"]);
  });
});
