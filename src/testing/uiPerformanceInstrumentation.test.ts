import { afterEach, describe, expect, it, vi } from "vitest";

import {
  armUiPerformanceMeasurement,
  finishUiPerformanceMeasurement,
  recordUiPerformanceCommit,
  resetUiPerformanceMeasurement,
} from "./uiPerformanceInstrumentation";

afterEach(() => {
  resetUiPerformanceMeasurement();
  vi.useRealTimers();
});

describe("UI performance instrumentation", () => {
  it("rejects unknown scenarios and non-elements", () => {
    const target = document.createElement("button");
    expect(() => armUiPerformanceMeasurement("unknown", target)).toThrow(/Unknown/);
    expect(() =>
      armUiPerformanceMeasurement("navigation-hover-v1", {} as Element),
    ).toThrow(/DOM element/);
  });

  it("records numeric User Timing, pointer frames, and bounded Profiler evidence", async () => {
    const target = document.createElement("button");
    document.body.append(target);
    armUiPerformanceMeasurement("navigation-hover-v1", target);
    target.dispatchEvent(new PointerEvent("pointerover", { bubbles: true }));
    recordUiPerformanceCommit("navigation", "update", 1.25, 2, 10, performance.now());
    target.dataset.active = "true";
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    const result = finishUiPerformanceMeasurement();
    expect(result.scenarioId).toBe("navigation-hover-v1");
    expect(result.interactionToVisibleMs).toBeGreaterThanOrEqual(0);
    expect(result.interactionToReadyMs).toBeGreaterThanOrEqual(result.interactionToVisibleMs);
    expect(result.react.commitCount).toBe(1);
    expect(result.react.totalRenderDurationMs).toBe(1.25);
    expect(result.react.avoidableCommitCount).toBe(0);
    expect(result.pointer.state).toBe("available");
    expect(result.pointer.eventCount).toBe(1);
    target.remove();
  });

  it("marks pointer metrics not applicable for discrete interactions", async () => {
    const target = document.createElement("input");
    document.body.append(target);
    armUiPerformanceMeasurement("deck-selection-v1", target);
    target.dispatchEvent(new Event("change", { bubbles: true }));
    target.checked = true;
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    const result = finishUiPerformanceMeasurement();
    expect(result.pointer).toEqual({
      state: "notApplicable",
      eventCount: null,
      frameCount: null,
      maxFrameWorkMs: null,
      maxFrameIntervalMs: null,
    });
    target.remove();
  });

  it("requires an observed interaction and visible frame", () => {
    const target = document.createElement("button");
    armUiPerformanceMeasurement("study-guide-open-v1", target);
    expect(() => finishUiPerformanceMeasurement()).toThrow(/No interaction/);
  });
});
