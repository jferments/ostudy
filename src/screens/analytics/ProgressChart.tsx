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
import { useAnimationsEnabled } from "../../ui/hooks/useAnimationsEnabled";
import { buildDifficultyTimelineSeries } from "./difficultySeries";

ChartJS.register(CategoryScale, LinearScale, LineController, LineElement, PointElement, Tooltip, Legend);

interface ProgressChartProps {
  buckets: TimelineBucket[];
  range: AnalyticsRange;
  reduceMotion?: boolean;
}

const compactDateFormatter = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
const monthFormatter = new Intl.DateTimeFormat(undefined, { month: "short", year: "2-digit" });

type ChartPoint = number | null;
type LineChartData = ChartData<"line", ChartPoint[], string>;

export function ProgressChart({ buckets, range, reduceMotion = false }: ProgressChartProps) {
  const animationsEnabled = useAnimationsEnabled();
  const motionDisabled = reduceMotion || !animationsEnabled;
  const colors = readChartColors();
  const labels = useMemo(
    () => buckets.map((bucket) => formatBucketLabel(bucket.startAt, range)),
    [buckets, range],
  );
  const series = useMemo(() => buildDifficultyTimelineSeries(buckets), [buckets]);
  const data = useMemo<LineChartData>(() => {
    const datasets: ChartDataset<"line", ChartPoint[]>[] = [
      lineDataset("Cards reviewed", series.reviewed, colors.accent, buckets),
      lineDataset("Wrong", series.wrong, colors.wrong, buckets),
      lineDataset("Hard", series.hard, colors.hard, buckets),
      lineDataset("Medium", series.medium, colors.medium, buckets),
      lineDataset("Easy", series.easy, colors.easy, buckets),
    ];
    return { labels, datasets };
  }, [buckets, colors, labels, series]);
  const options = useMemo<ChartOptions<"line">>(
    () => ({
      responsive: true,
      maintainAspectRatio: false,
      animation: motionDisabled ? false : { duration: 260 },
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
                String(bucket.newCards) + " new card" + (bucket.newCards === 1 ? "" : "s"),
                "Remembered: " + formatPercent(bucket.correctRate),
                "Average difficulty: " + formatConfidence(bucket.averageConfidence),
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
          title: { display: true, text: "Reviews", color: colors.muted },
        },
      },
    }),
    [buckets, colors, motionDisabled],
  );

  return (
    <div className="progress-chart-canvas">
      <Chart type="line" data={data} options={options} role="img" aria-label={buildAriaLabel(buckets)} />
    </div>
  );
}

function lineDataset(
  label: string,
  values: number[],
  color: string,
  buckets: TimelineBucket[],
): ChartDataset<"line", ChartPoint[]> {
  return {
    label,
    data: values,
    borderColor: color,
    backgroundColor: color,
    pointBackgroundColor: color,
    pointBorderColor: buckets.map((bucket) =>
      bucket.containsHighlightedSession ? color : "transparent",
    ),
    pointBorderWidth: buckets.map((bucket) => (bucket.containsHighlightedSession ? 3 : 0)),
    pointRadius: buckets.map((bucket) => (bucket.containsHighlightedSession ? 5 : 3)),
    spanGaps: false,
    tension: 0.24,
  };
}

interface ChartColors {
  accent: string;
  wrong: string;
  hard: string;
  medium: string;
  easy: string;
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
    wrong: value("--confidence-wrong", "#b4233b"),
    hard: value("--confidence-hard", "#c45d16"),
    medium: value("--confidence-medium", "#d8a000"),
    easy: value("--confidence-easy", "#197a46"),
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
  const sum = (read: (bucket: TimelineBucket) => number) =>
    active.reduce((total, bucket) => total + read(bucket), 0);
  const reviews = sum((bucket) => bucket.reviewCount);
  const wrong = sum((bucket) => bucket.wrongCount);
  const hard = sum((bucket) => bucket.hardCount);
  const medium = sum((bucket) => bucket.mediumCount);
  const easy = sum((bucket) => bucket.easyCount);
  return `Learning progress line chart with ${reviews} reviews: ${wrong} Wrong, ${hard} Hard, ${medium} Medium, and ${easy} Easy.`;
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
