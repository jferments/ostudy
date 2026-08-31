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
  confirmationAttempts?: number;
  confirmationDelayMs?: number;
}

const INITIAL_STATE: NativeWindowControlState = {
  maximized: false,
  fullScreen: false,
};
const DEFAULT_CONFIRMATION_ATTEMPTS = 40;
const DEFAULT_CONFIRMATION_DELAY_MS = 50;

export function useWindowControlsController(
  options: UseWindowControlsControllerOptions = {},
): WindowControlsController {
  const platformCapabilities = usePlatformCapabilities();
  const nativeSnapshot =
    options.nativeSnapshot === undefined
      ? platformCapabilities.native
      : options.nativeSnapshot;
  const adapter = options.adapter ?? nativeWindowControls;
  const confirmationAttempts =
    Math.max(1, options.confirmationAttempts ?? DEFAULT_CONFIRMATION_ATTEMPTS);
  const confirmationDelayMs =
    Math.max(0, options.confirmationDelayMs ?? DEFAULT_CONFIRMATION_DELAY_MS);
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
  const maximizeRestoreAvailableRef = useRef(maximizeRestoreAvailable);
  const fullScreenAvailableRef = useRef(fullScreenAvailable);
  const mutationEpochRef = useRef(0);
  const mountedRef = useRef(true);
  const onErrorRef = useRef(options.onError);
  supportedRef.current = supported;
  maximizeRestoreAvailableRef.current = maximizeRestoreAvailable;
  fullScreenAvailableRef.current = fullScreenAvailable;
  onErrorRef.current = options.onError;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      mutationEpochRef.current += 1;
      pendingRef.current = false;
    };
  }, []);

  useEffect(() => {
    mutationEpochRef.current += 1;
    pendingRef.current = false;
    setPendingAction(null);
    return () => {
      mutationEpochRef.current += 1;
      pendingRef.current = false;
    };
  }, [adapter, fullScreenAvailable, maximizeRestoreAvailable]);

  const reportError = useCallback((prefix: string, error: unknown) => {
    onErrorRef.current?.(`${prefix}: ${getErrorMessage(error)}`);
  }, []);

  const reconcile = useCallback(async (): Promise<void> => {
    if (!supportedRef.current || pendingRef.current) return;
    try {
      const next = await latestResult(readLane, () => adapter.readState());
      if (
        next &&
        mountedRef.current &&
        supportedRef.current &&
        !pendingRef.current
      ) {
        setState(next);
        setReady(true);
      }
    } catch (error: unknown) {
      if (
        mountedRef.current &&
        supportedRef.current &&
        !pendingRef.current
      ) {
        reportError("Window state could not be refreshed", error);
      }
    }
  }, [adapter, readLane, reportError]);

  const confirmMutationState = useCallback(
    async (
      mutationToken: number,
      actionAvailable: () => boolean,
      matchesExpectedState: (next: NativeWindowControlState) => boolean,
    ): Promise<boolean> => {
      const mutationIsCurrent = () =>
        mountedRef.current &&
        supportedRef.current &&
        actionAvailable() &&
        mutationEpochRef.current === mutationToken;
      let last: NativeWindowControlState | null = null;
      for (let attempt = 0; attempt < confirmationAttempts; attempt += 1) {
        if (!mutationIsCurrent()) return false;
        last = await adapter.readState();
        if (!mutationIsCurrent()) return false;
        if (matchesExpectedState(last)) {
          // Discard every resize read that began before this authoritative
          // post-mutation state. Later resize events remain free to reconcile.
          readLane.invalidate();
          setState(last);
          setReady(true);
          return true;
        }
        if (
          attempt + 1 < confirmationAttempts &&
          confirmationDelayMs > 0
        ) {
          await new Promise<void>((resolve) => {
            window.setTimeout(resolve, confirmationDelayMs);
          });
          if (!mutationIsCurrent()) return false;
        } else if (attempt + 1 < confirmationAttempts) {
          await Promise.resolve();
          if (!mutationIsCurrent()) return false;
        }
      }
      throw new Error(
        `native window state did not reach the requested value after ${confirmationAttempts} reads; ` +
          `last=${JSON.stringify(last)}`,
      );
    },
    [adapter, confirmationAttempts, confirmationDelayMs, readLane],
  );

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
      actionAvailable: () => boolean,
      matchesExpectedState: (next: NativeWindowControlState) => boolean,
      failureMessage: string,
    ): Promise<void> => {
      if (
        pendingRef.current ||
        !mountedRef.current ||
        !supportedRef.current ||
        !actionAvailable()
      ) {
        return;
      }
      const mutationToken = mutationEpochRef.current + 1;
      mutationEpochRef.current = mutationToken;
      pendingRef.current = true;
      setPendingAction(action);
      readLane.invalidate();
      const mutationIsCurrent = () =>
        mountedRef.current &&
        supportedRef.current &&
        actionAvailable() &&
        mutationEpochRef.current === mutationToken;
      try {
        await operation();
        if (!mutationIsCurrent()) return;
        await confirmMutationState(
          mutationToken,
          actionAvailable,
          matchesExpectedState,
        );
      } catch (error: unknown) {
        if (mutationIsCurrent()) reportError(failureMessage, error);
      } finally {
        if (mutationEpochRef.current === mutationToken) {
          pendingRef.current = false;
          if (mountedRef.current) setPendingAction(null);
        }
      }
    },
    [confirmMutationState, readLane, reportError],
  );

  const toggleMaximized = useCallback(async (): Promise<void> => {
    if (!maximizeRestoreAvailable) return;
    if (state.maximized) {
      await runMutation(
        "restore",
        () => adapter.restore(),
        () => maximizeRestoreAvailableRef.current,
        (next) => !next.maximized,
        "Window could not be restored",
      );
    } else {
      await runMutation(
        "maximize",
        () => adapter.maximize(),
        () => maximizeRestoreAvailableRef.current,
        (next) => next.maximized,
        "Window could not be maximized",
      );
    }
  }, [adapter, maximizeRestoreAvailable, runMutation, state.maximized]);

  const toggleFullScreen = useCallback(async (): Promise<void> => {
    if (!fullScreenAvailable) return;
    const enabled = !state.fullScreen;
    await runMutation(
      enabled ? "enter-full-screen" : "exit-full-screen",
      () => adapter.setFullScreen(enabled),
      () => fullScreenAvailableRef.current,
      (next) => next.fullScreen === enabled,
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
