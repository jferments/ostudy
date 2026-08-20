import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

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

function startOrientationTutorial(): void {
  window.dispatchEvent(
    new CustomEvent("ostudy:start-tutorial", {
      detail: { moduleId: "orientation" },
    }),
  );
}

describe("TourProvider", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    expect(platformBackHandlerCountForTests()).toBe(0);
  });

  it("does not auto-start merely because contextual anchors are rendered", async () => {
    const view = render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );

    await act(async () => Promise.resolve());
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(platformBackHandlerCountForTests()).toBe(0);
    view.unmount();
  });

  it("starts and advances a tutorial only after an explicit request", async () => {
    const view = render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );

    await act(async () => startOrientationTutorial());
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

  it("lets Android Back dismiss a manually started tutorial and records dismissal", async () => {
    const view = render(
      <TourProvider enabled>
        <Anchors />
      </TourProvider>,
    );
    await act(async () => startOrientationTutorial());

    await act(async () => {
      expect(dispatchPlatformBack()).toBe(true);
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(window.localStorage.getItem("ostudy.tutorials.v1")).toContain("orientation");
    view.unmount();
  });

  it("ignores explicit tutorial requests while tutorials are disabled", async () => {
    render(
      <TourProvider enabled={false}>
        <Anchors />
      </TourProvider>,
    );
    await act(async () => startOrientationTutorial());
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
