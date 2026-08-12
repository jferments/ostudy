import {
  createContext,
  type ReactNode,
  useContext,
  useEffect,
  useState,
} from "react";

import { getErrorMessage } from "../ipc/common";
import {
  getPlatformCapabilities,
  initializePlatformCapabilities,
  subscribePlatformCapabilities,
  type PlatformCapabilities,
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

  return <PlatformContext.Provider value={snapshot}>{children}</PlatformContext.Provider>;
}

export function usePlatformCapabilities(): PlatformCapabilities {
  return useContext(PlatformContext);
}
