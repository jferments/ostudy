import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Button } from "./Button";
import { EmptyState, LoadingState } from "./EmptyState";
import { FormField } from "./FormField";
import { InlineNotice } from "./InlineNotice";
import { Panel } from "./Panel";
import { SegmentedControl } from "./SegmentedControl";

describe("shared UI primitives", () => {
  it("associates a form label, description, and error with its input", () => {
    render(
      <FormField label="Deck name" description="Shown in navigation." error="Required" required>
        <input />
      </FormField>,
    );

    const input = screen.getByRole("textbox", { name: /Deck name/u });
    expect(input.getAttribute("aria-invalid")).toBe("true");
    const describedBy = input.getAttribute("aria-describedby") ?? "";
    expect(describedBy).toContain("description");
    expect(describedBy).toContain("error");
    expect(screen.getByRole("alert").textContent).toBe("Required");
  });

  it("owns destructive secondary button presentation without legacy classes", () => {
    render(<Button variant="dangerSecondary">Delete deck</Button>);

    const button = screen.getByRole("button", { name: "Delete deck" });
    expect(button.className).not.toContain("secondary-button");
    expect(button.className).not.toContain("danger-text");
  });

  it("renders semantic notices and surfaces", () => {
    render(
      <Panel title="Study settings" description="Adjust the session.">
        <InlineNotice kind="error">Could not save.</InlineNotice>
      </Panel>,
    );

    expect(screen.getByRole("heading", { name: "Study settings" })).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("Could not save.");
  });

  it("provides reusable empty and loading states", () => {
    const view = render(
      <EmptyState title="No decks" description="Create a deck to begin." actions={<button>Create</button>} />,
    );
    expect(screen.getByRole("heading", { name: "No decks" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create" })).toBeTruthy();
    view.rerender(<EmptyState compact description="No recent reviews yet." />);
    expect(screen.queryByRole("heading")).toBeNull();
    expect(screen.getByText("No recent reviews yet.")).toBeTruthy();
    view.rerender(<LoadingState label="Loading decks…" />);
    expect(screen.getByRole("status").textContent).toContain("Loading decks");
  });

  it("exposes radio semantics and roving arrow-key selection", () => {
    const onChange = vi.fn();
    render(
      <SegmentedControl
        label="Range"
        value="7d"
        options={[
          { value: "7d", label: "Week" },
          { value: "30d", label: "Month" },
          { value: "90d", label: "Quarter" },
        ]}
        onChange={onChange}
      />,
    );

    const week = screen.getByRole("radio", { name: "Week" });
    const month = screen.getByRole("radio", { name: "Month" });
    const quarter = screen.getByRole("radio", { name: "Quarter" });
    expect(screen.getByRole("radiogroup", { name: "Range" }).getAttribute("aria-orientation")).toBe(
      "horizontal",
    );
    expect(week.getAttribute("aria-checked")).toBe("true");
    expect(week.getAttribute("tabindex")).toBe("0");
    expect(month.getAttribute("tabindex")).toBe("-1");

    week.focus();
    fireEvent.keyDown(week, { key: "ArrowRight" });
    expect(document.activeElement).toBe(month);
    expect(onChange).toHaveBeenLastCalledWith("30d");

    fireEvent.keyDown(month, { key: "End" });
    expect(document.activeElement).toBe(quarter);
    expect(onChange).toHaveBeenLastCalledWith("90d");

    fireEvent.click(month);
    expect(onChange).toHaveBeenLastCalledWith("30d");
  });
});
