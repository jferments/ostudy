import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import * as analyticsIpc from "../ipc/analytics";
import { AnalyticsScreen } from "./AnalyticsScreen";

vi.mock("../ipc/analytics", () => ({
  getAnalytics: vi.fn(),
}));
vi.mock("./analytics/ProgressChart", () => ({
  ProgressChart: () => <div role="img" aria-label="Learning progress chart" />,
}));

describe("Analytics loading presentation", () => {
  it("renders the analytics shell and chart-local analyzing state before aggregation completes", () => {
    vi.mocked(analyticsIpc.getAnalytics).mockImplementation(
      () => new Promise(() => undefined),
    );

    render(
      <AnalyticsScreen
        decks={[]}
        launchRequest={null}
        onLaunchConsumed={vi.fn()}
        onPracticeScope={vi.fn()}
        onSearchTag={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Progress" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Learning progress" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Analyzing data…");
    expect(screen.queryByRole("img", { name: "Learning progress chart" })).toBeNull();
  });
});
