import type { Chart, LegendItem } from "chart.js";
import { useSyncExternalStore } from "react";

/**
 * The theme's chart colors and type, resolved to values a canvas can draw with. Canvas charts
 * cannot read CSS variables, so they take these instead of hard-coded colors; SVG and HTML
 * charts can use the same tokens as CSS variables directly.
 */
export interface ChartTheme {
  /** chart-1 to chart-6: the categorical palette, in order. */
  palette: readonly string[];
  /** Wrong, Hard, Medium and Easy, in the confidence colors of the rating buttons. */
  ratings: readonly [wrong: string, hard: string, medium: string, easy: string];
  text: string;
  secondaryText: string;
  mutedText: string;
  grid: string;
  tooltipBackground: string;
  tooltipBorder: string;
  /** Corner radius for tooltips, in CSS pixels. */
  radius: number;
  fontFamily: string;
  /** Type-scale steps in CSS pixels at the current interface scale. */
  fontSize: { caption: number; label: number; small: number };
}

type ReadProperty = (property: string) => string;

/** Reads the chart theme from the root element's computed style, or from the given reader. */
export function readChartTheme(read: ReadProperty = rootReader()): ChartTheme {
  // Browsers substitute var() in computed custom properties; follow any reference that is left.
  const value = (property: string, depth = 0): string => {
    const raw = read(property).trim();
    const reference = /^var\((--[a-z0-9-]+)\)$/iu.exec(raw)?.[1];
    return reference && depth < 8 ? value(reference, depth + 1) : raw;
  };
  const color = (token: string) => value(`--ostudy-color-${token}`);
  const rootSize = Number.parseFloat(read("font-size")) || 16;
  const length = (property: string, fallback: number) => toPixels(value(property), rootSize) ?? fallback;
  return {
    palette: [1, 2, 3, 4, 5, 6].map((index) => color(`chart-${index}`)),
    ratings: [color("confidence-0"), color("confidence-1"), color("confidence-2"), color("confidence-3")],
    text: color("text-primary"),
    secondaryText: color("text-secondary"),
    mutedText: color("text-muted"),
    grid: color("chart-grid"),
    tooltipBackground: color("surface-overlay"),
    tooltipBorder: color("border-subtle"),
    radius: length("--ostudy-radius-small", 8),
    fontFamily: read("font-family").trim() || "sans-serif",
    fontSize: {
      caption: length("--ostudy-font-size-caption", 12),
      label: length("--ostudy-font-size-label", 13),
      small: length("--ostudy-font-size-small", 14),
    },
  };
}

/**
 * The current chart theme. It is read again when the root's theme, contrast, interface scale or
 * interface font changes, so a drawn chart follows a theme switch.
 */
export function useChartTheme(): ChartTheme {
  return useSyncExternalStore(subscribe, currentTheme);
}

/**
 * `count` palette colors in palette order, skipping any that would be hard to tell apart from a
 * color already on the chart, such as a rating color, or from an earlier pick.
 */
export function distinctPaletteColors(theme: ChartTheme, count: number, inUse: readonly string[] = []): string[] {
  const taken = [...inUse];
  const picked: string[] = [];
  for (const color of theme.palette) {
    if (picked.length === count) break;
    if (taken.some((other) => colorDistance(color, other) < DISTINCT)) continue;
    picked.push(color);
    taken.push(color);
  }
  // A theme with too few distinct colors still gets a color for every series.
  for (const color of theme.palette) {
    if (picked.length === count) break;
    if (!picked.includes(color)) picked.push(color);
  }
  return picked;
}

/** A font for one step of the type scale, in the interface font. */
export function chartFont(theme: ChartTheme, step: keyof ChartTheme["fontSize"], weight?: number) {
  return { family: theme.fontFamily, size: theme.fontSize[step], ...(weight ? { weight } : {}) };
}

/** An axis with muted caption ticks and, unless turned off, chart grid lines. */
export function chartAxis(theme: ChartTheme, { grid = true, title }: { grid?: boolean; title?: string } = {}) {
  return {
    grid: { display: grid, color: theme.grid },
    border: { color: theme.grid },
    ticks: { color: theme.mutedText, font: chartFont(theme, "caption") },
    title: { display: Boolean(title), text: title ?? "", color: theme.mutedText, font: chartFont(theme, "caption") },
  };
}

/** A legend whose entries are short line samples in each series' color, width and dash. */
export function chartLegend(theme: ChartTheme) {
  return {
    labels: {
      color: theme.secondaryText,
      font: chartFont(theme, "label"),
      usePointStyle: true,
      pointStyleWidth: 22,
      padding: 14,
      generateLabels: (chart: Chart): LegendItem[] => chart.data.datasets.map((dataset, datasetIndex) => {
        const line = dataset as { borderColor?: unknown; borderWidth?: unknown; borderDash?: unknown };
        const color = typeof line.borderColor === "string" ? line.borderColor : theme.secondaryText;
        return {
          text: dataset.label ?? "",
          datasetIndex,
          hidden: !chart.isDatasetVisible(datasetIndex),
          fontColor: theme.secondaryText,
          fillStyle: color,
          strokeStyle: color,
          lineWidth: typeof line.borderWidth === "number" ? line.borderWidth : 2,
          lineDash: Array.isArray(line.borderDash) ? line.borderDash as number[] : [],
          pointStyle: "line",
        };
      }),
    },
  };
}

/** A tooltip on the overlay surface with the theme's text colors. */
export function chartTooltip(theme: ChartTheme) {
  return {
    backgroundColor: theme.tooltipBackground,
    borderColor: theme.tooltipBorder,
    borderWidth: 1,
    cornerRadius: theme.radius,
    padding: 10,
    titleColor: theme.text,
    titleFont: chartFont(theme, "small", 600),
    bodyColor: theme.secondaryText,
    bodyFont: chartFont(theme, "label"),
  };
}

/** Colors closer than this in OKLab read as the same series at chart line widths. */
const DISTINCT = 0.1;

function rootReader(): ReadProperty {
  const style = getComputedStyle(document.documentElement);
  return (property) => style.getPropertyValue(property);
}

function toPixels(value: string, rootSize: number): number | undefined {
  const match = /^\s*(-?[\d.]+)(px|rem)\s*$/u.exec(value);
  if (!match) return undefined;
  const amount = Number.parseFloat(match[1]!);
  return match[2] === "rem" ? amount * rootSize : amount;
}

let cached: { key: string; theme: ChartTheme } | undefined;

/** Reads the theme again only when an input to it changed, so other root styles cost nothing. */
function currentTheme(): ChartTheme {
  const root = document.documentElement;
  const key = [root.dataset.theme, root.dataset.contrast, root.style.getPropertyValue("--ui-scale"),
    root.style.getPropertyValue("--ui-font-family")].join("|");
  if (cached?.key !== key) cached = { key, theme: readChartTheme() };
  return cached.theme;
}

function subscribe(onChange: () => void): () => void {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "data-contrast", "style"] });
  return () => observer.disconnect();
}

/** Euclidean distance in OKLab between two #rrggbb colors; Infinity when either is not one. */
export function colorDistance(first: string, second: string): number {
  const a = oklab(first);
  const b = oklab(second);
  if (!a || !b) return Number.POSITIVE_INFINITY;
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

function oklab(color: string): [number, number, number] | undefined {
  const match = /^#([\da-f]{6})$/iu.exec(color.trim());
  if (!match) return undefined;
  const value = Number.parseInt(match[1]!, 16);
  const [r, g, b] = [value >> 16, (value >> 8) & 255, value & 255].map((channel) => {
    const c = channel / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}
