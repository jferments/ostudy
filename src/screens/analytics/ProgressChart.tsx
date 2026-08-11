import { useMemo } from "react";
import {
  CategoryScale,
  Chart as ChartJS,
  Legend,
  LinearScale,
  LineController,
  LineElement,
  PointElement,
  Tooltip,
  type ChartData,
  type ChartDataset,
  type ChartOptions,
} from "chart.js";
import { Chart } from "react-chartjs-2";

import type { AnalyticsRange, TimelineBucket } from "../../ipc/analytics";

ChartJS.register(CategoryScale, LinearScale, LineController, LineElement, PointElement, Tooltip, Legend);

interface ProgressChartProps {
  buckets: TimelineBucket[];
  range: AnalyticsRange;
  reduceMotion: boolean;
}

const compactDateFormatter = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
const monthFormatter = new Intl.DateTimeFormat(undefined, { month: "short", year: "2-digit" });

type ChartPoint = number | null;
type LineChartData = ChartData<"line", ChartPoint[], string>;

export function ProgressChart({ buckets, range, reduceMotion }: ProgressChartProps) {
  const colors = readChartColors();
  const labels = useMemo(
    () => buckets.map((bucket) => formatBucketLabel(bucket.startAt, range)),
    [buckets, range],
  );
  const data = useMemo<LineChartData>(() => {
    const practiced: ChartDataset<"line", ChartPoint[]> = {
      label: "Cards practiced",
      data: buckets.map((bucket) => bucket.cardsPracticed),
      borderColor: colors.accent,
      backgroundColor: colors.accent,
      pointBackgroundColor: buckets.map((bucket) =>
        bucket.containsHighlightedSession ? colors.warning : colors.accent,
      ),
      pointBorderColor: colors.surface,
      pointBorderWidth: 2,
      pointRadius: buckets.map((bucket) => (bucket.containsHighlightedSession ? 5 : 3)),
      spanGaps: false,
      tension: 0.24,
    };
    const remembered: ChartDataset<"line", ChartPoint[]> = {
      label: "Remembered",
      data: buckets.map((bucket) => bucket.rememberedCount),
      borderColor: colors.success,
      backgroundColor: colors.success,
      pointBackgroundColor: buckets.map((bucket) =>
        bucket.containsHighlightedSession ? colors.warning : colors.success,
      ),
      pointBorderColor: colors.surface,
      pointBorderWidth: 2,
      pointRadius: buckets.map((bucket) => (bucket.containsHighlightedSession ? 5 : 3)),
      spanGaps: false,
      tension: 0.24,
    };
    return { labels, datasets: [practiced, remembered] };
  }, [buckets, colors, labels]);
  const options = useMemo<ChartOptions<"line">>(
    () => ({
      responsive: true,
      maintainAspectRatio: false,
      animation: reduceMotion ? false : { duration: 260 },
      interaction: { mode: "index", intersect: false },
      plugins: {
        legend: {
          position: "top",
          align: "start",
          labels: { color: colors.textSoft, usePointStyle: true, boxWidth: 9 },
        },
        tooltip: {
          backgroundColor: colors.surfaceRaised,
          titleColor: colors.text,
          bodyColor: colors.textSoft,
          borderColor: colors.border,
          borderWidth: 1,
          callbacks: {
            afterBody(items) {
              const bucket = buckets[items[0]?.dataIndex ?? 0];
              if (!bucket) return [];
              return [
                String(bucket.reviewCount) + " review" + (bucket.reviewCount === 1 ? "" : "s"),
                String(bucket.newCards) + " new card" + (bucket.newCards === 1 ? "" : "s"),
                "Recall rate: " + formatPercent(bucket.correctRate),
                "Confidence: " + formatConfidence(bucket.averageConfidence),
                "Focused time: " + formatDuration(bucket.studyTimeMs),
              ];
            },
          },
        },
      },
      scales: {
        x: {
          grid: { display: false },
          ticks: { color: colors.muted, maxRotation: 0, autoSkip: true },
          border: { color: colors.border },
        },
        y: {
          beginAtZero: true,
          grid: { color: colors.grid },
          ticks: { color: colors.muted, precision: 0 },
          border: { color: colors.border },
          title: { display: true, text: "Cards", color: colors.muted },
        },
      },
    }),
    [buckets, colors, reduceMotion],
  );

  return (
    <div className="progress-chart-canvas">
      <Chart type="line" data={data} options={options} role="img" aria-label={buildAriaLabel(buckets)} />
    </div>
  );
}

interface ChartColors {
  accent: string;
  success: string;
  warning: string;
  surface: string;
  surfaceRaised: string;
  text: string;
  textSoft: string;
  muted: string;
  border: string;
  grid: string;
}

function readChartColors(): ChartColors {
  const style = getComputedStyle(document.documentElement);
  const value = (name: string, fallback: string) => style.getPropertyValue(name).trim() || fallback;
  return {
    accent: value("--accent", "#78a9ff"),
    success: value("--success", "#55d79a"),
    warning: value("--warning", "#f5c56b"),
    surface: value("--surface", "#121a28"),
    surfaceRaised: value("--surface-raised", "#182235"),
    text: value("--text", "#edf3fb"),
    textSoft: value("--text-soft", "#b6c1d2"),
    muted: value("--muted", "#7f8da4"),
    border: value("--border", "#2b3950"),
    grid: value("--border", "#2b3950"),
  };
}

function formatBucketLabel(value: number, range: AnalyticsRange): string {
  const date = new Date(value);
  return range === "all" ? monthFormatter.format(date) : compactDateFormatter.format(date);
}

function buildAriaLabel(buckets: TimelineBucket[]): string {
  const active = buckets.filter((bucket) => bucket.reviewCount > 0);
  if (active.length === 0) return "Learning progress chart. No reviews occurred in the selected range.";
  const reviews = active.reduce((total, bucket) => total + bucket.reviewCount, 0);
  const practiced = active.reduce((total, bucket) => total + bucket.cardsPracticed, 0);
  const remembered = active.reduce((total, bucket) => total + bucket.rememberedCount, 0);
  return (
    "Learning progress line chart with " +
    reviews +
    " reviews. One line shows " +
    practiced +
    " bucket-level cards practiced and the second shows " +
    remembered +
    " remembered answers."
  );
}

function formatPercent(value: number | null): string {
  return value === null ? "no data" : String(Math.round(value * 100)) + "%";
}

function formatConfidence(value: number | null): string {
  return value === null ? "no data" : value.toFixed(1) + " / 3";
}

function formatDuration(value: number): string {
  if (value < 60_000) return String(Math.round(value / 1_000)) + " sec";
  if (value < 3_600_000) return String(Math.round(value / 60_000)) + " min";
  const hours = value / 3_600_000;
  return hours.toFixed(hours < 10 ? 1 : 0) + " hr";
}
