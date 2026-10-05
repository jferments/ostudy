import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DocumentAnnotationChange } from "../../../ipc/documentAnnotations";
import type { Annotation } from "./annotationModel";
import { AnnotationSaveQueue, type SaveState } from "./annotationSaveQueue";

const documentId = "d".repeat(32);

function box(id: string, x = 10): Annotation {
  return {
    id,
    pageNumber: 1,
    createdAt: 1,
    shape: { kind: "rectangle", x, y: 10, width: 20, height: 20, color: "#dc2626", strokeWidth: 3 },
  };
}

function deferred() {
  let resolve: () => void = () => undefined;
  let reject: (error: unknown) => void = () => undefined;
  const promise = new Promise<void>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

describe("annotation saving", () => {
  let states: SaveState[];
  let send: ReturnType<typeof vi.fn<(documentId: string, changes: DocumentAnnotationChange[]) => Promise<unknown>>>;

  beforeEach(() => {
    vi.useFakeTimers();
    states = [];
    send = vi.fn<(documentId: string, changes: DocumentAnnotationChange[]) => Promise<unknown>>().mockResolvedValue({});
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function queue(delayMs = 400) {
    return new AnnotationSaveQueue({ documentId, send, onState: (state) => states.push(state), delayMs });
  }

  it("gathers a burst of edits into one write of each annotation's latest state", async () => {
    const saves = queue();
    saves.record([{ id: "a", before: null, after: box("a") }]);
    saves.record([{ id: "a", before: box("a"), after: box("a", 30) }, { id: "b", before: null, after: box("b") }]);
    saves.record([{ id: "b", before: box("b"), after: null }]);
    expect(states).toEqual([{ state: "pending" }, { state: "pending" }, { state: "pending" }]);
    expect(send).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(400);

    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith(documentId, [
      { op: "put", id: "a", pageNumber: 1, shape: box("a", 30).shape },
      { op: "delete", id: "b" },
    ]);
    expect(states.slice(-2)).toEqual([{ state: "saving" }, { state: "saved" }]);
    expect(saves.hasUnsaved()).toBe(false);
  });

  it("writes in requests of at most 500 changes, one request at a time", async () => {
    const saves = queue();
    saves.record(Array.from({ length: 1_201 }, (_, index) => ({ id: `n${index}`, before: null, after: box(`n${index}`) })));
    await expect(saves.flush()).resolves.toBe(true);
    expect(send.mock.calls.map(([, changes]) => changes.length)).toEqual([500, 500, 201]);
  });

  it("keeps an annotation edited during a write for the next one", async () => {
    const first = deferred();
    send.mockReturnValueOnce(first.promise);
    const saves = queue();
    saves.record([{ id: "a", before: null, after: box("a") }]);
    const flushed = saves.flush();
    saves.record([{ id: "a", before: box("a"), after: box("a", 40) }]);
    first.resolve();
    await expect(flushed).resolves.toBe(true);
    expect(send).toHaveBeenCalledTimes(2);
    expect(send.mock.calls[1]?.[1]).toEqual([{ op: "put", id: "a", pageNumber: 1, shape: box("a", 40).shape }]);
  });

  it("reports a failed write and sends the same states again when retried", async () => {
    send.mockRejectedValueOnce(new Error("The library is busy."));
    const saves = queue();
    saves.record([{ id: "a", before: null, after: box("a") }]);
    await expect(saves.flush()).resolves.toBe(false);
    expect(states.at(-1)).toEqual({ state: "failed", message: "The library is busy." });
    expect(saves.hasUnsaved()).toBe(true);

    await expect(saves.flush()).resolves.toBe(true);
    expect(send.mock.calls[1]?.[1]).toEqual(send.mock.calls[0]?.[1]);
    expect(states.at(-1)).toEqual({ state: "saved" });
  });

  it("shares one write between callers and stops its timer when disposed", async () => {
    const saves = queue();
    saves.record([{ id: "a", before: null, after: box("a") }]);
    const [first, second] = [saves.flush(), saves.flush()];
    await expect(Promise.all([first, second])).resolves.toEqual([true, true]);
    expect(send).toHaveBeenCalledTimes(1);

    saves.record([{ id: "b", before: null, after: box("b") }]);
    saves.dispose();
    await vi.advanceTimersByTimeAsync(1_000);
    expect(send).toHaveBeenCalledTimes(1);
    expect(saves.hasUnsaved()).toBe(true);
  });
});
