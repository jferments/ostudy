import { beforeEach, describe, expect, it, vi } from "vitest";

import { prepareOcclusionImage, previewOcclusionImage, type PreviewOcclusionInput } from "./occlusion";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

beforeEach(() => { invoke.mockReset(); });

describe("occlusion image gateway", () => {
  it("keeps the staged handle and draft identity unchanged across an import retry", async () => {
    const receipt = { asset: { id: "a".repeat(32) }, width: 10, height: 20, pixelFingerprint: "b".repeat(64) };
    invoke.mockResolvedValue(receipt);
    await expect(prepareOcclusionImage("c".repeat(32), "d".repeat(32))).resolves.toBe(receipt);
    await expect(prepareOcclusionImage("c".repeat(32), "d".repeat(32))).resolves.toBe(receipt);
    const args = ["prepare_occlusion_image", { input: { stagedFileId: "c".repeat(32), draftId: "d".repeat(32) } }];
    expect(invoke.mock.calls).toEqual([args, args]);
  });

  it("returns binary preview bytes without starting another import or loading the original", async () => {
    const input: PreviewOcclusionInput = {
      definitionJson: "{}", presentationKey: "a".repeat(32), assetDraftId: null,
      revealed: false, maximumEdge: 1024,
    };
    const bytes = new ArrayBuffer(8);
    invoke.mockResolvedValue(bytes);
    await expect(previewOcclusionImage(input)).resolves.toBe(bytes);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("preview_occlusion_image", { input });
  });

  it("preserves an image-worker failure for the owning controller to reconcile", async () => {
    const failure = { code: "occlusion_worker_busy", message: "Another image is being prepared." };
    invoke.mockRejectedValue(failure);
    await expect(prepareOcclusionImage("a".repeat(32), "b".repeat(32))).rejects.toBe(failure);
    expect(invoke).toHaveBeenCalledTimes(1);
  });
});
