import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../../test/factories";
import { newOcclusionDraft, type OcclusionEditorDraft } from "./occlusionDraft";
import { useOcclusionPreview } from "./useOcclusionPreview";

const mocks = vi.hoisted(() => ({ render: vi.fn(), createUrl: vi.fn(), revokeUrl: vi.fn() }));
vi.mock("../../ipc/occlusion", () => ({ previewOcclusionImage: mocks.render }));
const field = (text: string) => ({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] });
function draft(): OcclusionEditorDraft {
  return {
    ...newOcclusionDraft(), imageAssetId: "a".repeat(32), selectedQuestionId: "c".repeat(32),
    masks: [{ id: "b".repeat(32), rect: { x: 0.1, y: 0.2, width: 0.2, height: 0.2 } }],
    questions: ["c", "d"].map((letter) => ({
      id: letter.repeat(32), prompt: field("Identify"), answer: field(letter), explanation: null,
      maskIds: ["b".repeat(32)], mode: "hide_all" as const,
    })),
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((yes) => { resolve = yes; });
  return { resolve, promise };
}
beforeEach(() => {
  mocks.render.mockReset().mockResolvedValue(new ArrayBuffer(8));
  mocks.createUrl.mockReset().mockImplementation(() => `blob:preview-${mocks.createUrl.mock.calls.length}`);
  mocks.revokeUrl.mockReset();
  vi.stubGlobal("URL", class extends URL {
    static createObjectURL = mocks.createUrl;
    static revokeObjectURL = mocks.revokeUrl;
  });
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

describe("native image authoring preview lifecycle", () => {
  it("does not render incomplete or closed drafts and revokes completed preview URLs", async () => {
    const { result, rerender, unmount } = renderHook(({ value, enabled }) => useOcclusionPreview(value, null, enabled, false), {
      initialProps: { value: newOcclusionDraft(), enabled: true },
    });
    expect(result.current.ready).toBe(false);
    rerender({ value: draft(), enabled: false });
    await act(async () => { await Promise.resolve(); });
    expect(mocks.render).not.toHaveBeenCalled();
    rerender({ value: draft(), enabled: true });
    await waitFor(() => expect(result.current.url).toBe("blob:preview-1"));
    expect(required(mocks.render.mock.calls[0]?.[0], "preview render input")).toMatchObject({ presentationKey: "c".repeat(32), maximumEdge: 1600, revealed: false });
    unmount();
    expect(mocks.revokeUrl).toHaveBeenCalledExactlyOnceWith("blob:preview-1");
  });

  it("changing reveal phase immediately hides the previous raster until its new render arrives", async () => {
    const next = deferred<ArrayBuffer>();
    const value = draft();
    const { result, rerender } = renderHook(({ revealed }) => useOcclusionPreview(value, null, true, revealed), {
      initialProps: { revealed: false },
    });
    await waitFor(() => expect(result.current.url).toBe("blob:preview-1"));
    mocks.render.mockImplementationOnce(() => next.promise);
    rerender({ revealed: true });
    expect(result.current.url).toBeNull();
    expect(mocks.revokeUrl).toHaveBeenCalledWith("blob:preview-1");
    await act(async () => { next.resolve(new ArrayBuffer(8)); });
    await waitFor(() => expect(result.current.url).toBe("blob:preview-2"));
    expect(required(mocks.render.mock.calls[1]?.[0], "preview render input").revealed).toBe(true);
  });

  it("serializes rapid target changes and skips an obsolete queued preview", async () => {
    const first = deferred<ArrayBuffer>();
    mocks.render.mockImplementationOnce(() => first.promise);
    const original = draft();
    const { result, rerender } = renderHook(({ value, revealed }) => useOcclusionPreview(value, null, true, revealed), {
      initialProps: { value: original, revealed: false },
    });
    await waitFor(() => expect(mocks.render).toHaveBeenCalledTimes(1));
    rerender({ value: { ...original, selectedQuestionId: "d".repeat(32) }, revealed: false });
    rerender({ value: original, revealed: true });
    expect(result.current.url).toBeNull();
    expect(mocks.render).toHaveBeenCalledTimes(1);
    await act(async () => { first.resolve(new ArrayBuffer(8)); });
    await waitFor(() => expect(result.current.url).toBe("blob:preview-1"));
    expect(mocks.render).toHaveBeenCalledTimes(2);
    expect(required(mocks.render.mock.calls[1]?.[0], "preview render input")).toMatchObject({ presentationKey: "c".repeat(32), revealed: true });
    expect(mocks.createUrl).toHaveBeenCalledTimes(1);
  });

  it("a failed render can retry without rewriting the authored question", async () => {
    mocks.render.mockRejectedValueOnce(new Error("Image temporarily busy"));
    const value = draft();
    const before = structuredClone(value);
    const { result } = renderHook(() => useOcclusionPreview(value, "e".repeat(32), true, false));
    await waitFor(() => expect(result.current.error).toBe("Image temporarily busy"));
    act(() => result.current.retry());
    await waitFor(() => expect(result.current.url).toBe("blob:preview-1"));
    expect(required(mocks.render.mock.calls[1]?.[0], "preview render input")).toEqual(required(mocks.render.mock.calls[0]?.[0], "preview render input"));
    expect(value).toEqual(before);
  });
});
