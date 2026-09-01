import { beforeEach, describe, expect, it, vi } from "vitest";

const windowApi = vi.hoisted(() => ({
  isMaximized: vi.fn(),
  isFullscreen: vi.fn(),
  maximize: vi.fn(),
  unmaximize: vi.fn(),
  setFullscreen: vi.fn(),
  onResized: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowApi,
}));

import { nativeWindowControls } from "./windowControls";

describe("nativeWindowControls", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    windowApi.isMaximized.mockResolvedValue(false);
    windowApi.isFullscreen.mockResolvedValue(false);
    windowApi.maximize.mockResolvedValue(undefined);
    windowApi.unmaximize.mockResolvedValue(undefined);
    windowApi.setFullscreen.mockResolvedValue(undefined);
    windowApi.onResized.mockResolvedValue(() => undefined);
  });

  it("reads native maximize and full-screen state together", async () => {
    windowApi.isMaximized.mockResolvedValue(true);
    windowApi.isFullscreen.mockResolvedValue(true);

    await expect(nativeWindowControls.readState()).resolves.toEqual({
      maximized: true,
      fullScreen: true,
    });
    expect(windowApi.isMaximized).toHaveBeenCalledOnce();
    expect(windowApi.isFullscreen).toHaveBeenCalledOnce();
  });

  it("maps each mutation to the exact native window operation", async () => {
    await nativeWindowControls.maximize();
    await nativeWindowControls.restore();
    await nativeWindowControls.setFullScreen(true);
    await nativeWindowControls.setFullScreen(false);

    expect(windowApi.maximize).toHaveBeenCalledOnce();
    expect(windowApi.unmaximize).toHaveBeenCalledOnce();
    expect(windowApi.setFullscreen.mock.calls).toEqual([[true], [false]]);
  });

  it("reconciles native resize changes and returns the native cleanup", async () => {
    const cleanup = vi.fn();
    let resized: (() => void) | undefined;
    windowApi.onResized.mockImplementation(async (listener: () => void) => {
      resized = listener;
      return cleanup;
    });
    const listener = vi.fn();

    const unsubscribe = await nativeWindowControls.subscribe(listener);
    resized?.();
    unsubscribe();

    expect(listener).toHaveBeenCalledOnce();
    expect(cleanup).toHaveBeenCalledOnce();
  });
});
