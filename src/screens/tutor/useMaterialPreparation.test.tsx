import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cancelLearningTask, getLearningTaskProgress, getSemanticPreparation, prepareSemanticMaterial,
  previewSemanticPreparation, type PreparationPreview, type PreparationStatus, type StudyScope,
} from "../../ipc/knowledge";
import { useMaterialPreparation } from "./useMaterialPreparation";

vi.mock("../../ipc/knowledge", () => ({
  cancelLearningTask: vi.fn(), getLearningTaskProgress: vi.fn(), getSemanticPreparation: vi.fn(),
  prepareSemanticMaterial: vi.fn(), previewSemanticPreparation: vi.fn(),
}));
const scope: StudyScope = { kind: "deck", id: "a".repeat(32) };
const preview: PreparationPreview = {
  selection: { scope, includeConversations: false }, consentKey: "b".repeat(64), sourceCount: 3,
  sourceCharacters: 100, imageCount: 0, extractionProvider: "openrouter", extractionModel: "chat",
  embeddingProvider: "openrouter", embeddingModel: "embedding", embeddingDimensions: 768,
};
function status(operationId: string, state: PreparationStatus["state"] = "complete"): PreparationStatus {
  return { scopeKey: "c".repeat(64), operationId, consentKey: preview.consentKey,
    embeddingSpace: "d".repeat(64), state, totalSources: 3, completedSources: 3,
    errorCode: "", errorMessage: "", updatedAt: 1 };
}
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(getSemanticPreparation).mockResolvedValue(null);
  vi.mocked(getLearningTaskProgress).mockResolvedValue(null);
  vi.mocked(previewSemanticPreparation).mockResolvedValue(preview);
  vi.mocked(prepareSemanticMaterial).mockImplementation(async (input) => status(input.operationId));
});

describe("explicit semantic preparation", () => {
  it("reads local status but never previews or uploads on mount", async () => {
    renderHook(() => useMaterialPreparation(scope));
    await waitFor(() => expect(getSemanticPreparation).toHaveBeenCalledTimes(1));
    expect(previewSemanticPreparation).not.toHaveBeenCalled();
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
  });

  it("requires a reviewed preview and binds its exact selection/consent to the write", async () => {
    const { result } = renderHook(() => useMaterialPreparation(scope));
    await act(async () => { await result.current.confirm(); });
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
    await act(async () => { await result.current.review(); });
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
    await act(async () => { await result.current.confirm(); });
    expect(prepareSemanticMaterial).toHaveBeenCalledWith({
      operationId: expect.stringMatching(/^[a-f0-9]{32}$/u), selection: preview.selection, consentKey: preview.consentKey,
    });
    expect(result.current.status?.state).toBe("complete");
    expect(result.current.pending).toBeNull();
  });

  it("reuses the original request after an ambiguous acknowledgement", async () => {
    vi.mocked(prepareSemanticMaterial).mockRejectedValueOnce(new Error("Disconnected"));
    const { result } = renderHook(() => useMaterialPreparation(scope));
    await act(async () => { await result.current.review(); });
    await act(async () => { await result.current.confirm(); });
    const original = vi.mocked(prepareSemanticMaterial).mock.calls[0][0];
    expect(result.current.pending).toEqual(original);
    await act(async () => { await result.current.confirm(); });
    expect(vi.mocked(prepareSemanticMaterial).mock.calls[1][0]).toEqual(original);
    expect(result.current.pending).toBeNull();
  });

  it("keeps an ambiguous receipt when a status read returns no record", async () => {
    vi.mocked(prepareSemanticMaterial).mockRejectedValueOnce(new Error("Disconnected"));
    const { result } = renderHook(() => useMaterialPreparation(scope));
    await act(async () => { await result.current.review(); });
    await act(async () => { await result.current.confirm(); });
    const original = vi.mocked(prepareSemanticMaterial).mock.calls[0][0];
    await act(async () => { await result.current.reload(); });
    expect(result.current.status).toBeNull();
    expect(result.current.pending).toEqual(original);
    expect(result.current.locked).toBe(true);
    await act(async () => { await result.current.confirm(); });
    expect(vi.mocked(prepareSemanticMaterial).mock.calls[1][0]).toEqual(original);
    expect(result.current.pending).toBeNull();
  });

  it("requires new consent when material or providers changed", async () => {
    vi.mocked(prepareSemanticMaterial).mockRejectedValueOnce({ code: "semantic_preparation_changed", message: "Review again" });
    const { result } = renderHook(() => useMaterialPreparation(scope));
    await act(async () => { await result.current.review(); });
    await act(async () => { await result.current.confirm(); });
    expect(result.current.preview).toBeNull();
    expect(result.current.pending).toBeNull();
    await act(async () => { await result.current.confirm(); });
    expect(prepareSemanticMaterial).toHaveBeenCalledTimes(1);
  });

  it("keeps a running receipt and cancels only its operation", async () => {
    vi.mocked(prepareSemanticMaterial).mockImplementation(async (input) => status(input.operationId, "running"));
    const { result } = renderHook(() => useMaterialPreparation(scope));
    await act(async () => { await result.current.review(); });
    await act(async () => { await result.current.confirm(); });
    const original = vi.mocked(prepareSemanticMaterial).mock.calls[0][0];
    await act(async () => { await result.current.cancel(); });
    expect(cancelLearningTask).toHaveBeenCalledWith(original.operationId);
    expect(result.current.locked).toBe(true);
  });

  it("ignores an obsolete preview after the material scope changes", async () => {
    let finish!: (value: PreparationPreview) => void;
    vi.mocked(previewSemanticPreparation).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    const { result, rerender } = renderHook(({ selected }) => useMaterialPreparation(selected), { initialProps: { selected: scope } });
    let reviewing!: Promise<void>;
    act(() => { reviewing = result.current.review(); });
    rerender({ selected: { kind: "deck", id: "e".repeat(32) } });
    await act(async () => { finish(preview); await reviewing; });
    expect(result.current.preview).toBeNull();
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
  });
});


