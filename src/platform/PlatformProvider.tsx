import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

import { reportFrontendFailure } from "../ipc/diagnostics";
import {
  listenToAndroidBack,
  listenToAndroidInsets,
  listenToAndroidNetwork,
  setPlatformBackInterception,
} from "../ipc/platform";
import { BackInterceptionSynchronizer } from "./backInterception";
import {
  dispatchPlatformBack,
  subscribePlatformBackStack,
} from "./backNavigation";
import {
  getPlatformCapabilities,
  initializePlatformCapabilities,
  subscribePlatformCapabilities,
  type PlatformCapabilities,
  updateAndroidInsets,
  updateAndroidNetwork,
} from "./capabilities";

const PlatformContext = createContext<PlatformCapabilities>(getPlatformCapabilities());

export function PlatformProvider({ children }: { children: ReactNode }) {
  const [snapshot, setSnapshot] = useState(getPlatformCapabilities);
  const backInterceptionRef = useRef(new BackInterceptionSynchronizer());
  const listenerOwner = useRef<object | null>(null);

  const applyBackInterception = useCallback(async (enabled: boolean): Promise<void> => {
    const owner = listenerOwner.current;
    await setPlatformBackInterception(enabled);
    if (owner !== null && listenerOwner.current === owner) {
      document.documentElement.dataset.androidBackInterception = enabled ? "true" : "false";
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    const unsubscribe = subscribePlatformCapabilities(() => {
      if (!disposed) setSnapshot(getPlatformCapabilities());
    });
    void initializePlatformCapabilities().catch((error: unknown) => {
      if (!disposed) reportFrontendFailure("platform_initialization_failed", error);
    });
    return () => {
      disposed = true;
      unsubscribe();
    };
  }, []);

  useEffect(() => {
    const disposers: Array<() => void> = [];
    let disposed = false;
    const owner = {};
    listenerOwner.current = owner;
    const synchronizer = backInterceptionRef.current;
    const root = document.documentElement;
    root.dataset.androidBackListener = "pending";

    function isAndroid(): boolean {
      return getPlatformCapabilities().native?.operatingSystem === "android";
    }

    function disposeListener(dispose: () => void): void {
      try {
        dispose();
      } catch (error: unknown) {
        reportFrontendFailure("platform_listener_cleanup_failed", error);
      }
    }

    function requestBackInterception(enabled: boolean): void {
      if (disposed) return;
      void synchronizer.request(enabled, applyBackInterception).catch((error: unknown) => {
        if (!disposed && isAndroid()) {
          reportFrontendFailure("android_back_interception_failed", error);
        }
      });
    }

    // Native interception must never be enabled before the plugin event channel
    // exists. Otherwise Kotlin can correctly consume a physical Back gesture
    // while no JavaScript listener is registered to dispatch the shared stack.
    void listenToAndroidBack(() => {
      if (disposed) return;
      const consumed = dispatchPlatformBack();
      if (!consumed) {
        // A handler may have disappeared between native interception and event
        // delivery. Disable interception so the next Back action belongs to
        // Android's predictive back-to-home implementation.
        requestBackInterception(false);
      }
    })
      .then((dispose) => {
        if (disposed) {
          disposeListener(dispose);
          return;
        }
        disposers.push(dispose);
        root.dataset.androidBackListener = "ready";
        const backStack = subscribePlatformBackStack((count) => {
          requestBackInterception(count > 0);
        });
        disposers.push(backStack);
      })
      .catch((error: unknown) => {
        if (!disposed) {
          root.dataset.androidBackListener = "error";
          if (isAndroid()) reportFrontendFailure("android_back_listener_failed", error);
        }
      });

    void listenToAndroidInsets((insets) => {
      if (disposed) return;
      updateAndroidInsets(insets);
      root.style.setProperty("--android-safe-top", `${Math.max(0, insets.top)}px`);
      root.style.setProperty("--android-safe-right", `${Math.max(0, insets.right)}px`);
      root.style.setProperty("--android-safe-bottom", `${Math.max(0, insets.bottom)}px`);
      root.style.setProperty("--android-safe-left", `${Math.max(0, insets.left)}px`);
      root.style.setProperty("--android-ime-bottom", `${Math.max(0, insets.imeBottom)}px`);
      root.dataset.androidImeVisible = insets.imeVisible ? "true" : "false";
    })
      .then((dispose) => {
        if (disposed) disposeListener(dispose);
        else disposers.push(dispose);
      })
      .catch((error: unknown) => {
        if (!disposed && isAndroid()) reportFrontendFailure("android_insets_listener_failed", error);
      });

    void listenToAndroidNetwork((network) => {
      if (!disposed) updateAndroidNetwork(network);
    })
      .then((dispose) => {
        if (disposed) disposeListener(dispose);
        else disposers.push(dispose);
      })
      .catch((error: unknown) => {
        if (!disposed && isAndroid()) reportFrontendFailure("android_network_listener_failed", error);
      });

    return () => {
      disposed = true;
      if (listenerOwner.current === owner) listenerOwner.current = null;
      for (const dispose of disposers.splice(0)) disposeListener(dispose);
      void synchronizer.request(false, applyBackInterception).catch((error: unknown) => {
        if (isAndroid()) reportFrontendFailure("android_back_cleanup_failed", error);
      });
      delete root.dataset.androidBackListener;
      delete root.dataset.androidBackInterception;
      delete root.dataset.androidImeVisible;
    };
  }, [applyBackInterception]);

  return <PlatformContext.Provider value={snapshot}>{children}</PlatformContext.Provider>;
}

export function usePlatformCapabilities(): PlatformCapabilities {
  return useContext(PlatformContext);
}
