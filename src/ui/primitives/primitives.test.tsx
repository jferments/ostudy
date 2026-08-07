import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

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
    view.rerender(<LoadingState label="Loading decks…" />);
    expect(screen.getByRole("status").textContent).toContain("Loading decks");
  });

  it("exposes a keyboard-accessible segmented choice", () => {
    const onChange = vi.fn();
    render(
      <SegmentedControl
        label="Range"
        value="7d"
        options={[
          { value: "7d", label: "Week" },
          { value: "30d", label: "Month" },
        ]}
        onChange={onChange}
      />,
    );

    expect(screen.getByRole("radio", { name: "Week" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(screen.getByRole("radio", { name: "Month" }));
    expect(onChange).toHaveBeenCalledWith("30d");
  });
});
