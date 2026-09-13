import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { FontControl, NumberControl, RangeControl, SelectControl } from "./SettingsControls";

describe("shared Settings controls", () => {
  it("switches from a bundled font to an editable installed family", () => {
    function Harness() {
      const [value, setValue] = useState("ibm-plex-sans");
      return <FontControl label="Interface font" value={value} onChange={setValue} />;
    }
    render(<Harness />);
    fireEvent.change(screen.getByRole("combobox", { name: "Interface font" }), { target: { value: "custom" } });
    const field = screen.getByRole("textbox", { name: "Font family name" });
    expect((field as HTMLInputElement).value).toBe("Arial");
    fireEvent.change(field, { target: { value: "Atkinson Hyperlegible" } });
    expect((field as HTMLInputElement).value).toBe("Atkinson Hyperlegible");
  });

  it("exposes slider value and bounded keyboard adjustment", async () => {
    function Harness() {
      const [value, setValue] = useState(90);
      return <RangeControl label="Desired retention" min={80} max={97} step={1} value={value} suffix="%" onChange={setValue} />;
    }
    render(<Harness />);
    const slider = screen.getByRole("slider", { name: "Desired retention" });
    act(() => slider.focus());
    fireEvent.keyDown(slider, { key: "ArrowRight" });
    await waitFor(() => expect(slider.getAttribute("aria-valuenow")).toBe("91"));
    expect(screen.getByText("91%")).toBeTruthy();
    fireEvent.keyDown(slider, { key: "End" });
    await waitFor(() => expect(slider.getAttribute("aria-valuenow")).toBe("97"));
  });

  it("retains numeric workload bounds and native choice values", () => {
    const update = vi.fn();
    render(<><NumberControl label="Daily limit" value={20} onChange={update} />
      <SelectControl label="Motion" value="full" options={[{ value: "full", label: "Full motion" }, { value: "none", label: "No motion" }]} onChange={update} /></>);
    fireEvent.change(screen.getByRole("spinbutton", { name: "Daily limit" }), { target: { value: "10001" } });
    expect(update).toHaveBeenLastCalledWith(10000);
    fireEvent.change(screen.getByRole("combobox", { name: "Motion" }), { target: { value: "none" } });
    expect(update).toHaveBeenLastCalledWith("none");
  });
});
