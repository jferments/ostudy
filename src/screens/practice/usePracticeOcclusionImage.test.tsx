import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../../test/factories";
import { usePracticeOcclusionImage } from "./usePracticeOcclusionImage";

const mocks = vi.hoisted(() => ({ read: vi.fn(), createUrl: vi.fn(), revokeUrl: vi.fn() }));
vi.mock("../../ipc/questionAttempts", () => ({ readPracticeOcclusionImage: mocks.read }));
const target = { sessionId: "1".repeat(32), cardId: "2".repeat(32), instanceId: "3".repeat(32) };
let images: ControlledImage[];
class ControlledImage {
  naturalWidth = 400;
  naturalHeight = 300;
  src = "";
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  constructor() { images.push(this); }
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
beforeEach(() => {
  images = [];
  mocks.read.mockReset();
  mocks.read.mockResolvedValue(new ArrayBuffer(8));
  mocks.createUrl.mockReset();
  mocks.createUrl.mockImplementation(() => `blob:prepared-${mocks.createUrl.mock.calls.length}`);
  mocks.revokeUrl.mockReset();
  vi.stubGlobal("Image", ControlledImage);
  vi.stubGlobal("URL", class extends URL {
    static createObjectURL = mocks.createUrl;
    static revokeObjectURL = mocks.revokeUrl;
  });
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("authorized Practice occlusion image loading", () => {
  it("requests only captured identity/revision and waits for decoded pixels before readiness", async () => {
    const { result, unmount } = renderHook(() => usePracticeOcclusionImage(target, 1, 1024));
    await waitFor(() => expect(mocks.createUrl).toHaveBeenCalledTimes(1));
    expect(mocks.read).toHaveBeenCalledExactlyOnceWith({ target, expectedRevision: 1, maximumEdge: 1024 });
    expect(result.current.ready).toBe(false);
    expect(result.current.url).toBeNull();
    act(() => required(images[0], "prepared image 1").onload?.());
    expect(result.current.ready).toBe(true);
    expect(result.current.url).toBe("blob:prepared-1");
    expect(result.current.size).toEqual({ width: 400, height: 300 });
    unmount();
    expect(mocks.revokeUrl).toHaveBeenCalledExactlyOnceWith("blob:prepared-1");
  });

  it("keeps the visible raster while a sharper version is prepared and releases it only after replacement", async () => {
    const sharper = deferred<ArrayBuffer>();
    mocks.read.mockResolvedValueOnce(new ArrayBuffer(8)).mockImplementationOnce(() => sharper.promise);
    const { result, rerender } = renderHook(({ edge }) => usePracticeOcclusionImage(target, 1, edge), { initialProps: { edge: 1024 } });
    await waitFor(() => expect(mocks.createUrl).toHaveBeenCalledTimes(1));
    act(() => required(images[0], "prepared image 1").onload?.());
    expect(result.current.url).toBe("blob:prepared-1");
    const firstLoadedAt = result.current.loadedAt;
    rerender({ edge: 2048 });
    expect(result.current.url).toBe("blob:prepared-1");
    expect(result.current.ready).toBe(true);
    expect(mocks.revokeUrl).not.toHaveBeenCalled();
    await act(async () => { sharper.resolve(new ArrayBuffer(9)); await sharper.promise; });
    await waitFor(() => expect(mocks.createUrl).toHaveBeenCalledTimes(2));
    expect(result.current.url).toBe("blob:prepared-1");
    act(() => required(images[1], "prepared image 2").onload?.());
    expect(result.current.url).toBe("blob:prepared-2");
    expect(result.current.loadedAt).toBe(firstLoadedAt);
    await waitFor(() => expect(mocks.revokeUrl).toHaveBeenCalledWith("blob:prepared-1"));
    expect(mocks.read.mock.calls.map(([input]) => input.maximumEdge)).toEqual([1024, 2048]);
  });

  it("a late hidden raster cannot replace a newer revealed revision", async () => {
    const hidden = deferred<ArrayBuffer>();
    const revealed = deferred<ArrayBuffer>();
    mocks.read.mockImplementationOnce(() => hidden.promise).mockImplementationOnce(() => revealed.promise);
    const { result, rerender } = renderHook(({ revision }) => usePracticeOcclusionImage(target, revision, 1024), { initialProps: { revision: 1 } });
    await waitFor(() => expect(mocks.read).toHaveBeenCalledTimes(1));
    rerender({ revision: 2 });
    expect(result.current.url).toBeNull();
    await act(async () => { hidden.resolve(new ArrayBuffer(8)); await hidden.promise; });
    await waitFor(() => expect(mocks.read).toHaveBeenCalledTimes(2));
    expect(mocks.createUrl).not.toHaveBeenCalled();
    await act(async () => { revealed.resolve(new ArrayBuffer(9)); await revealed.promise; });
    await waitFor(() => expect(mocks.createUrl).toHaveBeenCalledTimes(1));
    act(() => required(images[1], "prepared image 2").onload?.());
    expect(result.current.ready).toBe(true);
    expect(required(mocks.read.mock.calls[1]?.[0], "raster read call argument").expectedRevision).toBe(2);
  });

  it("retries a refinement failure without hiding the already usable image", async () => {
    mocks.read.mockResolvedValueOnce(new ArrayBuffer(8)).mockRejectedValueOnce(new Error("Source temporarily unavailable"));
    const { result, rerender } = renderHook(({ edge }) => usePracticeOcclusionImage(target, 2, edge), { initialProps: { edge: 1024 } });
    await waitFor(() => expect(mocks.createUrl).toHaveBeenCalledTimes(1));
    act(() => required(images[0], "prepared image 1").onload?.());
    rerender({ edge: 2048 });
    await waitFor(() => expect(result.current.error).toBe("Source temporarily unavailable"));
    expect(result.current.url).toBe("blob:prepared-1");
    expect(result.current.ready).toBe(true);
    act(() => result.current.retry());
    await waitFor(() => expect(mocks.createUrl).toHaveBeenCalledTimes(2));
    act(() => required(images[2], "prepared image 3").onload?.());
    expect(result.current.url).toBe("blob:prepared-2");
    expect(mocks.read.mock.calls.map(([input]) => input.expectedRevision)).toEqual([2, 2, 2]);
  });

  it("rejects invalid decoded dimensions and never exposes the failed URL", async () => {
    const { result, unmount } = renderHook(() => usePracticeOcclusionImage(target, 1, 1024));
    await waitFor(() => expect(mocks.createUrl).toHaveBeenCalledTimes(1));
    required(images[0], "prepared image 1").naturalWidth = 10000;
    act(() => required(images[0], "prepared image 1").onload?.());
    expect(result.current.ready).toBe(false);
    expect(result.current.url).toBeNull();
    expect(result.current.error).toMatch(/invalid dimensions/);
    unmount();
    expect(mocks.revokeUrl).toHaveBeenCalledTimes(1);
  });
});
