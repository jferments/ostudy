import type { ProfilerOnRenderCallback } from "react";

export const UI_PERFORMANCE_INSTRUMENTATION_VERSION = "ui-performance-v1";
export const UI_PERFORMANCE_CAPTURE_ENABLED =
  typeof __OSTUDY_UI_PERFORMANCE_CAPTURE__ !== "undefined" &&
  __OSTUDY_UI_PERFORMANCE_CAPTURE__;
export const UI_PERFORMANCE_SCENARIOS = [
  "navigation-hover-v1",
  "document-list-interaction-v1",
  "deck-drag-v1",
  "deck-drag-production-v2",
  "deck-selection-v1",
  "study-guide-open-v1",
  "active-practice-progress-v1",
] as const;
export type UiPerformanceScenarioId = (typeof UI_PERFORMANCE_SCENARIOS)[number];

export const UI_PERFORMANCE_BOUNDARIES = [
  "navigation",
  "documents",
  "deck-library",
  "practice-setup",
  "guide-library",
  "guide-dialog",
  "active-practice",
] as const;
export type UiPerformanceBoundaryId = (typeof UI_PERFORMANCE_BOUNDARIES)[number];

const EXPECTED_BOUNDARIES: Record<UiPerformanceScenarioId, readonly UiPerformanceBoundaryId[]> = {
  "navigation-hover-v1": ["navigation"],
  "document-list-interaction-v1": ["documents"],
  "deck-drag-v1": ["deck-library"],
  "deck-drag-production-v2": ["deck-library"],
  "deck-selection-v1": ["practice-setup"],
  "study-guide-open-v1": ["guide-library", "guide-dialog"],
  "active-practice-progress-v1": ["active-practice"],
};
const POINTER_SCENARIOS = new Set<UiPerformanceScenarioId>([
  "navigation-hover-v1",
  "deck-drag-v1",
  "deck-drag-production-v2",
]);
const MAX_COMMITS = 512;
const MAX_POINTER_EVENTS = 2048;

export interface UiPerformanceCommit {
  boundaryId: UiPerformanceBoundaryId;
  phase: "mount" | "update" | "nested-update";
  actualDurationMs: number;
  baseDurationMs: number;
  startTimeMs: number;
  commitTimeMs: number;
}

export interface UiPerformanceSample {
  scenarioId: UiPerformanceScenarioId;
  interactionToVisibleMs: number;
  interactionToReadyMs: number;
  userTiming: { startTimeMs: number; visibleTimeMs: number; readyTimeMs: number };
  react: {
    commitCount: number;
    totalRenderDurationMs: number;
    avoidableCommitCount: number;
    commits: UiPerformanceCommit[];
  };
  pointer: {
    state: "available" | "notApplicable";
    eventCount: number | null;
    frameCount: number | null;
    maxFrameWorkMs: number | null;
    maxFrameIntervalMs: number | null;
  };
}

interface ActiveMeasurement {
  scenarioId: UiPerformanceScenarioId;
  target: Element;
  startTime: number | null;
  visibleTime: number | null;
  commits: UiPerformanceCommit[];
  pointerEventCount: number;
  pointerFrameWork: Map<number, number>;
  productionFrameCount: number;
  frameIntervals: number[];
  currentFrame: number;
  lastFrameTime: number;
  visibleFramePending: boolean;
  overflowed: boolean;
  mutationObserver: MutationObserver;
  frameHandle: number;
  scheduleVisibleMark(): void;
  removeListeners(): void;
}

export interface UiPerformancePointerWork {
  readonly owner: object;
  readonly frame: number;
  readonly startedAt: number;
  readonly phase: UiPerformancePointerWorkPhase;
}

export type UiPerformancePointerWorkPhase =
  | "deck-drag-activation"
  | "deck-drag-pointer"
  | "deck-drag-frame";

let active: ActiveMeasurement | null = null;

function assertScenario(value: string): asserts value is UiPerformanceScenarioId {
  if (!(UI_PERFORMANCE_SCENARIOS as readonly string[]).includes(value)) {
    throw new Error(`Unknown UI performance scenario: ${value}`);
  }
}

function finiteNonnegative(value: number, label: string): number {
  if (!Number.isFinite(value) || value < 0) throw new Error(`${label} must be finite and nonnegative.`);
  return value;
}

function stopActive(): void {
  if (!active) return;
  active.mutationObserver.disconnect();
  cancelAnimationFrame(active.frameHandle);
  active.removeListeners();
  active = null;
}