describe("conversation preparation consent", () => {
  it("uses the explicit conversation selection in the preview and confirmed operation", async () => {
    const withConversations = { ...preview, selection: { scope, includeConversations: true }, consentKey: "e".repeat(64) };
    vi.mocked(previewSemanticPreparation).mockResolvedValue(withConversations);
    const { result } = renderHook(() => useMaterialPreparation(scope, true));
    await act(async () => { await result.current.review(); });
    expect(previewSemanticPreparation).toHaveBeenCalledWith(withConversations.selection);
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
    await act(async () => { await result.current.confirm(); });
    expect(prepareSemanticMaterial).toHaveBeenCalledWith(expect.objectContaining({
      selection: withConversations.selection, consentKey: withConversations.consentKey,
    }));
  });

  it("invalidates consent when the conversation-inclusion option changes", async () => {
    const { result, rerender } = renderHook(({ include }) => useMaterialPreparation(scope, include), {
      initialProps: { include: false },
    });
    await act(async () => { await result.current.review(); });
    expect(result.current.preview).toEqual(preview);
    rerender({ include: true });
    await act(async () => { await result.current.confirm(); });
    expect(result.current.preview).toBeNull();
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
  });

  it("ignores a late preview from the old conversation selection", async () => {
    let finish!: (value: PreparationPreview) => void;
    vi.mocked(previewSemanticPreparation).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    const { result, rerender } = renderHook(({ include }) => useMaterialPreparation(scope, include), {
      initialProps: { include: false },
    });
    let reviewing!: Promise<void>;
    act(() => { reviewing = result.current.review(); });
    rerender({ include: true });
    await act(async () => { finish(preview); await reviewing; });
    expect(result.current.preview).toBeNull();
    expect(prepareSemanticMaterial).not.toHaveBeenCalled();
  });
});
