import {
  armUiPerformanceMeasurement,
  finishUiPerformanceMeasurement,
  resetUiPerformanceMeasurement,
  UI_PERFORMANCE_CAPTURE_ENABLED,
  UI_PERFORMANCE_INSTRUMENTATION_VERSION,
  type UiPerformanceSample,
} from "./uiPerformanceInstrumentation";

export interface UiPerformanceBridge {
  readonly version: string;
  readonly profilingRenderer: true;
  arm(scenarioId: string, target: Element): void;
  finish(): UiPerformanceSample;
  reset(): void;
}

declare global {
  interface Window {
    __OSTUDY_UI_PERFORMANCE__?: UiPerformanceBridge;
  }
}

export function installUiPerformanceBridge(): void {
  if (!UI_PERFORMANCE_CAPTURE_ENABLED) return;
  window.__OSTUDY_UI_PERFORMANCE__ = Object.freeze({
    version: UI_PERFORMANCE_INSTRUMENTATION_VERSION,
    profilingRenderer: true as const,
    arm: armUiPerformanceMeasurement,
    finish: finishUiPerformanceMeasurement,
    reset: resetUiPerformanceMeasurement,
  });
}
