import { useEffect, useRef } from "react";

import { listenToLifecycle, type LifecyclePhase } from "../ipc/platform";

/** Why the current card is not visible to the learner right now. */
export type StudyTimePause = "hidden" | "lifecycle" | "editor";

/** The largest active time a review can record (24 hours). */
export const MAX_ACTIVE_TIME_MS = 86_400_000;

const PAUSING_PHASES: ReadonlySet<LifecyclePhase> = new Set([
  "background",
  "suspending",
  "termination_requested",
  "terminating",
]);
const RESUMING_PHASES: ReadonlySet<LifecyclePhase> = new Set(["starting", "foreground", "resuming"]);

function monotonicNow(): number {
  return performance.now();
}

/**
 * Accumulates the time one card is actually visible. The clock runs only while
 * no pause reason is active, so time with the app hidden, paused by the
 * platform, or replaced by the card editor does not count.
 */
export class VisibleStudyTime {
  readonly #now: () => number;
  readonly #pauses = new Set<StudyTimePause>();
  #accumulatedMs = 0;
  #runningSince: number | null;

  constructor(now: () => number = monotonicNow, initialPauses: Iterable<StudyTimePause> = []) {
    this.#now = now;
    for (const reason of initialPauses) this.#pauses.add(reason);
    this.#runningSince = this.#pauses.size === 0 ? now() : null;
  }

  /** Start measuring a newly shown card. Active pause reasons still apply. */
  restart(): void {
    this.#accumulatedMs = 0;
    this.#runningSince = this.#pauses.size === 0 ? this.#now() : null;
  }

  pause(reason: StudyTimePause): void {
    if (this.#pauses.has(reason)) return;
    this.#accumulatedMs = this.elapsedMs();
    this.#runningSince = null;
    this.#pauses.add(reason);
  }

  resume(reason: StudyTimePause): void {
    if (!this.#pauses.delete(reason) || this.#pauses.size > 0) return;
    this.#runningSince = this.#now();
  }

  /** Visible milliseconds since the last restart. */
  elapsedMs(): number {
    if (this.#runningSince === null) return this.#accumulatedMs;
    return this.#accumulatedMs + Math.max(0, this.#now() - this.#runningSince);
  }
}

/**
 * The active time a review records: whole visible milliseconds, or `null` when
 * the measurement is outside what a review can store. The raw value is never
 * capped, so an out-of-range measurement is reported as unknown instead.
 */
export function activeTimeForReview(elapsedMs: number): number | null {
  const rounded = Math.round(elapsedMs);
  return Number.isFinite(rounded) && rounded >= 0 && rounded <= MAX_ACTIVE_TIME_MS ? rounded : null;
}

function documentHidden(): boolean {
  return typeof document !== "undefined" && document.visibilityState !== "visible";
}

/**
 * One visible-time accumulator for a practice screen, paused while the document
 * is hidden and between the platform's background and foreground events.
 */
export function useVisibleStudyTime(): VisibleStudyTime {
  const timerRef = useRef<VisibleStudyTime | null>(null);
  if (timerRef.current === null) {
    timerRef.current = new VisibleStudyTime(monotonicNow, documentHidden() ? ["hidden"] : []);
  }
  const timer = timerRef.current;

  useEffect(() => {
    const observeVisibility = (): void => {
      if (documentHidden()) timer.pause("hidden");
      else timer.resume("hidden");
    };
    observeVisibility();
    document.addEventListener("visibilitychange", observeVisibility);
    let disposed = false;
    let unlisten: (() => void) | undefined;
    // Desktop and Android report lifecycle changes through native events; a
    // missing native bridge leaves document visibility as the only signal.
    void listenToLifecycle((snapshot) => {
      if (PAUSING_PHASES.has(snapshot.phase)) timer.pause("lifecycle");
      else if (RESUMING_PHASES.has(snapshot.phase)) timer.resume("lifecycle");
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      document.removeEventListener("visibilitychange", observeVisibility);
      unlisten?.();
    };
  }, [timer]);

  return timer;
}