export function armUiPerformanceMeasurement(scenarioValue: string, target: Element): void {
  assertScenario(scenarioValue);
  if (!(target instanceof Element)) throw new Error("Performance target must be a DOM element.");
  stopActive();
  performance.clearMarks("ostudy-ui-interaction-start");
  performance.clearMarks("ostudy-ui-visible");
  performance.clearMarks("ostudy-ui-ready");
  performance.clearMeasures("ostudy-ui-interaction-to-visible");
  performance.clearMeasures("ostudy-ui-interaction-to-ready");

  let measurement!: ActiveMeasurement;
  const scheduleVisibleMark = (): void => {
    if (
      measurement.startTime === null ||
      measurement.visibleTime !== null ||
      measurement.visibleFramePending
    ) {
      return;
    }
    measurement.visibleFramePending = true;
    requestAnimationFrame(() => {
      measurement.visibleFramePending = false;
      if (active !== measurement || measurement.visibleTime !== null) return;
      const time = performance.now();
      measurement.visibleTime = time;
      performance.mark("ostudy-ui-visible", { startTime: time });
      performance.measure("ostudy-ui-interaction-to-visible", {
        start: "ostudy-ui-interaction-start",
        end: "ostudy-ui-visible",
      });
    });
  };
  const begin = (): void => {
    if (measurement.startTime !== null) return;
    measurement.startTime = performance.now();
    performance.mark("ostudy-ui-interaction-start", { startTime: measurement.startTime });
    scheduleVisibleMark();
  };
  const pointerStart = (): void => {
    begin();
    (measurement as ActiveMeasurement & { pointerStartedAt?: number }).pointerStartedAt = performance.now();
  };
  const pointerEnd = (): void => {
    const extended = measurement as ActiveMeasurement & { pointerStartedAt?: number };
    if (extended.pointerStartedAt === undefined) return;
    measurement.pointerEventCount += 1;
    if (measurement.pointerEventCount > MAX_POINTER_EVENTS) measurement.overflowed = true;
    const duration = performance.now() - extended.pointerStartedAt;
    measurement.pointerFrameWork.set(
      measurement.currentFrame,
      (measurement.pointerFrameWork.get(measurement.currentFrame) ?? 0) + duration,
    );
    delete extended.pointerStartedAt;
  };
  const beginEvents =
    scenarioValue === "deck-drag-production-v2"
      ? []
      : POINTER_SCENARIOS.has(scenarioValue)
        ? ["pointerover", "pointerdown", "pointermove"]
        : ["click", "change"];
  beginEvents.forEach((eventName) => target.addEventListener(eventName, pointerStart, true));
  beginEvents.forEach((eventName) => target.addEventListener(eventName, pointerEnd, false));

  const observer = new MutationObserver(() => {
    scheduleVisibleMark();
  });
  observer.observe(document.documentElement, {
    attributes: true,
    childList: true,
    characterData: true,
    subtree: true,
  });
  const tick = (): void => {
    const time = performance.now();
    if (measurement.lastFrameTime > 0 && measurement.startTime !== null) {
      measurement.frameIntervals.push(time - measurement.lastFrameTime);
    }
    measurement.lastFrameTime = time;
    measurement.currentFrame += 1;
    measurement.frameHandle = requestAnimationFrame(tick);
  };
  measurement = {
    scenarioId: scenarioValue,
    target,
    startTime: null,
    visibleTime: null,
    commits: [],
    pointerEventCount: 0,
    pointerFrameWork: new Map(),
    productionFrameCount: 0,
    frameIntervals: [],
    currentFrame: 0,
    lastFrameTime: 0,
    visibleFramePending: false,
    overflowed: false,
    mutationObserver: observer,
    frameHandle: requestAnimationFrame(tick),
    scheduleVisibleMark,
    removeListeners: () => {
      beginEvents.forEach((eventName) => target.removeEventListener(eventName, pointerStart, true));
      beginEvents.forEach((eventName) => target.removeEventListener(eventName, pointerEnd, false));
    },
  };
  active = measurement;
}

export function beginUiPerformancePointerWork(
  phase: UiPerformancePointerWorkPhase,
): UiPerformancePointerWork | null {
  const measurement = active;
  if (!measurement || measurement.scenarioId !== "deck-drag-production-v2") {
    return null;
  }
  const startedAt = performance.now();
  if (measurement.startTime === null) {
    if (phase !== "deck-drag-activation") return null;
    measurement.startTime = startedAt;
    performance.mark("ostudy-ui-interaction-start", { startTime: startedAt });
    measurement.scheduleVisibleMark();
  }
  return {
    owner: measurement,
    frame: measurement.currentFrame,
    startedAt,
    phase,
  };
}

