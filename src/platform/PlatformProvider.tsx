import {
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useState,
} from "react";

import { getErrorMessage } from "../ipc/common";
import {
  listenToAndroidBack,
  listenToAndroidInsets,
  listenToAndroidNetwork,
  setPlatformBackInterception,
} from "../ipc/platform";
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

  useEffect(() => {
    const unsubscribe = subscribePlatformCapabilities(() => setSnapshot(getPlatformCapabilities()));
    void initializePlatformCapabilities().catch((error: unknown) => {
      console.error("Unable to initialize native platform capabilities:", getErrorMessage(error));
    });
    return unsubscribe;
  }, []);

  useEffect(() => {
    const disposers: Array<() => void> = [];
    let disposed = false;

    const backStack = subscribePlatformBackStack((count) => {
      void setPlatformBackInterception(count > 0).catch((error: unknown) => {
        if (!disposed && getPlatformCapabilities().native?.operatingSystem === "android") {
          console.error("Unable to update Android Back interception:", getErrorMessage(error));
        }
      });
    });
    disposers.push(backStack);

    void listenToAndroidBack(() => {
      const consumed = dispatchPlatformBack();
      if (!consumed) {
        // A handler may have disappeared between native interception and event
        // delivery. Disable interception so the next Back action belongs to
        // Android's predictive back-to-home implementation.
        void setPlatformBackInterception(false);
      }
    })
      .then((dispose) => {
        if (disposed) dispose();
        else disposers.push(dispose);
      })
      .catch(() => undefined);

    void listenToAndroidInsets((insets) => {
      updateAndroidInsets(insets);
      const root = document.documentElement;
      root.style.setProperty("--android-safe-top", `${Math.max(0, insets.top)}px`);
      root.style.setProperty("--android-safe-right", `${Math.max(0, insets.right)}px`);
      root.style.setProperty("--android-safe-bottom", `${Math.max(0, insets.bottom)}px`);
      root.style.setProperty("--android-safe-left", `${Math.max(0, insets.left)}px`);
      root.style.setProperty("--android-ime-bottom", `${Math.max(0, insets.imeBottom)}px`);
      root.dataset.androidImeVisible = insets.imeVisible ? "true" : "false";
    })
      .then((dispose) => {
        if (disposed) dispose();
        else disposers.push(dispose);
      })
      .catch(() => undefined);

    void listenToAndroidNetwork(updateAndroidNetwork)
      .then((dispose) => {
        if (disposed) dispose();
        else disposers.push(dispose);
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      for (const dispose of disposers.splice(0)) dispose();
      delete document.documentElement.dataset.androidImeVisible;
    };
  }, []);

  return <PlatformContext.Provider value={snapshot}>{children}</PlatformContext.Provider>;
}

export function usePlatformCapabilities(): PlatformCapabilities {
  return useContext(PlatformContext);
}
