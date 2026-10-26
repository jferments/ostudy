import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import { listAiProviderProfiles } from "../ipc/aiProfiles";

/**
 * Builds made with `VITE_OSTUDY_AI=off` never show AI features, whatever is
 * set up: the flashcard and notes app on its own.
 */
export const AI_FEATURES_BUILT = import.meta.env.VITE_OSTUDY_AI !== "off";

export interface AiFeatures {
  /**
   * Whether AI features show: the Tutor, lessons, study guides, podcasts,
   * and every action that asks a model for something. True once an enabled
   * AI service is set up.
   */
  enabled: boolean;
  /** False until the services have been read once. */
  known: boolean;
  /** Whether any AI service exists at all, enabled or not. */
  hasServices: boolean;
  /** Reads the services again, as after leaving Settings. */
  refresh: () => void;
}

// Without a provider (most component tests) AI features show, as before.
const AiFeaturesContext = createContext<AiFeatures>({
  enabled: AI_FEATURES_BUILT,
  known: true,
  hasServices: true,
  refresh: () => undefined,
});

/**
 * Reads whether AI features show, for `AiFeaturesProvider` to share. Every
 * AI-only page, tab, panel and action reads `useAiFeatures().enabled` (or
 * renders inside `AiOnly`), so turning AI off hides them all while cards,
 * notes, documents and their marks keep working. The services are read at
 * start and whenever `section` changes, so setting AI up in Settings shows the
 * features on leaving it.
 */
export function useAiFeaturesSource(section: string): AiFeatures {
  const [state, setState] = useState({ known: false, enabled: false, hasServices: false });
  const [generation, setGeneration] = useState(0);
  const refresh = useCallback(() => setGeneration((current) => current + 1), []);

  useEffect(() => {
    if (!AI_FEATURES_BUILT) {
      setState({ known: true, enabled: false, hasServices: false });
      return;
    }
    let live = true;
    listAiProviderProfiles().then(
      (profiles) => {
        if (!live) return;
        setState({
          known: true,
          enabled: profiles.some((profile) => profile.enabled),
          hasServices: profiles.length > 0,
        });
      },
      // An unreadable list keeps what was known; a first failure shows no AI.
      () => {
        if (live) setState((current) => ({ ...current, known: true }));
      },
    );
    return () => {
      live = false;
    };
  }, [generation, section]);

  return useMemo<AiFeatures>(() => ({ ...state, refresh }), [refresh, state]);
}

/** Shares what `useAiFeaturesSource` read with everything below it. */
export function AiFeaturesProvider({ value, children }: { value: AiFeatures; children: ReactNode }) {
  return <AiFeaturesContext.Provider value={value}>{children}</AiFeaturesContext.Provider>;
}

export function useAiFeatures(): AiFeatures {
  return useContext(AiFeaturesContext);
}

/** Renders its children only while AI features show. */
export function AiOnly({ children, fallback = null }: { children: ReactNode; fallback?: ReactNode }) {
  return useAiFeatures().enabled ? <>{children}</> : <>{fallback}</>;
}
