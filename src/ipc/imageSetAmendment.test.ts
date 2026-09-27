import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "./transport";
import { amendImageSet, type AmendImageSetInput } from "./imageSets";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const request: AmendImageSetInput = {
  operationId: "a".repeat(32), documentId: "b".repeat(32),
  expectedRevision: 2, expectedFilterRevision: 3,
  members: [
    { memberId: "d".repeat(32), quarterTurns: 1, inclusion: null },
    { memberId: "c".repeat(32), quarterTurns: 0, inclusion: { left: 0, top: 0, right: 0.5, bottom: 1 } },
  ],
  append: [{ stagedFileId: "e".repeat(32), quarterTurns: 0, inclusion: null }],
};

describe("image-set amendment gateway", () => {
  beforeEach(() => vi.clearAllMocks());

  it("preserves source order, original-coordinate crops, revisions and retry identity", async () => {
    const result = { documentId: request.documentId, revision: 3, filterRevision: 3 };
    vi.mocked(invoke).mockResolvedValue(result);
    expect(await amendImageSet(request)).toEqual(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("amend_image_set", { input: request });
    expect(await amendImageSet(request)).toEqual(result);
    expect(invoke).toHaveBeenNthCalledWith(2, "amend_image_set", { input: request });
  });

  it("does not turn a failed amendment into processing, generation or another write", async () => {
    const failure = { code: "image_set_changed", message: "Reload the source" };
    vi.mocked(invoke).mockRejectedValue(failure);
    await expect(amendImageSet(request)).rejects.toBe(failure);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(request.expectedRevision).toBe(2);
    expect(request.operationId).toBe("a".repeat(32));
  });
});
