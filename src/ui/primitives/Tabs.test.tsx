import { fireEvent, render, screen } from "@testing-library/react";
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

  it("supports roving arrow, Home, and End keyboard navigation", () => {
    render(<Harness />);
    const first = screen.getByRole("tab", { name: "First" });
    first.focus();
    fireEvent.keyDown(first, { key: "ArrowRight" });
    expect(screen.getByRole("tab", { name: "Second" }).getAttribute("aria-selected")).toBe("true");

    fireEvent.keyDown(screen.getByRole("tab", { name: "Second" }), { key: "End" });
    expect(screen.getByRole("tab", { name: "Third" }).getAttribute("aria-selected")).toBe("true");

    fireEvent.keyDown(screen.getByRole("tab", { name: "Third" }), { key: "Home" });
    expect(screen.getByRole("tab", { name: "First" }).getAttribute("aria-selected")).toBe("true");
  });
});
