import {
  createContext,
  type ReactNode,
  useContext,
  useLayoutEffect,
  useMemo,
} from "react";

import { useMediaQuery } from "../hooks/useMediaQuery";

export type LayoutMode = "compact" | "medium" | "wide";
export type InputMode = "coarse" | "fine";

interface LayoutModeContextValue {
  layoutMode: LayoutMode;
  inputMode: InputMode;
}

const LayoutModeContext = createContext<LayoutModeContextValue>({
  layoutMode: "wide",
  inputMode: "fine",
});

export function LayoutModeProvider({ children }: { children: ReactNode }) {
  const compact = useMediaQuery("(max-width: 760px)");
  const mediumOrSmaller = useMediaQuery("(max-width: 1180px)");
  const coarsePointer = useMediaQuery("(pointer: coarse)");
  const layoutMode: LayoutMode = compact ? "compact" : mediumOrSmaller ? "medium" : "wide";
  const inputMode: InputMode = coarsePointer ? "coarse" : "fine";

  useLayoutEffect(() => {
    const root = document.documentElement;
    root.dataset.layoutMode = layoutMode;
    root.dataset.inputMode = inputMode;
    return () => {
      if (root.dataset.layoutMode === layoutMode) delete root.dataset.layoutMode;
      if (root.dataset.inputMode === inputMode) delete root.dataset.inputMode;
    };
  }, [inputMode, layoutMode]);

  const value = useMemo(() => ({ layoutMode, inputMode }), [inputMode, layoutMode]);
  return <LayoutModeContext.Provider value={value}>{children}</LayoutModeContext.Provider>;
}

export function useLayoutMode(): LayoutModeContextValue {
  return useContext(LayoutModeContext);
}
