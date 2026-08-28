import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  NativePlatformSnapshot,
  ServiceCapability,
} from "../ipc/platform";
import type {
  NativeWindowControlState,
  WindowControlsAdapter,
} from "../platform/windowControls";
import { useWindowControlsController } from "./useWindowControlsController";

function capability(
  state: ServiceCapability["state"],
  reasonCode: string | null = null,
): ServiceCapability {
  return { state, reasonCode, canRequestPermission: false };
}

function makeSnapshot(
  maximizeRestore: ServiceCapability = capability("available"),
  fullScreen: ServiceCapability = capability("available"),
): NativePlatformSnapshot {
  const available = capability("available");
  const unavailable = capability("unavailable", "not_configured_for_test");
  return {
    schemaVersion: 2,
    runtimeFamily: "desktop",
    operatingSystem: "linux",
    hardwareBackAvailable: false,
    services: {
      windowMaximizeRestore: maximizeRestore,
      windowFullScreen: fullScreen,
      externalFileImport: available,
      externalFileExport: available,
      secureSecrets: available,
      lifecycleEvents: available,
      localTimeZone: available,
      resourceObservation: available,
      nativePdfRender: available,
      nativePdfTextRescue: available,
      ocr: available,
      camera: unavailable,
      qrScan: unavailable,
      haptics: unavailable,
      systemShare: unavailable,
      localNetwork: available,
      backgroundExecution: unavailable,
    },
  };
}

function makeAdapter(
  overrides: Partial<WindowControlsAdapter> = {},
): WindowControlsAdapter {
  return {
    readState: vi.fn().mockResolvedValue({ maximized: false, fullScreen: false }),
    maximize: vi.fn().mockResolvedValue(undefined),
    restore: vi.fn().mockResolvedValue(undefined),
    setFullScreen: vi.fn().mockResolvedValue(undefined),
    subscribe: vi.fn().mockResolvedValue(() => undefined),
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("useWindowControlsController", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("initializes from native state and reconciles external native changes", async () => {
    let listener: (() => void) | undefined;
    const cleanup = vi.fn();
    const readState = vi
      .fn<() => Promise<NativeWindowControlState>>()
      .mockResolvedValueOnce({ maximized: false, fullScreen: false })
      .mockResolvedValueOnce({ maximized: true, fullScreen: true });
    const adapter = makeAdapter({
      readState,
      subscribe: vi.fn(async (nextListener: () => void) => {
        listener = nextListener;
        return cleanup;
      }),
    });

    const { result, unmount } = renderHook(() =>
      useWindowControlsController({ nativeSnapshot: makeSnapshot(), adapter }),
    );

    await waitFor(() => expect(result.current.ready).toBe(true));
    expect(result.current.maximized).toBe(false);
    expect(result.current.fullScreen).toBe(false);

    act(() => listener?.());
    await waitFor(() => expect(result.current.maximized).toBe(true));
    expect(result.current.fullScreen).toBe(true);

    unmount();
    expect(cleanup).toHaveBeenCalledOnce();
  });

  it("ignores an obsolete native read that finishes after a newer resize read", async () => {
    const first = deferred<NativeWindowControlState>();
    const second = deferred<NativeWindowControlState>();
    let listener: (() => void) | undefined;
    const adapter = makeAdapter({
      readState: vi
        .fn<() => Promise<NativeWindowControlState>>()
        .mockReturnValueOnce(first.promise)
        .mockReturnValueOnce(second.promise),
      subscribe: vi.fn(async (nextListener: () => void) => {
        listener = nextListener;
        return () => undefined;
      }),
    });

    const { result } = renderHook(() =>
      useWindowControlsController({ nativeSnapshot: makeSnapshot(), adapter }),
    );
    await waitFor(() => expect(listener).toBeTypeOf("function"));
    act(() => listener?.());
    await act(async () => {
      second.resolve({ maximized: true, fullScreen: false });
      await second.promise;
    });
    expect(result.current.maximized).toBe(true);

    await act(async () => {
      first.resolve({ maximized: false, fullScreen: true });
      await first.promise;
    });
    expect(result.current.maximized).toBe(true);
    expect(result.current.fullScreen).toBe(false);
  });

  it("serializes mutations and uses the reconciled state for restore", async () => {
    const maximizing = deferred<void>();
    const adapter = makeAdapter({
      readState: vi
        .fn<() => Promise<NativeWindowControlState>>()
        .mockResolvedValueOnce({ maximized: false, fullScreen: false })
        .mockResolvedValue({ maximized: true, fullScreen: false }),
      maximize: vi.fn(() => maximizing.promise),
    });
    const { result } = renderHook(() =>
      useWindowControlsController({ nativeSnapshot: makeSnapshot(), adapter }),
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    let firstMutation!: Promise<void>;
    act(() => {
      firstMutation = result.current.toggleMaximized();
      void result.current.toggleMaximized();
    });
    expect(adapter.maximize).toHaveBeenCalledOnce();
    expect(result.current.pendingAction).toBe("maximize");

    await act(async () => {
      maximizing.resolve();
      await firstMutation;
    });
    expect(result.current.maximized).toBe(true);
    expect(result.current.pendingAction).toBeNull();

    await act(async () => result.current.toggleMaximized());
    expect(adapter.restore).toHaveBeenCalledOnce();
  });

  it("recovers from a failed command and reports the native error", async () => {
    const onError = vi.fn();
    const maximize = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(new Error("window manager refused"))
      .mockResolvedValueOnce(undefined);
    const adapter = makeAdapter({ maximize });
    const { result } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        onError,
      }),
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    await act(async () => result.current.toggleMaximized());
    expect(onError).toHaveBeenCalledWith(
      "Window could not be maximized: window manager refused",
    );
    expect(result.current.pendingAction).toBeNull();

    await act(async () => result.current.toggleMaximized());
    expect(maximize).toHaveBeenCalledTimes(2);
  });

  it("uses native full-screen state to choose enter versus exit", async () => {
    const adapter = makeAdapter({
      readState: vi.fn().mockResolvedValue({ maximized: false, fullScreen: true }),
    });
    const { result } = renderHook(() =>
      useWindowControlsController({ nativeSnapshot: makeSnapshot(), adapter }),
    );
    await waitFor(() => expect(result.current.fullScreen).toBe(true));

    await act(async () => result.current.toggleFullScreen());

    expect(adapter.setFullScreen).toHaveBeenCalledWith(false);
  });

  it("does not initialize or mutate for an unsupported mobile capability snapshot", async () => {
    const unavailable = capability("unavailable", "native_mobile_window_controls_not_supported");
    const adapter = makeAdapter();
    const { result } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(unavailable, unavailable),
        adapter,
      }),
    );

    expect(result.current.maximizeRestoreAvailable).toBe(false);
    expect(result.current.fullScreenAvailable).toBe(false);
    await act(async () => {
      await result.current.toggleMaximized();
      await result.current.toggleFullScreen();
    });
    expect(adapter.readState).not.toHaveBeenCalled();
    expect(adapter.subscribe).not.toHaveBeenCalled();
    expect(adapter.maximize).not.toHaveBeenCalled();
    expect(adapter.setFullScreen).not.toHaveBeenCalled();
  });
});
