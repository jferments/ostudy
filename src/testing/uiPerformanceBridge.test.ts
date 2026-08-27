import { afterEach, describe, expect, it } from "vitest";

import { installUiPerformanceBridge } from "./uiPerformanceBridge";

afterEach(() => {
  delete window.__OSTUDY_UI_PERFORMANCE__;
});

describe("UI performance bridge production boundary", () => {
  it("does not install in the ordinary test/production-disabled build", () => {
    installUiPerformanceBridge();
    expect(window.__OSTUDY_UI_PERFORMANCE__).toBeUndefined();
  });
});
