import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Toast } from "./Toast";

describe("Toast", () => {
  it("renders a status message and dismisses it", () => {
    const onDismiss = vi.fn();

    render(
      <Toast
        toast={{ id: 1, kind: "success", text: "Deck saved." }}
        onDismiss={onDismiss}
      />,
    );

    expect(screen.getByRole("status").textContent).toContain("Deck saved.");
    fireEvent.click(screen.getByRole("button", { name: "Dismiss notification" }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("uses an alert role for errors", () => {
    render(
      <Toast
        toast={{ id: 2, kind: "error", text: "Could not save." }}
        onDismiss={() => undefined}
      />,
    );

    expect(screen.getByRole("alert").textContent).toContain("Could not save.");
  });
});
