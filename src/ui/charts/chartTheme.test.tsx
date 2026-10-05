import { readFileSync } from "node:fs";
import { act, render, screen, waitFor } from "@testing-library/react";
import type { Chart } from "chart.js";
import { afterEach, describe, expect, it } from "vitest";

import {
  chartAxis, chartLegend, chartTooltip, colorDistance, distinctPaletteColors, readChartTheme, useChartTheme,
  type ChartTheme,
} from "./chartTheme";

const THEMES = ["light", "dark", "synthwave", "console"] as const;

/** Reads tokens the way the browser resolves them for one theme, from the generated token files. */
function themeReader(theme: (typeof THEMES)[number], rootSize = 16) {
  const values = new Map<string, string>();
  for (const file of ["reference.css", `theme-${theme}.css`]) {
    const source = readFileSync(`src/ui/tokens/generated/${file}`, "utf8");
    for (const [, name, value] of source.matchAll(/(--ostudy-[a-z0-9-]+): ([^;]+);/gu)) values.set(name!, value!.trim());
  }
  const resolve = (value: string): string => value.replace(/var\((--[a-z0-9-]+)\)/gu, (_, name: string) => resolve(values.get(name) ?? ""));
  return (property: string) => {
    if (property === "font-size") return `${rootSize}px`;
    if (property === "font-family") return "\"IBM Plex Sans\", sans-serif";
    return resolve(values.get(property) ?? "");
  };
}

describe("chart theme", () => {
  it("resolves every color to a theme value and every size on the type scale at the interface scale", () => {
    for (const name of THEMES) {
      const theme = readChartTheme(themeReader(name, 20));
      const colors = [...theme.palette, ...theme.ratings, theme.text, theme.secondaryText, theme.mutedText, theme.grid,
        theme.tooltipBackground, theme.tooltipBorder];
      expect(colors.filter((color) => !/^#[\da-f]{6}$/iu.test(color)), name).toEqual([]);
      expect(theme.palette).toHaveLength(6);
      // caption, label and small are 0.75, 0.8125 and 0.875 rem at a 20 px root.
      expect(theme.fontSize).toEqual({ caption: 15, label: 16.25, small: 17.5 });
      expect(theme.fontFamily).toBe("\"IBM Plex Sans\", sans-serif");
    }
    expect(readChartTheme(themeReader("light")).radius).toBeGreaterThan(readChartTheme(themeReader("console")).radius);
  });

  it("keeps a total series clear of every rating color in all four themes", () => {
    for (const name of THEMES) {
      const theme = readChartTheme(themeReader(name));
      const [total] = distinctPaletteColors(theme, 1, theme.ratings);
      expect(theme.palette, name).toContain(total);
      for (const rating of theme.ratings) expect(colorDistance(total!, rating), `${name} ${rating}`).toBeGreaterThanOrEqual(0.1);
    }
    // Console's first chart color is its success green, which Easy already uses.
    const consoleTheme = readChartTheme(themeReader("console"));
    expect(consoleTheme.palette[0]).toBe(consoleTheme.ratings[3]);
    expect(distinctPaletteColors(consoleTheme, 1, consoleTheme.ratings)[0]).not.toBe(consoleTheme.palette[0]);
  });

  it("picks palette colors in order, skips near matches and still fills every series", () => {
    const theme = { palette: ["#ff0000", "#fe0606", "#00ff00", "#0000ff"] } as unknown as ChartTheme;
    expect(distinctPaletteColors(theme, 2)).toEqual(["#ff0000", "#00ff00"]);
    expect(distinctPaletteColors(theme, 2, ["#00fe02"])).toEqual(["#ff0000", "#0000ff"]);
    expect(distinctPaletteColors(theme, 4, ["#00fe02"])).toEqual(["#ff0000", "#0000ff", "#fe0606", "#00ff00"]);
  });

  it("draws axes, legend and tooltips in the theme's text colors and interface font", () => {
    const theme = readChartTheme(themeReader("dark"));
    const font = (size: number, weight?: number) => ({ family: theme.fontFamily, size, ...(weight ? { weight } : {}) });
    expect(chartAxis(theme, { title: "Reviews" })).toEqual({
      grid: { display: true, color: theme.grid },
      border: { color: theme.grid },
      ticks: { color: theme.mutedText, font: font(theme.fontSize.caption) },
      title: { display: true, text: "Reviews", color: theme.mutedText, font: font(theme.fontSize.caption) },
    });
    expect(chartLegend(theme).labels).toMatchObject({ color: theme.secondaryText, font: font(theme.fontSize.label) });
    expect(chartTooltip(theme)).toMatchObject({
      backgroundColor: theme.tooltipBackground, borderColor: theme.tooltipBorder, cornerRadius: theme.radius,
      titleColor: theme.text, titleFont: font(theme.fontSize.small, 600), bodyColor: theme.secondaryText,
    });
  });
});

describe("chartLegend", () => {
  it("draws each entry as a sample of its line, dash included, and keeps hidden series toggleable", () => {
    const theme = readChartTheme(themeReader("light"));
    const chart = {
      data: { datasets: [
        { label: "Wrong", borderColor: theme.ratings[0], borderWidth: 2 },
        { label: "Hard", borderColor: theme.ratings[1], borderWidth: 2, borderDash: [7, 4] },
      ] },
      isDatasetVisible: (index: number) => index === 0,
    } as unknown as Chart;
    expect(chartLegend(theme).labels.generateLabels(chart)).toEqual([
      expect.objectContaining({ text: "Wrong", datasetIndex: 0, hidden: false, strokeStyle: theme.ratings[0], lineWidth: 2, lineDash: [], pointStyle: "line" }),
      expect.objectContaining({ text: "Hard", datasetIndex: 1, hidden: true, strokeStyle: theme.ratings[1], lineWidth: 2, lineDash: [7, 4], pointStyle: "line" }),
    ]);
  });
});

describe("useChartTheme", () => {
  const root = document.documentElement;
  const style = document.createElement("style");
  style.textContent = ":root[data-theme='light'] { --ostudy-color-chart-grid: #e1e4e9; } :root[data-theme='dark'] { --ostudy-color-chart-grid: #252a33; }";

  afterEach(() => {
    style.remove();
    delete root.dataset.theme;
    root.removeAttribute("style");
  });

  it("follows a theme switch and ignores unrelated root styles", async () => {
    document.head.append(style);
    root.dataset.theme = "light";
    let renders = 0;
    function Grid() {
      renders += 1;
      return <p>{useChartTheme().grid}</p>;
    }
    render(<Grid />);
    expect(screen.getByText("#e1e4e9")).toBeTruthy();

    act(() => { root.dataset.theme = "dark"; });
    await waitFor(() => expect(screen.getByText("#252a33")).toBeTruthy());

    const settled = renders;
    act(() => root.style.setProperty("--android-safe-top", "24px"));
    await act(async () => { await Promise.resolve(); });
    expect(renders).toBe(settled);
  });
});
