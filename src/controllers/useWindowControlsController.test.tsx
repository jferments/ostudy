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
        .mockResolvedValueOnce({ maximized: true, fullScreen: false })
        .mockResolvedValueOnce({ maximized: false, fullScreen: false }),
      maximize: vi.fn(() => maximizing.promise),
    });
    const { result } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        confirmationAttempts: 3,
        confirmationDelayMs: 0,
      }),
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
    expect(result.current.maximized).toBe(false);
  });

  it("waits for asynchronous window-manager state after a native command", async () => {
    const readState = vi
      .fn<() => Promise<NativeWindowControlState>>()
      .mockResolvedValueOnce({ maximized: false, fullScreen: false })
      .mockResolvedValueOnce({ maximized: false, fullScreen: false })
      .mockResolvedValueOnce({ maximized: false, fullScreen: false })
      .mockResolvedValueOnce({ maximized: true, fullScreen: false });
    const adapter = makeAdapter({ readState });
    const { result } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        confirmationAttempts: 3,
        confirmationDelayMs: 0,
      }),
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    await act(async () => result.current.toggleMaximized());

    expect(result.current.maximized).toBe(true);
    expect(readState).toHaveBeenCalledTimes(4);
  });

  it("keeps an obsolete resize read from overwriting confirmed mutation state", async () => {
    const staleResize = deferred<NativeWindowControlState>();
    let listener: (() => void) | undefined;
    const readState = vi
      .fn<() => Promise<NativeWindowControlState>>()
      .mockResolvedValueOnce({ maximized: false, fullScreen: false })
      .mockReturnValueOnce(staleResize.promise)
      .mockResolvedValueOnce({ maximized: true, fullScreen: false });
    const adapter = makeAdapter({
      readState,
      subscribe: vi.fn(async (nextListener: () => void) => {
        listener = nextListener;
        return () => undefined;
      }),
    });
    const { result } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        confirmationAttempts: 2,
        confirmationDelayMs: 0,
      }),
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    act(() => listener?.());
    await waitFor(() => expect(readState).toHaveBeenCalledTimes(2));
    await act(async () => result.current.toggleMaximized());
    expect(result.current.maximized).toBe(true);

    await act(async () => {
      staleResize.resolve({ maximized: false, fullScreen: false });
      await staleResize.promise;
    });
    expect(result.current.maximized).toBe(true);
  });

  it("reports a bounded failure when native state never reaches the request", async () => {
    const onError = vi.fn();
    const adapter = makeAdapter({
      readState: vi.fn().mockResolvedValue({ maximized: false, fullScreen: false }),
    });
    const { result } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        onError,
        confirmationAttempts: 2,
        confirmationDelayMs: 0,
      }),
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    await act(async () => result.current.toggleMaximized());

    expect(onError).toHaveBeenCalledWith(
      expect.stringContaining(
        "Window could not be maximized: native window state did not reach the requested value after 2 reads",
      ),
    );
    expect(result.current.maximized).toBe(false);
    expect(result.current.pendingAction).toBeNull();
  });

  it("recovers from a failed command and reports the native error", async () => {
    const onError = vi.fn();
    const maximize = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(new Error("window manager refused"))
      .mockResolvedValueOnce(undefined);
    const adapter = makeAdapter({
      maximize,
      readState: vi
        .fn<() => Promise<NativeWindowControlState>>()
        .mockResolvedValueOnce({ maximized: false, fullScreen: false })
        .mockResolvedValueOnce({ maximized: true, fullScreen: false }),
    });
    const { result } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        onError,
        confirmationAttempts: 2,
        confirmationDelayMs: 0,
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
      readState: vi
        .fn<() => Promise<NativeWindowControlState>>()
        .mockResolvedValueOnce({ maximized: false, fullScreen: true })
        .mockResolvedValueOnce({ maximized: false, fullScreen: false }),
    });
    const { result } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        confirmationAttempts: 2,
        confirmationDelayMs: 0,
      }),
    );
    await waitFor(() => expect(result.current.fullScreen).toBe(true));

    await act(async () => result.current.toggleFullScreen());

    expect(adapter.setFullScreen).toHaveBeenCalledWith(false);
    expect(result.current.fullScreen).toBe(false);
  });

  it("does not read or publish mutation state after unmount", async () => {
    const maximizing = deferred<void>();
    const readState = vi
      .fn<() => Promise<NativeWindowControlState>>()
      .mockResolvedValue({ maximized: false, fullScreen: false });
    const onError = vi.fn();
    const adapter = makeAdapter({
      readState,
      maximize: vi.fn(() => maximizing.promise),
    });
    const { result, unmount } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        onError,
        confirmationAttempts: 2,
        confirmationDelayMs: 0,
      }),
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    let mutation!: Promise<void>;
    act(() => {
      mutation = result.current.toggleMaximized();
    });
    unmount();
    await act(async () => {
      maximizing.resolve();
      await mutation;
    });

    expect(readState).toHaveBeenCalledOnce();
    expect(onError).not.toHaveBeenCalled();
  });

  it("does not let an obsolete capability-era mutation clear a newer action", async () => {
    const oldMaximize = deferred<void>();
    const currentMaximize = deferred<void>();
    const maximize = vi
      .fn<() => Promise<void>>()
      .mockReturnValueOnce(oldMaximize.promise)
      .mockReturnValueOnce(currentMaximize.promise);
    const readState = vi
      .fn<() => Promise<NativeWindowControlState>>()
      .mockResolvedValueOnce({ maximized: false, fullScreen: false })
      .mockResolvedValueOnce({ maximized: false, fullScreen: false })
      .mockResolvedValueOnce({ maximized: true, fullScreen: false });
    const adapter = makeAdapter({ readState, maximize });
    const unavailable = capability(
      "unavailable",
      "native_mobile_window_controls_not_supported",
    );
    const { result, rerender } = renderHook(
      ({ nativeSnapshot }: { nativeSnapshot: NativePlatformSnapshot }) =>
        useWindowControlsController({
          nativeSnapshot,
          adapter,
          confirmationAttempts: 2,
          confirmationDelayMs: 0,
        }),
      { initialProps: { nativeSnapshot: makeSnapshot() } },
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    let oldMutation!: Promise<void>;
    act(() => {
      oldMutation = result.current.toggleMaximized();
    });
    rerender({ nativeSnapshot: makeSnapshot(unavailable, unavailable) });
    rerender({ nativeSnapshot: makeSnapshot() });
    await waitFor(() => expect(result.current.ready).toBe(true));

    let currentMutation!: Promise<void>;
    act(() => {
      currentMutation = result.current.toggleMaximized();
    });
    expect(result.current.pendingAction).toBe("maximize");

    await act(async () => {
      oldMaximize.resolve();
      await oldMutation;
    });
    expect(result.current.pendingAction).toBe("maximize");
    expect(result.current.maximized).toBe(false);

    await act(async () => {
      currentMaximize.resolve();
      await currentMutation;
    });
    expect(result.current.pendingAction).toBeNull();
    expect(result.current.maximized).toBe(true);
  });

  it("does not let a mutation from a replaced adapter publish or clear state", async () => {
    const oldMaximize = deferred<void>();
    const currentMaximize = deferred<void>();
    const oldAdapter = makeAdapter({
      maximize: vi.fn(() => oldMaximize.promise),
    });
    const currentAdapter = makeAdapter({
      maximize: vi.fn(() => currentMaximize.promise),
      readState: vi
        .fn<() => Promise<NativeWindowControlState>>()
        .mockResolvedValueOnce({ maximized: false, fullScreen: false })
        .mockResolvedValueOnce({ maximized: true, fullScreen: false }),
    });
    const { result, rerender } = renderHook(
      ({ adapter }: { adapter: WindowControlsAdapter }) =>
        useWindowControlsController({
          nativeSnapshot: makeSnapshot(),
          adapter,
          confirmationAttempts: 2,
          confirmationDelayMs: 0,
        }),
      { initialProps: { adapter: oldAdapter } },
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    let oldMutation!: Promise<void>;
    act(() => {
      oldMutation = result.current.toggleMaximized();
    });
    rerender({ adapter: currentAdapter });
    await waitFor(() => expect(result.current.ready).toBe(true));

    let currentMutation!: Promise<void>;
    act(() => {
      currentMutation = result.current.toggleMaximized();
    });
    await act(async () => {
      oldMaximize.resolve();
      await oldMutation;
    });
    expect(result.current.pendingAction).toBe("maximize");

    await act(async () => {
      currentMaximize.resolve();
      await currentMutation;
    });
    expect(result.current.pendingAction).toBeNull();
    expect(result.current.maximized).toBe(true);
    expect(oldAdapter.readState).toHaveBeenCalledOnce();
  });

  it("suppresses a confirmation error that resolves after unmount", async () => {
    const confirmation = deferred<NativeWindowControlState>();
    const onError = vi.fn();
    const readState = vi
      .fn<() => Promise<NativeWindowControlState>>()
      .mockResolvedValueOnce({ maximized: false, fullScreen: false })
      .mockReturnValueOnce(confirmation.promise);
    const adapter = makeAdapter({ readState });
    const { result, unmount } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        onError,
        confirmationAttempts: 2,
        confirmationDelayMs: 0,
      }),
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    let mutation!: Promise<void>;
    act(() => {
      mutation = result.current.toggleMaximized();
    });
    await waitFor(() => expect(readState).toHaveBeenCalledTimes(2));
    unmount();
    await act(async () => {
      confirmation.reject(new Error("obsolete confirmation failure"));
      await mutation;
    });

    expect(onError).not.toHaveBeenCalled();
  });

  it("suppresses an obsolete resize error after a mutation starts", async () => {
    const staleResize = deferred<NativeWindowControlState>();
    const maximizing = deferred<void>();
    const onError = vi.fn();
    let listener: (() => void) | undefined;
    const readState = vi
      .fn<() => Promise<NativeWindowControlState>>()
      .mockResolvedValueOnce({ maximized: false, fullScreen: false })
      .mockReturnValueOnce(staleResize.promise)
      .mockResolvedValueOnce({ maximized: true, fullScreen: false });
    const adapter = makeAdapter({
      readState,
      maximize: vi.fn(() => maximizing.promise),
      subscribe: vi.fn(async (nextListener: () => void) => {
        listener = nextListener;
        return () => undefined;
      }),
    });
    const { result } = renderHook(() =>
      useWindowControlsController({
        nativeSnapshot: makeSnapshot(),
        adapter,
        onError,
        confirmationAttempts: 2,
        confirmationDelayMs: 0,
      }),
    );
    await waitFor(() => expect(result.current.ready).toBe(true));

    act(() => listener?.());
    await waitFor(() => expect(readState).toHaveBeenCalledTimes(2));
    let mutation!: Promise<void>;
    act(() => {
      mutation = result.current.toggleMaximized();
    });
    await act(async () => {
      staleResize.reject(new Error("obsolete resize failure"));
      try {
        await staleResize.promise;
      } catch {
        // The controller's latest-result lane owns this obsolete rejection.
      }
    });
    expect(onError).not.toHaveBeenCalled();

    await act(async () => {
      maximizing.resolve();
      await mutation;
    });
    expect(result.current.maximized).toBe(true);
    expect(onError).not.toHaveBeenCalled();
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
