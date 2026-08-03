import { useMemo } from "react";
import {
  BarElement,
  CategoryScale,
  Chart as ChartJS,
  Legend,
  LinearScale,
  LineElement,
  PointElement,
  Tooltip,
  type ChartData,
  type ChartDataset,
  type ChartOptions,
} from "chart.js";
import { Chart } from "react-chartjs-2";

import type { AnalyticsRange, TimelineBucket } from "../../analytics-ipc";

ChartJS.register(
  CategoryScale,
  LinearScale,
  BarElement,
  LineElement,
  PointElement,
  Tooltip,
  Legend,
);

interface ProgressChartProps {
  buckets: TimelineBucket[];
  range: AnalyticsRange;
  metric: "cards" | "time";
  reduceMotion: boolean;
}

const compactDateFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
});
const monthFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  year: "2-digit",
});

type ChartPoint = number | null;
type BarChartData = ChartData<"bar", ChartPoint[], string>;

export function ProgressChart({
  buckets,
  range,
  metric,
  reduceMotion,
}: ProgressChartProps) {
  const colors = readChartColors();
  const labels = useMemo(
    () => buckets.map((bucket) => formatBucketLabel(bucket.startAt, range)),
    [buckets, range],
  );
  const data = useMemo<BarChartData>(() => {
    const volumeDataset: ChartDataset<"bar", ChartPoint[]> = {
      type: "bar",
      label: metric === "cards" ? "Cards practiced" : "Focused study minutes",
      data: buckets.map((bucket) =>
        metric === "cards"
          ? bucket.cardsPracticed
          : Math.round((bucket.studyTimeMs / 60_000) * 10) / 10,
      ),
      backgroundColor: buckets.map((bucket) =>
        bucket.containsHighlightedSession ? colors.warning : colors.accentSoft,
      ),
      borderColor: colors.accent,
      borderWidth: 1,
      borderRadius: 5,
      yAxisID: "volume",
      order: 2,
    };
    const recallDataset: ChartDataset<"line", ChartPoint[]> = {
      type: "line",
      label: "Remembered",
      data: buckets.map((bucket) =>
        bucket.correctRate === null ? null : Math.round(bucket.correctRate * 1000) / 10,
      ),
      borderColor: colors.success,
      backgroundColor: colors.success,
      pointBackgroundColor: buckets.map((bucket) =>
        bucket.containsHighlightedSession ? colors.warning : colors.success,
      ),
      pointBorderColor: colors.surface,
      pointBorderWidth: 2,
      pointRadius: buckets.map((bucket) =>
        bucket.containsHighlightedSession ? 5 : bucket.correctRate === null ? 0 : 3,
      ),
      spanGaps: false,
      tension: 0.24,
      yAxisID: "recall",
      order: 1,
    };

    // Chart.js supports mixed datasets when the outer chart has a default type
    // and each non-default dataset declares its own type. The React adapter's
    // generic is intentionally anchored to the outer bar chart, so the mixed
    // dataset array is narrowed only at this boundary.
    const datasets = [volumeDataset, recallDataset] as unknown as BarChartData["datasets"];
    return { labels, datasets };
  }, [buckets, colors, labels, metric]);
  const options = useMemo<ChartOptions<"bar">>(
    () => ({
      responsive: true,
      maintainAspectRatio: false,
      animation: reduceMotion ? false : { duration: 260 },
      interaction: { mode: "index", intersect: false },
      plugins: {
        legend: {
          position: "top",
          align: "start",
          labels: {
            color: colors.textSoft,
            usePointStyle: true,
            boxWidth: 9,
          },
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
                `${bucket.reviewCount} review${bucket.reviewCount === 1 ? "" : "s"}`,
                `${bucket.newCards} new card${bucket.newCards === 1 ? "" : "s"}`,
                `Confidence: ${formatConfidence(bucket.averageConfidence)}`,
                `Focused time: ${formatDuration(bucket.studyTimeMs)}`,
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
        volume: {
          beginAtZero: true,
          position: "left",
          grid: { color: colors.grid },
          ticks: { color: colors.muted, precision: metric === "cards" ? 0 : 1 },
          border: { color: colors.border },
          title: {
            display: true,
            text: metric === "cards" ? "Unique cards" : "Minutes",
            color: colors.muted,
          },
        },
        recall: {
          beginAtZero: true,
          max: 100,
          position: "right",
          grid: { drawOnChartArea: false },
          ticks: {
            color: colors.muted,
            callback: (value) => `${value}%`,
          },
          border: { color: colors.border },
          title: { display: true, text: "Remembered", color: colors.muted },
        },
      },
    }),
    [buckets, colors, metric, reduceMotion],
  );

  const ariaLabel = buildAriaLabel(buckets, metric);
  return (
    <div className="progress-chart-canvas">
      <Chart
        type="bar"
        data={data}
        options={options}
        role="img"
        aria-label={ariaLabel}
      />
    </div>
  );
}

interface ChartColors {
  accent: string;
  accentSoft: string;
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
  const value = (name: string, fallback: string) =>
    style.getPropertyValue(name).trim() || fallback;
  return {
    accent: value("--accent", "#78a9ff"),
    accentSoft: value("--accent-soft", "rgba(120, 169, 255, 0.28)"),
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

function buildAriaLabel(
  buckets: TimelineBucket[],
  metric: "cards" | "time",
): string {
  const active = buckets.filter((bucket) => bucket.reviewCount > 0);
  if (active.length === 0) {
    return "Learning progress chart. No reviews occurred in the selected range.";
  }
  const reviews = active.reduce((total, bucket) => total + bucket.reviewCount, 0);
  const cards = active.reduce((total, bucket) => total + bucket.cardsPracticed, 0);
  const time = active.reduce((total, bucket) => total + bucket.studyTimeMs, 0);
  return `Learning progress chart with ${reviews} reviews across ${cards} bucket-level unique cards and ${formatDuration(time)} of focused study time. Bars show ${metric === "cards" ? "unique cards practiced" : "focused study minutes"}; the line shows remembered-answer rate.`;
}

function formatConfidence(value: number | null): string {
  return value === null ? "no data" : `${value.toFixed(1)} / 3`;
}

function formatDuration(value: number): string {
  if (value < 60_000) return `${Math.round(value / 1_000)} sec`;
  if (value < 3_600_000) return `${Math.round(value / 60_000)} min`;
  const hours = value / 3_600_000;
  return `${hours.toFixed(hours < 10 ? 1 : 0)} hr`;
}
