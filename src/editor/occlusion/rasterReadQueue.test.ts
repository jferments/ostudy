import { describe, expect, it, vi } from "vitest";

import { RasterReadQueue } from "./rasterReadQueue";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

describe("bounded native raster read coordination", () => {
  it("does not enter two native raster operations at once", async () => {
    const queue = new RasterReadQueue();
    const first = deferred<string>();
    const runFirst = vi.fn(() => first.promise);
    const runSecond = vi.fn(async () => "second");
    const pendingFirst = queue.read(() => true, runFirst);
    const pendingSecond = queue.read(() => true, runSecond);
    await Promise.resolve();
    expect(runFirst).toHaveBeenCalledTimes(1);
    expect(runSecond).not.toHaveBeenCalled();
    first.resolve("first");
    expect(await pendingFirst).toBe("first");
    expect(await pendingSecond).toBe("second");
    expect(runSecond).toHaveBeenCalledTimes(1);
  });

  it("discards a superseded queued read before allocating native image work", async () => {
    const queue = new RasterReadQueue();
    const first = deferred<number>();
    let current = true;
    const obsolete = vi.fn(async () => 2);
    const pendingFirst = queue.read(() => true, () => first.promise);
    const pendingObsolete = queue.read(() => current, obsolete);
    current = false;
    first.resolve(1);
    await pendingFirst;
    expect(await pendingObsolete).toBeUndefined();
    expect(obsolete).not.toHaveBeenCalled();
  });

  it("a failed read does not poison a later image request", async () => {
    const queue = new RasterReadQueue();
    const failed = queue.read(() => true, async () => { throw new Error("Image unavailable"); });
    const next = queue.read(() => true, async () => "ready");
    await expect(failed).rejects.toThrow("Image unavailable");
    expect(await next).toBe("ready");
  });
});
