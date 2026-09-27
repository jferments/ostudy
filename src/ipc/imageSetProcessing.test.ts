import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "./transport";
import { getImageSet, getImageSetPreview, prepareImageSet, setImageSetFilters } from "./imageSets";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

beforeEach(() => vi.mocked(invoke).mockReset());

describe("native image source gateways", () => {
  it("keeps viewport inspection separate from durable inclusion and preparation", async () => {
    const preview = { revision: 7, width: 256, height: 256, dataUrl: "data:image/png;base64,fixture" };
    vi.mocked(invoke).mockResolvedValue(preview);
    const input = { documentId: "a".repeat(32), memberId: "b".repeat(32), expectedRevision: 7,
      viewport: { left: 0.25, top: 0.25, right: 0.5, bottom: 0.5 } };
    expect(await getImageSetPreview(input)).toEqual(preview);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("get_image_set_preview", { input });
  });

  it("forwards the durable retry identity and both revisions without allocating a replacement", async () => {
    const input = { operationId: "a".repeat(32), documentId: "b".repeat(32), expectedRevision: 3,
      expectedFilterRevision: 9, includeInterface: false, overrides: [{ regionId: "c".repeat(64), action: "include" as const }] };
    vi.mocked(invoke).mockRejectedValueOnce(new Error("acknowledgement interrupted"));
    await expect(setImageSetFilters(input)).rejects.toThrow("acknowledgement interrupted");
    const result = { documentId: input.documentId, revision: 3, filterRevision: 10 };
    vi.mocked(invoke).mockResolvedValueOnce(result);
    expect(await setImageSetFilters(input)).toEqual(result);
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["set_image_set_filters", { input }], ["set_image_set_filters", { input }],
    ]);
  });

  it("makes local preparation explicit and leaves ordinary inspection read-only", async () => {
    vi.mocked(invoke).mockResolvedValue({ prepared: true });
    await getImageSet("source");
    await prepareImageSet("source", true);
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["get_image_set", { input: { documentId: "source" } }],
      ["prepare_image_set", { input: { documentId: "source", retryUnreadable: true } }],
    ]);
  });
});
