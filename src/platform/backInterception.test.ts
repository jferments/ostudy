import { describe, expect, it, vi } from "vitest";

import { BackInterceptionSynchronizer } from "./backInterception";

function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("BackInterceptionSynchronizer", () => {
  it("applies rapid state changes in order and finishes on the latest desired state", async () => {
    const synchronizer = new BackInterceptionSynchronizer();
    const first = deferred();
    const second = deferred();
    const calls: boolean[] = [];
    const apply = vi.fn((enabled: boolean) => {
      calls.push(enabled);
      if (calls.length === 1) return first.promise;
      if (calls.length === 2) return second.promise;
      return Promise.resolve();
    });

    const running = synchronizer.request(true, apply);
    synchronizer.request(false, apply);

    expect(calls).toEqual([true]);
    first.resolve();
    await Promise.resolve();
    expect(calls).toEqual([true, false]);

    synchronizer.request(true, apply);
    second.resolve();
    await running;

    expect(calls).toEqual([true, false, true]);
  });

  it("retries after an apply failure", async () => {
    const synchronizer = new BackInterceptionSynchronizer();
    const apply = vi
      .fn<(enabled: boolean) => Promise<void>>()
      .mockRejectedValueOnce(new Error("native unavailable"))
      .mockResolvedValue(undefined);

    await expect(synchronizer.request(true, apply)).rejects.toThrow("native unavailable");
    await expect(synchronizer.request(true, apply)).resolves.toBeUndefined();
    expect(apply).toHaveBeenCalledTimes(2);
  });
});
