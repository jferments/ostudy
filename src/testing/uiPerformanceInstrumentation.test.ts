import { afterEach, describe, expect, it, vi } from "vitest";

import {
  armUiPerformanceMeasurement,
  beginUiPerformancePointerWork,
  endUiPerformancePointerWork,
  finishUiPerformanceMeasurement,
  recordUiPerformanceCommit,
  resetUiPerformanceMeasurement,
} from "./uiPerformanceInstrumentation";

afterEach(() => {
  resetUiPerformanceMeasurement();
  vi.restoreAllMocks();
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

  it("finishes after a same-turn response frame without losing strict evidence", async () => {
    const target = document.createElement("button");
    document.body.append(target);
    armUiPerformanceMeasurement("navigation-hover-v1", target);
    target.dispatchEvent(new PointerEvent("pointerover", { bubbles: true }));
    recordUiPerformanceCommit("navigation", "update", 1.25, 2, 10, performance.now());
    const result = await finishUiPerformanceMeasurement();
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
    target.setAttribute("checked", "");
    const result = await finishUiPerformanceMeasurement();
    expect(result.pointer).toEqual({
      state: "notApplicable",
      eventCount: null,
      frameCount: null,
      maxFrameWorkMs: null,
      maxFrameIntervalMs: null,
    });
    target.remove();
  });

  it("does not let late target listeners start the production deck-drag measurement", async () => {
    const target = document.createElement("article");
    document.body.append(target);
    armUiPerformanceMeasurement("deck-drag-production-v2", target);

    target.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    target.dispatchEvent(new PointerEvent("pointermove", { bubbles: true }));

    await expect(finishUiPerformanceMeasurement()).rejects.toThrow(/No interaction/);
    target.remove();
  });

  it("counts production activation once while measuring activation and rAF work", async () => {
    const target = document.createElement("article");
    document.body.append(target);
    let clock = 10;
    vi.spyOn(performance, "now").mockImplementation(() => clock);
    armUiPerformanceMeasurement("deck-drag-production-v2", target);

    const activation = beginUiPerformancePointerWork("deck-drag-activation");
    clock = 16;
    endUiPerformancePointerWork(activation);
    const frame = beginUiPerformancePointerWork("deck-drag-frame");
    clock = 25;
    target.dataset.dragPreview = "true";
    endUiPerformancePointerWork(frame);
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

    const result = await finishUiPerformanceMeasurement();
    expect(result.userTiming.startTimeMs).toBe(10);
    expect(result.pointer.eventCount).toBe(1);
    expect(result.pointer.frameCount).toBeGreaterThanOrEqual(1);
    expect(result.pointer.maxFrameWorkMs).toBe(15);
    target.remove();
  });

  it("fails closed when production activation has no completed rAF work", async () => {
    const target = document.createElement("article");
    document.body.append(target);
    armUiPerformanceMeasurement("deck-drag-production-v2", target);

    const activation = beginUiPerformancePointerWork("deck-drag-activation");
    target.dataset.dragPreview = "true";
    endUiPerformancePointerWork(activation);

    await expect(finishUiPerformanceMeasurement()).rejects.toThrow(
      /No production drag animation frame completed/,
    );
    target.remove();
  });

  it("does not add a frame barrier after the visible mark already exists", async () => {
    const target = document.createElement("button");
    document.body.append(target);
    armUiPerformanceMeasurement("study-guide-open-v1", target);
    target.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    target.dataset.open = "true";
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    const requestFrame = vi.spyOn(window, "requestAnimationFrame");

    await finishUiPerformanceMeasurement();

    expect(requestFrame).not.toHaveBeenCalled();
    target.remove();
  });

  it("requires an observed interaction", async () => {
    const target = document.createElement("button");
    armUiPerformanceMeasurement("study-guide-open-v1", target);
    await expect(finishUiPerformanceMeasurement()).rejects.toThrow(/No interaction/);
  });

  it("preserves the bounded-capacity failure", async () => {
    const target = document.createElement("button");
    armUiPerformanceMeasurement("navigation-hover-v1", target);
    for (let index = 0; index < 2_049; index += 1) {
      target.dispatchEvent(new PointerEvent("pointerover", { bubbles: true }));
    }

    await expect(finishUiPerformanceMeasurement()).rejects.toThrow(/bounded capacity/);
  });

  it("rejects a measurement replaced while its visible frame is pending", async () => {
    const target = document.createElement("button");
    document.body.append(target);
    armUiPerformanceMeasurement("study-guide-open-v1", target);
    target.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    target.dataset.open = "true";
    const pending = finishUiPerformanceMeasurement();
    resetUiPerformanceMeasurement();
    await expect(pending).rejects.toThrow(/changed while awaiting/);
    target.remove();
  });

  it("does not synthesize visibility when the scheduled response mark is missed", async () => {
    const frameCallbacks: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frameCallbacks.push(callback);
      return frameCallbacks.length;
    });
    const target = document.createElement("button");
    document.body.append(target);
    armUiPerformanceMeasurement("study-guide-open-v1", target);
    target.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    const pending = finishUiPerformanceMeasurement();
    expect(frameCallbacks).toHaveLength(3);

    frameCallbacks[2](performance.now());

    await expect(pending).rejects.toThrow(/No visible-response mutation/);
    target.remove();
  });
});
