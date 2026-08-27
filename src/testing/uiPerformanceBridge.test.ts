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
    const finish = vi.fn(async () => ({ scenarioId: "navigation-hover-v1" }));
    vi.doMock("./uiPerformanceInstrumentation", () => ({
      armUiPerformanceMeasurement: vi.fn(),
      finishUiPerformanceMeasurement: finish,
      resetUiPerformanceMeasurement: vi.fn(),
      UI_PERFORMANCE_CAPTURE_ENABLED: true,
      UI_PERFORMANCE_INSTRUMENTATION_VERSION: "ui-performance-v1",
    }));
    const { installUiPerformanceBridge } = await import("./uiPerformanceBridge");

    installUiPerformanceBridge();

    expect(window.__OSTUDY_UI_PERFORMANCE__).toMatchObject({
      version: "ui-performance-v1",
      profilingRenderer: true,
    });
    await expect(window.__OSTUDY_UI_PERFORMANCE__?.finish()).resolves.toEqual({
      scenarioId: "navigation-hover-v1",
    });
    expect(finish).toHaveBeenCalledOnce();
  });
});
