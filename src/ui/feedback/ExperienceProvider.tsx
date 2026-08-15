import {
  createContext,
  type CSSProperties,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { audioFeedback, hapticFeedback } from "../../platform/feedback";
import type { FeedbackEvent } from "./feedbackEvents";
import styles from "./experience.module.css";

interface ExperienceContextValue {
  emit(event: FeedbackEvent): void;
}

interface EffectPulse {
  id: number;
  event: FeedbackEvent;
  celebrate: boolean;
}

type ConfettiStyle = CSSProperties & {
  "--confetti-x": string;
  "--confetti-drift": string;
  "--confetti-distance": string;
  "--confetti-rotation": string;
  "--confetti-delay": string;
};

const ExperienceContext = createContext<ExperienceContextValue>({ emit: () => undefined });
const CONFETTI_PIECES = Array.from({ length: 24 }, (_, index) => index);

export function ExperienceProvider({ children }: { children: ReactNode }) {
  const [pulse, setPulse] = useState<EffectPulse | null>(null);
  const sequence = useRef(1);
  const clearTimer = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (clearTimer.current !== null) window.clearTimeout(clearTimer.current);
    },
    [],
  );

  const emit = useCallback((event: FeedbackEvent) => {
    const root = document.documentElement;
    const motion = root.dataset.motion ?? "full";
    const celebrate =
      event.type === "session_completed" &&
      root.dataset.celebrations !== "disabled" &&
      motion !== "reduced" &&
      motion !== "none";
    const id = sequence.current;
    sequence.current += 1;
    setPulse({ id, event, celebrate });

    if (event.type === "review_committed") {
      void hapticFeedback.pulse(event.score >= 3 ? "medium" : "light");
      void audioFeedback.play(`review-${event.score}`);
    } else if (event.type === "session_completed") {
      void hapticFeedback.pulse("strong");
      void audioFeedback.play("session-complete");
    } else if (event.type === "operation_failed") {
      void hapticFeedback.pulse("light");
    }

    if (clearTimer.current !== null) window.clearTimeout(clearTimer.current);
    clearTimer.current = window.setTimeout(() => {
      setPulse((current) => (current?.id === id ? null : current));
      clearTimer.current = null;
    }, event.type === "session_completed" ? 1_200 : 500);
  }, []);

  const value = useMemo(() => ({ emit }), [emit]);
  return (
    <ExperienceContext.Provider value={value}>
      {children}
      <div
        key={pulse?.id ?? 0}
        className={styles.effectsLayer}
        aria-hidden="true"
        data-effect={pulse?.event.type ?? ""}
        data-celebrate={pulse?.celebrate ? "true" : undefined}
      >
        {pulse?.celebrate
          ? CONFETTI_PIECES.map((piece) => (
              <span
                key={piece}
                className={styles.confetti}
                style={confettiStyle(piece)}
              />
            ))
          : null}
      </div>
    </ExperienceContext.Provider>
  );
}

export function useExperience(): ExperienceContextValue {
  return useContext(ExperienceContext);
}

function confettiStyle(piece: number): ConfettiStyle {
  const column = piece % 12;
  const row = Math.floor(piece / 12);
  return {
    "--confetti-x": `${8 + (column * 84) / 11}%`,
    "--confetti-drift": `${((piece % 5) - 2) * 22}px`,
    "--confetti-distance": `${72 + row * 12}vh`,
    "--confetti-rotation": `${360 + (piece % 4) * 90}deg`,
    "--confetti-delay": `${(piece % 6) * 24}ms`,
  };
}
