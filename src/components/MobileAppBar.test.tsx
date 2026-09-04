import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MobileAppBar } from "./MobileAppBar";

describe("MobileAppBar", () => {
  it("renders the route title and the upper-right Settings action without window controls", () => {
    const onNavigate = vi.fn();
    render(
      <MobileAppBar
        section="documents"
        activeWorkflow={null}
        onNavigate={onNavigate}
      />,
    );

    const bar = screen.getByTestId("mobile-app-bar");
    expect(within(bar).getByText("Documents")).toBeTruthy();
    const settings = within(bar).getByRole("button", { name: "Settings" });
    expect(settings).toBeTruthy();
    expect(settings.getAttribute("aria-current")).toBeNull();
    expect(
      within(bar).queryByRole("button", {
        name: /maximize|restore window|full screen/iu,
      }),
    ).toBeNull();

    fireEvent.click(settings);
    expect(onNavigate).toHaveBeenCalledWith("settings");
  });

  it("keeps the active Practice workflow title and leaves Settings inactive", () => {
    render(
      <MobileAppBar
        section="decks"
        activeWorkflow="practice"
        onNavigate={vi.fn()}
      />,
    );

    expect(screen.getByText("Practice")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Settings" }).getAttribute("aria-current")).toBeNull();
  });

  it("marks the Settings gear active for Settings and its Card templates compatibility route", () => {
    const view = render(
      <MobileAppBar section="settings" activeWorkflow={null} onNavigate={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: "Settings" }).getAttribute("aria-current")).toBe(
      "page",
    );

    view.rerender(
      <MobileAppBar section="templates" activeWorkflow={null} onNavigate={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: "Settings" }).getAttribute("aria-current")).toBe(
      "page",
    );
  });
});
