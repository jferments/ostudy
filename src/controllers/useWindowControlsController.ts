import { useCallback, useEffect, useRef, useState } from "react";

import { getErrorMessage } from "../ipc/common";
import type { NativePlatformSnapshot } from "../ipc/platform";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import {
  nativeWindowControls,
  type NativeWindowControlState,
  type WindowControlsAdapter,
} from "../platform/windowControls";
import { latestResult, useRequestLane } from "./requestLane";

export type WindowControlAction =
  | "maximize"
  | "restore"
  | "enter-full-screen"
  | "exit-full-screen";

export interface WindowControlsController {
  maximizeRestoreAvailable: boolean;
  fullScreenAvailable: boolean;
  ready: boolean;
  maximized: boolean;
  fullScreen: boolean;
  pendingAction: WindowControlAction | null;
  toggleMaximized(): Promise<void>;
  toggleFullScreen(): Promise<void>;
}

interface UseWindowControlsControllerOptions {
  nativeSnapshot?: NativePlatformSnapshot | null;
  adapter?: WindowControlsAdapter;
  onError?: (message: string) => void;
}

const INITIAL_STATE: NativeWindowControlState = {
  maximized: false,
  fullScreen: false,
};

export function useWindowControlsController(
  options: UseWindowControlsControllerOptions = {},
): WindowControlsController {
  const platformCapabilities = usePlatformCapabilities();
  const nativeSnapshot =
    options.nativeSnapshot === undefined
      ? platformCapabilities.native
      : options.nativeSnapshot;
  const adapter = options.adapter ?? nativeWindowControls;
  const maximizeRestoreAvailable =
    nativeSnapshot?.services.windowMaximizeRestore.state === "available";
  const fullScreenAvailable =
    nativeSnapshot?.services.windowFullScreen.state === "available";
  const supported = maximizeRestoreAvailable || fullScreenAvailable;
  const [state, setState] = useState(INITIAL_STATE);
  const [ready, setReady] = useState(false);
  const [pendingAction, setPendingAction] = useState<WindowControlAction | null>(null);
  const readLane = useRequestLane();
  const pendingRef = useRef(false);
  const supportedRef = useRef(supported);
  const mountedRef = useRef(true);
  const onErrorRef = useRef(options.onError);
  supportedRef.current = supported;
  onErrorRef.current = options.onError;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const reportError = useCallback((prefix: string, error: unknown) => {
    onErrorRef.current?.(`${prefix}: ${getErrorMessage(error)}`);
  }, []);

  const reconcile = useCallback(async (): Promise<void> => {
    if (!supportedRef.current) return;
    try {
      const next = await latestResult(readLane, () => adapter.readState());
      if (next && mountedRef.current && supportedRef.current) {
        setState(next);
        setReady(true);
      }
    } catch (error: unknown) {
      reportError("Window state could not be refreshed", error);
    }
  }, [adapter, readLane, reportError]);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | null = null;

    if (!supported) {
      readLane.invalidate();
      pendingRef.current = false;
      setPendingAction(null);
      setState(INITIAL_STATE);
      setReady(false);
      return;
    }

    setReady(false);
    void adapter
      .subscribe(() => void reconcile())
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unsubscribe = dispose;
        void reconcile();
      })
      .catch((error: unknown) => {
        if (!disposed) {
          reportError("Window state changes could not be observed", error);
          void reconcile();
        }
      });

    return () => {
      disposed = true;
      readLane.invalidate();
      unsubscribe?.();
    };
  }, [adapter, readLane, reconcile, reportError, supported]);

  const runMutation = useCallback(
    async (
      action: WindowControlAction,
      operation: () => Promise<void>,
      failureMessage: string,
    ): Promise<void> => {
      if (pendingRef.current || !supportedRef.current) return;
      pendingRef.current = true;
      setPendingAction(action);
      try {
        try {
          await operation();
        } catch (error: unknown) {
          reportError(failureMessage, error);
          return;
        }
        await reconcile();
      } finally {
        pendingRef.current = false;
        if (mountedRef.current) setPendingAction(null);
      }
    },
    [reconcile, reportError],
  );

  const toggleMaximized = useCallback(async (): Promise<void> => {
    if (!maximizeRestoreAvailable) return;
    if (state.maximized) {
      await runMutation("restore", () => adapter.restore(), "Window could not be restored");
    } else {
      await runMutation("maximize", () => adapter.maximize(), "Window could not be maximized");
    }
  }, [adapter, maximizeRestoreAvailable, runMutation, state.maximized]);

  const toggleFullScreen = useCallback(async (): Promise<void> => {
    if (!fullScreenAvailable) return;
    const enabled = !state.fullScreen;
    await runMutation(
      enabled ? "enter-full-screen" : "exit-full-screen",
      () => adapter.setFullScreen(enabled),
      enabled ? "Full screen could not be entered" : "Full screen could not be exited",
    );
  }, [adapter, fullScreenAvailable, runMutation, state.fullScreen]);

  return {
    maximizeRestoreAvailable,
    fullScreenAvailable,
    ready,
    maximized: state.maximized,
    fullScreen: state.fullScreen,
    pendingAction,
    toggleMaximized,
    toggleFullScreen,
  };
}
