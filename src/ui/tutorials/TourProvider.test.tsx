import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
} from "../../platform/backNavigation";
import { startTutorial, TourProvider } from "./TourProvider";

function Anchors() {
  return (
    <div>
      <nav data-tour-anchor="app.navigation">Navigation</nav>
      <div data-tour-anchor="flashcards.workspaces">Workspaces</div>
    </div>
  );
}

describe("TourProvider", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
    expect(platformBackHandlerCountForTests()).toBe(0);
  });

  it("does not auto-start a tutorial when contextual anchors appear", async () => {
    render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );

    await act(async () => {
      vi.advanceTimersByTime(5_000);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(platformBackHandlerCountForTests()).toBe(0);
  });

  it("starts a tutorial only after an explicit start action", async () => {
    const view = render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );

    act(() => startTutorial("orientation"));
    expect(
      screen.getByRole("dialog", {
        name: "Everything for flashcards lives together",
      }),
    ).toBeTruthy();
    expect(platformBackHandlerCountForTests()).toBe(1);

    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByRole("dialog", { name: "Move between study tools" })).toBeTruthy();

    view.unmount();
  });

  it("lets Android Back dismiss an explicitly started tutorial and records the dismissal", async () => {
    const view = render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );
    act(() => startTutorial("orientation"));

    await act(async () => {
      expect(dispatchPlatformBack()).toBe(true);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(window.localStorage.getItem("ostudy.tutorials.v1")).toContain("orientation");
    view.unmount();
  });

  it("ignores explicit tutorial starts while tutorials are disabled", () => {
    render(
      <TourProvider enabled={false}>
        <Anchors />
      </TourProvider>,
    );
    act(() => startTutorial("orientation"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
