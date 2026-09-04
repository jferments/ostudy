import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import { Tabs, type TabDefinition } from "./Tabs";

type Value = "primary" | "advanced";

function AdvancedPanel() {
  const [draft, setDraft] = useState("");
  return (
    <label>
      Advanced draft
      <input value={draft} onChange={(event) => setDraft(event.target.value)} />
    </label>
  );
}

const tabs: ReadonlyArray<TabDefinition<Value>> = [
  { value: "primary", label: "Primary", panel: <div>Primary panel</div> },
  {
    value: "advanced",
    label: "Advanced tool",
    hiddenFromTabList: true,
    panel: <AdvancedPanel />,
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
  it("keeps advanced menu destinations out of the tab strip and preserves their draft after first use", () => {
    render(<Harness />);

    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Primary"]);
    expect(screen.queryByLabelText("Advanced draft")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Open advanced tool" }));
    const input = screen.getByLabelText("Advanced draft") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "custom template" } });

    fireEvent.click(screen.getByRole("tab", { name: "Primary" }));
    expect(screen.getAllByRole("tab")).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "Open advanced tool" }));
    expect((screen.getByLabelText("Advanced draft") as HTMLInputElement).value).toBe(
      "custom template",
    );
  });
});
