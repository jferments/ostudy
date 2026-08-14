import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
} from "../../platform/backNavigation";
import { TourProvider } from "./TourProvider";

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

  it("starts a short first-run module only when its contextual anchor exists", async () => {
    const view = render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );

    await act(async () => {
      vi.advanceTimersByTime(700);
    });
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

  it("lets Android Back dismiss the active tutorial and records the dismissal", async () => {
    const view = render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );
    await act(async () => {
      vi.advanceTimersByTime(700);
    });

    expect(dispatchPlatformBack()).toBe(true);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(window.localStorage.getItem("ostudy.tutorials.v1")).toContain("orientation");
    view.unmount();
  });

  it("never starts contextual guidance when tutorials are disabled", async () => {
    render(
      <TourProvider enabled={false}>
        <Anchors />
      </TourProvider>,
    );
    await act(async () => {
      vi.advanceTimersByTime(2_000);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
