import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import { Tabs, type TabDefinition } from "./Tabs";

type Value = "primary" | "advanced";

const tabs: ReadonlyArray<TabDefinition<Value>> = [
  { value: "primary", label: "Primary", panel: <div>Primary panel</div> },
  {
    value: "advanced",
    label: "Advanced tool",
    hiddenFromTabList: true,
    panel: <div>Advanced panel</div>,
  },
];

function Harness() {
  const [value, setValue] = useState<Value>("primary");
  return (
    <>
      <button type="button" onClick={() => setValue("advanced")}>
        Open advanced tool
      </button>
      <Tabs value={value} onValueChange={setValue} tabs={tabs} ariaLabel="Sections" />
    </>
  );
}

describe("Tabs hidden panels", () => {
  it("keeps advanced menu destinations out of the tab strip and mounts them only when selected", () => {
    render(<Harness />);

    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Primary"]);
    expect(screen.queryByText("Advanced panel")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Open advanced tool" }));

    expect(screen.getByRole("tabpanel", { name: "Advanced tool" })).toBeTruthy();
    expect(screen.getByText("Advanced panel")).toBeTruthy();
    expect(screen.getAllByRole("tab")).toHaveLength(1);
    expect(screen.getByRole("tab", { name: "Primary" }).getAttribute("tabindex")).toBe("0");
  });
});
