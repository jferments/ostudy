import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MobileAppBar } from "./MobileAppBar";

describe("MobileAppBar", () => {
  it("renders the route title without app-owned window controls", () => {
    render(<MobileAppBar section="documents" activeWorkflow={null} />);

    const bar = screen.getByTestId("mobile-app-bar");
    expect(within(bar).getByText("Documents")).toBeTruthy();
    expect(within(bar).queryByRole("button")).toBeNull();
  });

  it("keeps the active Practice workflow title", () => {
    render(<MobileAppBar section="decks" activeWorkflow="practice" />);

    expect(screen.getByText("Practice")).toBeTruthy();
  });
});
