import { act, render, waitFor } from "@testing-library/react";
import type { ChartData, ChartOptions } from "chart.js";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { TimelineBucket } from "../../ipc/analytics";

type Drawn = { data: ChartData<"line">; options: ChartOptions<"line"> };
const drawn = vi.hoisted(() => [] as Drawn[]);
vi.mock("react-chartjs-2", () => ({
  Chart: (props: Drawn) => {
    drawn.push(props);
    return null;
  },
}));

import { ProgressChart } from "./ProgressChart";

function bucket(startAt: number): TimelineBucket {
  return {
    startLocalDate: "2026-08-11", endLocalDate: "2026-08-11", startAt, endAt: startAt + 1, cardsPracticed: 8, reviewCount: 10,
    newCards: 2, rememberedCount: 8, wrongCount: 2, hardCount: 1, mediumCount: 4, easyCount: 3, correctRate: 0.8,
    averageConfidence: 1.8, averageResponseTimeMs: 4000, studyTimeMs: 40_000, sessionCount: 1, containsHighlightedSession: false,
    retentionReviewCount: 5, retentionRememberedCount: 4, trueRetention: 0.8,
  };
}

const ratings = { "confidence-0": "#b5392e", "confidence-1": "#b04e1e", "confidence-2": "#8a5e00", "confidence-3": "#1c7a4b" };
const shared = { "text-primary": "#15181d", "text-secondary": "#424955", "text-muted": "#5c6471", "chart-grid": "#e1e4e9" };
const palette = (first: string) => ({ "chart-1": first, "chart-2": "#7fd4ff", "chart-3": "#f2c14e", "chart-4": "#8a3fb8", "chart-5": "#ff9f6b", "chart-6": "#c9a7ff" });
const block = (theme: string, values: Record<string, string>) =>
  `:root[data-theme='${theme}'] { ${Object.entries(values).map(([name, value]) => `--ostudy-color-${name}: ${value};`).join(" ")} }`;

describe("ProgressChart theme", () => {
  const root = document.documentElement;
  const style = document.createElement("style");
  // In "clash", the first palette color is the Easy color, as Console's is.
  style.textContent = [
    block("plain", { ...palette("#3346d3"), ...ratings, ...shared }),
    block("clash", { ...palette(ratings["confidence-3"]), ...ratings, ...shared, "chart-grid": "#1a2e21" }),
  ].join("\n");

  afterEach(() => {
    style.remove();
    delete root.dataset.theme;
    drawn.length = 0;
  });

  it("draws the total and the four ratings in theme colors and redraws them on a theme switch", async () => {
    document.head.append(style);
    root.dataset.theme = "plain";
    render(<ProgressChart buckets={[bucket(1), bucket(86_400_001)]} range="7d" />);

    const first = drawn.at(-1)!;
    expect(first.data.datasets.map((dataset) => [dataset.label, dataset.borderColor])).toEqual([
      ["Cards reviewed", "#3346d3"], ["Wrong", ratings["confidence-0"]], ["Hard", ratings["confidence-1"]],
      ["Medium", ratings["confidence-2"]], ["Easy", ratings["confidence-3"]],
    ]);
    const dashes = first.data.datasets.map((dataset) => (dataset.borderDash as number[]).join(","));
    expect(dashes.filter(Boolean)).toHaveLength(2);
    expect(dashes[2]).not.toBe(dashes[3]);
    expect(first.options.scales?.y?.grid?.color).toBe(shared["chart-grid"]);
    expect(first.options.scales?.x?.ticks?.color).toBe(shared["text-muted"]);
    expect(first.options.plugins?.legend?.labels?.color).toBe(shared["text-secondary"]);

    act(() => { root.dataset.theme = "clash"; });
    await waitFor(() => expect(drawn.at(-1)!.options.scales?.y?.grid?.color).toBe("#1a2e21"));
    expect(drawn.at(-1)!.data.datasets[0]!.borderColor).toBe("#7fd4ff");
  });
});
