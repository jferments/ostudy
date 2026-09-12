import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import { Tabs } from "./Tabs";

function Harness() {
  const [value, setValue] = useState<"first" | "second" | "third">("first");
  return (
    <Tabs
      value={value}
      onValueChange={setValue}
      ariaLabel="Example sections"
      tabs={[
        { value: "first", label: "First", panel: <p>First panel</p> },
        { value: "second", label: "Second", panel: <p>Second panel</p> },
        { value: "third", label: "Third", panel: <p>Third panel</p> },
      ]}
    />
  );
}

async function expectActiveTab(name: string, panel: string) {
  // Base UI schedules composite focus. Assert the user-visible result, not
  // synchronous implementation timing immediately after dispatching keydown.
  await waitFor(() => {
    const tab = screen.getByRole("tab", { name });
    expect(document.activeElement).toBe(tab);
    expect(tab.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tabpanel").textContent).toContain(panel);
  });
}

describe("Tabs", () => {
  it("associates the active tab and panel with accessible tab semantics", () => {
    render(<Harness />);
    const first = screen.getByRole("tab", { name: "First" });
    expect(first.getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tabpanel").textContent).toContain("First panel");

    fireEvent.click(screen.getByRole("tab", { name: "Second" }));
    expect(screen.getByRole("tab", { name: "Second" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tabpanel").textContent).toContain("Second panel");
  });

  it("supports roving arrow, Home, End and wrapping keyboard navigation", async () => {
    render(<Harness />);
    const first = screen.getByRole("tab", { name: "First" });
    act(() => first.focus());
    fireEvent.keyDown(first, { key: "ArrowRight" });
    await expectActiveTab("Second", "Second panel");

    fireEvent.keyDown(screen.getByRole("tab", { name: "Second" }), { key: "End" });
    await expectActiveTab("Third", "Third panel");

    fireEvent.keyDown(screen.getByRole("tab", { name: "Third" }), { key: "Home" });
    await expectActiveTab("First", "First panel");

    fireEvent.keyDown(first, { key: "ArrowLeft" });
    await expectActiveTab("Third", "Third panel");
  });
});
