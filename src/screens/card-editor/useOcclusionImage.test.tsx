import { act, renderHook, waitFor } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { newOcclusionDraft, type OcclusionDraftUpdate } from "./occlusionDraft";
import { useOcclusionImport } from "./useOcclusionImage";

const mocks = vi.hoisted(() => ({ pick: vi.fn(), prepare: vi.fn(), flush: vi.fn(), imported: vi.fn() }));
vi.mock("../../ipc/externalFiles", () => ({ selectAndStageExternalImport: mocks.pick }));
vi.mock("../../ipc/occlusion", () => ({ prepareOcclusionImage: mocks.prepare }));
vi.mock("../../platform/assets", () => ({ managedAssetUrl: (id: string) => `blob:${id}` }));
const draftId = "a".repeat(32);
const stagedId = "b".repeat(32);
const prepared = {
  asset: { id: "c".repeat(32), originalFilename: "occlusion-source.png", mimeType: "image/png", byteSize: 200, createdAt: 1 },
  width: 400, height: 300, pixelFingerprint: "d".repeat(64),
};
function useHarness() {
  const [draft, setDraft] = useState(newOcclusionDraft);
  const importing = useOcclusionImport({
    draft, assetDraftId: draftId, disabled: false,
    flushDraft: async () => { await mocks.flush(structuredClone(draft)); },
    onUpdate: (update: OcclusionDraftUpdate) => setDraft(update), onAssetImported: mocks.imported,
  });
  return { draft, importing };
}
beforeEach(() => {
  vi.clearAllMocks();
  mocks.pick.mockReset();
  mocks.prepare.mockReset();
  mocks.flush.mockReset();
  mocks.flush.mockResolvedValue(undefined);
  mocks.pick.mockResolvedValue({ id: stagedId });
  mocks.prepare.mockResolvedValue(prepared);
});

describe("recoverable occlusion image import", () => {
  it("flushes the picker identity and staged handle before their corresponding native operations", async () => {
    const order: string[] = [];
    mocks.flush.mockImplementation(async (draft) => { order.push(draft.pendingImport.stagedFileId ? "flush-stage" : "flush-picker"); });
    mocks.pick.mockImplementation(async () => { order.push("pick"); return { id: stagedId }; });
    mocks.prepare.mockImplementation(async () => { order.push("prepare"); return prepared; });
    const { result } = renderHook(useHarness);
    act(() => result.current.importing.start());
    await waitFor(() => expect(result.current.draft.imageAssetId).toBe(prepared.asset.id));
    expect(order).toEqual(["flush-picker", "pick", "flush-stage", "prepare"]);
    expect(mocks.prepare).toHaveBeenCalledExactlyOnceWith(stagedId, draftId);
    expect(mocks.imported).toHaveBeenCalledExactlyOnceWith(prepared.asset);
    expect(result.current.draft.pendingImport).toBeNull();
  });

  it("retries a lost preparation acknowledgement with the same staged handle and no second picker", async () => {
    mocks.prepare.mockRejectedValueOnce(new Error("Acknowledgement lost"));
    const { result } = renderHook(useHarness);
    act(() => result.current.importing.start());
    await waitFor(() => expect(result.current.importing.error).toBe("Acknowledgement lost"));
    const pending = result.current.draft.pendingImport;
    expect(pending?.stagedFileId).toBe(stagedId);
    act(() => result.current.importing.retry());
    await waitFor(() => expect(result.current.draft.imageAssetId).toBe(prepared.asset.id));
    expect(mocks.pick).toHaveBeenCalledTimes(1);
    expect(mocks.prepare.mock.calls).toEqual([[stagedId, draftId], [stagedId, draftId]]);
    expect(mocks.imported).toHaveBeenCalledTimes(1);
  });

  it("does not open a picker when persisting its identity fails", async () => {
    mocks.flush.mockRejectedValueOnce(new Error("Draft unavailable"));
    const { result } = renderHook(useHarness);
    act(() => result.current.importing.start());
    await waitFor(() => expect(result.current.importing.error).toBe("Draft unavailable"));
    expect(mocks.pick).not.toHaveBeenCalled();
    expect(mocks.prepare).not.toHaveBeenCalled();
    expect(result.current.draft.pendingImport).not.toBeNull();
  });

  it("treats picker cancellation as no mutation rather than an import error", async () => {
    mocks.pick.mockResolvedValueOnce(null);
    const { result } = renderHook(useHarness);
    act(() => result.current.importing.start());
    await waitFor(() => expect(mocks.pick).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(result.current.draft.pendingImport).toBeNull());
    expect(result.current.draft.imageAssetId).toBeNull();
    expect(result.current.importing.error).toBeNull();
    expect(mocks.prepare).not.toHaveBeenCalled();
  });
});
