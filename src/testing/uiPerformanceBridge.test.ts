import { afterEach, describe, expect, it, vi } from "vitest";

afterEach(async () => {
  delete window.__OSTUDY_UI_PERFORMANCE__;
  vi.doUnmock("./uiPerformanceInstrumentation");
  vi.resetModules();
});

describe("UI performance bridge production boundary", () => {
  it("does not install in the ordinary test/production-disabled build", async () => {
    const { installUiPerformanceBridge } = await import("./uiPerformanceBridge");
    installUiPerformanceBridge();
    expect(window.__OSTUDY_UI_PERFORMANCE__).toBeUndefined();
  });

  it("installs the asynchronous bridge in a capture-enabled build", async () => {
    const sample = { scenarioId: "navigation-hover-v1" };
    const arm = vi.fn();
    const finish = vi.fn(async () => sample);
    const reset = vi.fn();
    vi.doMock("./uiPerformanceInstrumentation", () => ({
      armUiPerformanceMeasurement: arm,
      finishUiPerformanceMeasurement: finish,
      resetUiPerformanceMeasurement: reset,
      UI_PERFORMANCE_CAPTURE_ENABLED: true,
      UI_PERFORMANCE_INSTRUMENTATION_VERSION: "ui-performance-v1",
    }));
    const { installUiPerformanceBridge } = await import("./uiPerformanceBridge");

    installUiPerformanceBridge();

    expect(window.__OSTUDY_UI_PERFORMANCE__).toMatchObject({
      version: "ui-performance-v1",
      profilingRenderer: true,
    });
    const bridge = window.__OSTUDY_UI_PERFORMANCE__;
    if (!bridge) throw new Error("Expected the capture bridge to be installed.");
    const first = bridge.finish();
    const recovered = bridge.finish();
    expect(recovered).toBe(first);
    await expect(first).resolves.toBe(sample);
    expect(bridge.finish()).toBe(first);
    expect(finish).toHaveBeenCalledOnce();

    const target = document.createElement("button");
    bridge.arm("navigation-hover-v1", target);
    expect(arm).toHaveBeenCalledWith("navigation-hover-v1", target);
    expect(bridge.finish()).not.toBe(first);
    expect(finish).toHaveBeenCalledTimes(2);

    const second = bridge.finish();
    bridge.reset();
    expect(reset).toHaveBeenCalledOnce();
    expect(bridge.finish()).not.toBe(second);
    expect(finish).toHaveBeenCalledTimes(3);
  });
});
