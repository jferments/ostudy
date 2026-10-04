import { readFileSync } from "node:fs";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { SegmentedControl, type SegmentedOptions } from "./SegmentedControl";

const bridge = readFileSync("src/ui/tokens/generated/tailwind-theme.css", "utf8");
const foundations = readFileSync("src/ui/foundations/index.css", "utf8");
const reference = readFileSync("src/ui/tokens/generated/reference.css", "utf8");

/** Follow a min-h-* spacing utility through the Tailwind bridge and the foundation variables to its value. */
function minimumHeight(element: HTMLElement): string {
  const step = element.className.split(/\s+/u).map((name) => /^tw:min-h-([a-z0-9]+)$/u.exec(name)?.[1]).find(Boolean);
  if (!step) throw new Error(`no minimum height on ${element.className}`);
  let value = new RegExp(`--spacing-${step}: ([^;]+);`, "u").exec(bridge)?.[1];
  for (const source of [foundations, reference]) {
    const variable = value ? /^var\((--[a-z0-9-]+)\)$/u.exec(value)?.[1] : undefined;
    if (variable) value = new RegExp(`${variable}: ([^;]+);`, "u").exec(source)?.[1];
  }
  if (!value) throw new Error(`min-h-${step} does not resolve`);
  return value;
}

const ranges = [
  { value: "7d", label: "7 days" }, { value: "30d", label: "30 days" }, { value: "90d", label: "90 days" },
  { value: "1y", label: "1 year" }, { value: "all", label: "All time" },
] as const;

describe("SegmentedControl", () => {
  it("keeps every option's own box at the touch target and lets a long label wrap instead of clipping", () => {
    render(<SegmentedControl label="Time range" value="7d" options={ranges} onChange={vi.fn()} />);
    for (const radio of screen.getAllByRole("radio")) {
      expect(minimumHeight(radio)).toBe("max(44px, 2.75rem)");
      for (const element of [radio, ...radio.querySelectorAll<HTMLElement>("*")]) {
        const classes = element.className.split(/\s+/u);
        expect(classes.filter((name) => /^tw:(?:h-|truncate$|whitespace-nowrap$|overflow-hidden$|text-ellipsis$|line-clamp-)/u.test(name)))
          .toEqual([]);
      }
    }
  });

  it("checks nothing while no option applies, and Tab still enters at the first option", async () => {
    const onChange = vi.fn();
    function Harness() {
      const [value, setValue] = useState<(typeof ranges)[number]["value"] | null>(null);
      return <SegmentedControl label="Time range" value={value} options={ranges} onChange={(next) => { setValue(next); onChange(next); }} />;
    }
    render(<Harness />);
    const group = screen.getByRole("radiogroup", { name: "Time range" });
    expect(within(group).queryByRole("radio", { checked: true })).toBeNull();
    const first = within(group).getByRole("radio", { name: "7 days" });
    expect(first.getAttribute("tabindex")).toBe("0");

    act(() => first.focus());
    fireEvent.keyDown(first, { key: "End" });
    await waitFor(() => {
      expect(within(group).getByRole("radio", { name: "All time", checked: true })).toBe(document.activeElement);
    });
    expect(onChange).toHaveBeenLastCalledWith("all");
  });

  it("skips a disabled option and keeps the accessible name to the visible label", async () => {
    const onChange = vi.fn();
    function Harness() {
      const [value, setValue] = useState<"day" | "week" | "month">("day");
      return <SegmentedControl label="Group by" value={value} onChange={(next) => { setValue(next); onChange(next); }} options={[
        { value: "day", label: "Day" }, { value: "week", label: "Week", disabled: true }, { value: "month", label: "Month" },
      ]} />;
    }
    render(<Harness />);
    const day = screen.getByRole("radio", { name: "Day" });
    expect(screen.getByRole("radio", { name: "Week" }).hasAttribute("data-disabled")).toBe(true);
    act(() => day.focus());
    fireEvent.keyDown(day, { key: "ArrowRight" });
    await waitFor(() => expect(onChange).toHaveBeenLastCalledWith("month"));
    expect(screen.getByRole("radio", { name: "Month", checked: true })).toBe(document.activeElement);
  });

  it("accepts two to five options", () => {
    const one = [{ value: "a", label: "A" }] as const;
    const six = [...ranges, { value: "custom", label: "Custom" }] as const;
    // @ts-expect-error One option is not a choice.
    const tooFew: SegmentedOptions<"a"> = one;
    // @ts-expect-error Six options no longer fit; use a select or a menu.
    const tooMany: SegmentedOptions<string> = six;
    expect([tooFew.length, tooMany.length]).toEqual([1, 6]);
  });
});
