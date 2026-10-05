import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { FontControl, HourControl, NumberControl, RangeControl, SelectControl } from "./SettingsControls";

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

  it("offers every hour from 12 a.m. to 11 p.m. and reports the chosen hour as a number", () => {
    const update = vi.fn();
    render(<HourControl label="New day starts at" value={4}
      description="Reviews before this time count toward the previous day." onChange={update} />);
    const select = screen.getByRole("combobox", { name: "New day starts at" }) as HTMLSelectElement;
    const labels = Array.from(select.options, (option) => option.textContent);
    expect(labels).toEqual([
      "12 a.m.", "1 a.m.", "2 a.m.", "3 a.m.", "4 a.m.", "5 a.m.", "6 a.m.", "7 a.m.",
      "8 a.m.", "9 a.m.", "10 a.m.", "11 a.m.", "12 p.m.", "1 p.m.", "2 p.m.", "3 p.m.",
      "4 p.m.", "5 p.m.", "6 p.m.", "7 p.m.", "8 p.m.", "9 p.m.", "10 p.m.", "11 p.m.",
    ]);
    expect(select.selectedOptions[0]?.textContent).toBe("4 a.m.");
    expect(select.getAttribute("aria-describedby")).toBeTruthy();
    expect(screen.getByText("Reviews before this time count toward the previous day.").id)
      .toBe(select.getAttribute("aria-describedby"));
    fireEvent.change(select, { target: { value: "0" } });
    expect(update).toHaveBeenLastCalledWith(0);
    fireEvent.change(select, { target: { value: "23" } });
    expect(update).toHaveBeenLastCalledWith(23);
  });
});
