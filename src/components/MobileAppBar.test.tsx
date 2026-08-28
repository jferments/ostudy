import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MobileAppBar } from "./MobileAppBar";

describe("MobileAppBar", () => {
  it("composes compact desktop window controls without changing the route title", () => {
    render(
      <MobileAppBar
        section="documents"
        activeWorkflow={null}
        windowControls={<button type="button">Window control</button>}
      />,
    );

    const bar = screen.getByTestId("mobile-app-bar");
    expect(within(bar).getByText("Documents")).toBeTruthy();
    expect(within(bar).getByRole("button", { name: "Window control" })).toBeTruthy();
  });

  it("keeps the active Practice workflow title alongside the control slot", () => {
    render(
      <MobileAppBar
        section="decks"
        activeWorkflow="practice"
        windowControls={<span data-testid="compact-window-controls" />}
      />,
    );

    expect(screen.getByText("Practice")).toBeTruthy();
    expect(screen.getByTestId("compact-window-controls")).toBeTruthy();
  });
});
