import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";

import { audioFeedback, hapticFeedback } from "../../platform/feedback";
import type { FeedbackEvent } from "./feedbackEvents";
import styles from "./experience.module.css";

interface ExperienceContextValue {
  emit(event: FeedbackEvent): void;
}

const ExperienceContext = createContext<ExperienceContextValue>({ emit: () => undefined });

export function ExperienceProvider({ children }: { children: ReactNode }) {
  const [pulse, setPulse] = useState<{ id: number; event: FeedbackEvent } | null>(null);

  const emit = useCallback((event: FeedbackEvent) => {
    setPulse({ id: Date.now(), event });
    if (event.type === "review_committed") {
      void hapticFeedback.pulse(event.score >= 3 ? "medium" : "light");
      void audioFeedback.play(`review-${event.score}`);
    } else if (event.type === "session_completed") {
      void hapticFeedback.pulse("strong");
      void audioFeedback.play("session-complete");
    }
    window.setTimeout(() => setPulse((current) => (current?.event === event ? null : current)), 500);
  }, []);

  const value = useMemo(() => ({ emit }), [emit]);
  return (
    <ExperienceContext.Provider value={value}>
      {children}
      <div className={styles.effectsLayer} aria-hidden="true" data-effect={pulse?.event.type ?? ""} />
    </ExperienceContext.Provider>
  );
}

export function useExperience(): ExperienceContextValue {
  return useContext(ExperienceContext);
}