export function endUiPerformancePointerWork(
  work: UiPerformancePointerWork | null,
): void {
  if (!work || active !== work.owner) return;
  const measurement = work.owner as ActiveMeasurement;
  if (work.phase === "deck-drag-frame") {
    measurement.productionFrameCount += 1;
  } else {
    measurement.pointerEventCount += 1;
    if (measurement.pointerEventCount > MAX_POINTER_EVENTS) measurement.overflowed = true;
  }
  const duration = finiteNonnegative(
    performance.now() - work.startedAt,
    "pointer work",
  );
  measurement.pointerFrameWork.set(
    work.frame,
    (measurement.pointerFrameWork.get(work.frame) ?? 0) + duration,
  );
}

export const recordUiPerformanceCommit: ProfilerOnRenderCallback = (
  id,
  phase,
  actualDuration,
  baseDuration,
  startTime,
  commitTime,
) => {
  if (!active || active.startTime === null || commitTime < active.startTime) return;
  if (!(UI_PERFORMANCE_BOUNDARIES as readonly string[]).includes(id)) return;
  if (active.commits.length >= MAX_COMMITS) {
    active.overflowed = true;
    return;
  }
  active.commits.push({
    boundaryId: id as UiPerformanceBoundaryId,
    phase,
    actualDurationMs: finiteNonnegative(actualDuration, "actualDuration"),
    baseDurationMs: finiteNonnegative(baseDuration, "baseDuration"),
    startTimeMs: finiteNonnegative(startTime, "startTime"),
    commitTimeMs: finiteNonnegative(commitTime, "commitTime"),
  });
};

export async function finishUiPerformanceMeasurement(): Promise<UiPerformanceSample> {
  const measurement = active;
  if (!measurement || measurement.startTime === null) throw new Error("No interaction was observed.");
  if (measurement.overflowed) throw new Error("UI performance measurement exceeded its bounded capacity.");
  const productionFramePending =
    measurement.scenarioId === "deck-drag-production-v2" &&
    measurement.productionFrameCount === 0;
  if (
    (measurement.visibleTime === null && measurement.visibleFramePending) ||
    productionFramePending
  ) {
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  }
  if (active !== measurement) {
    throw new Error("UI performance measurement changed while awaiting its visible response.");
  }
  if (measurement.overflowed) throw new Error("UI performance measurement exceeded its bounded capacity.");
  if (
    measurement.scenarioId === "deck-drag-production-v2" &&
    measurement.productionFrameCount === 0
  ) {
    throw new Error("No production drag animation frame completed.");
  }
  const visibleTime = measurement.visibleTime;
  if (visibleTime === null) throw new Error("No visible-response mutation was observed.");
  const readyTime = performance.now();
  performance.mark("ostudy-ui-ready", { startTime: readyTime });
  performance.measure("ostudy-ui-interaction-to-ready", {
    start: "ostudy-ui-interaction-start",
    end: "ostudy-ui-ready",
  });
  const expected = new Set(EXPECTED_BOUNDARIES[measurement.scenarioId]);
  const uniqueCommitTimes = new Set(measurement.commits.map((commit) => commit.commitTimeMs));
  const avoidableCommitTimes = new Set(
    measurement.commits
      .filter((commit) => !expected.has(commit.boundaryId))
      .map((commit) => commit.commitTimeMs),
  );
  const pointerApplicable = POINTER_SCENARIOS.has(measurement.scenarioId);
  const frameWork = [...measurement.pointerFrameWork.values()];
  const result: UiPerformanceSample = {
    scenarioId: measurement.scenarioId,
    interactionToVisibleMs: finiteNonnegative(visibleTime - measurement.startTime, "visible latency"),
    interactionToReadyMs: finiteNonnegative(readyTime - measurement.startTime, "ready latency"),
    userTiming: { startTimeMs: measurement.startTime, visibleTimeMs: visibleTime, readyTimeMs: readyTime },
    react: {
      commitCount: uniqueCommitTimes.size,
      totalRenderDurationMs: measurement.commits.reduce((sum, commit) => sum + commit.actualDurationMs, 0),
      avoidableCommitCount: avoidableCommitTimes.size,
      commits: measurement.commits,
    },
    pointer: {
      state: pointerApplicable ? "available" : "notApplicable",
      eventCount: pointerApplicable ? measurement.pointerEventCount : null,
      frameCount: pointerApplicable ? measurement.pointerFrameWork.size : null,
      maxFrameWorkMs: pointerApplicable ? Math.max(0, ...frameWork) : null,
      maxFrameIntervalMs: pointerApplicable ? Math.max(0, ...measurement.frameIntervals) : null,
    },
  };
  stopActive();
  return result;
}

export function resetUiPerformanceMeasurement(): void {
  stopActive();
}
