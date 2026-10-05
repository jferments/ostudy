import { useMemo } from "react";
import {
  CategoryScale, Chart as ChartJS, Legend, LinearScale, LineController, LineElement, PointElement, Tooltip,
  type ChartData, type ChartDataset, type ChartOptions,
} from "chart.js";
import { Chart } from "react-chartjs-2";
import type { AnalyticsRange, TimelineBucket } from "../../ipc/analytics";
import { chartAxis, chartLegend, chartTooltip, distinctPaletteColors, useChartTheme } from "../../ui/charts/chartTheme";
import { useAnimationsEnabled } from "../../ui/hooks/useAnimationsEnabled";
import { buildDifficultyTimelineSeries } from "./difficultySeries";
import { styles } from "./presentation";

ChartJS.register(CategoryScale, LinearScale, LineController, LineElement, PointElement, Tooltip, Legend);
interface ProgressChartProps { buckets: TimelineBucket[]; range: AnalyticsRange; }
const compactDateFormatter = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
const monthFormatter = new Intl.DateTimeFormat(undefined, { month: "short", year: "2-digit" });
type ChartPoint = number | null;
type LineChartData = ChartData<"line", ChartPoint[], string>;

export function ProgressChart({ buckets, range }: ProgressChartProps) {
  const animationsEnabled = useAnimationsEnabled();
  const motionDisabled = !animationsEnabled;
  const theme = useChartTheme();
  const labels = useMemo(() => buckets.map((bucket) => formatBucketLabel(bucket.startAt, range)), [buckets, range]);
  const series = useMemo(() => buildDifficultyTimelineSeries(buckets), [buckets]);
  const data = useMemo<LineChartData>(() => {
    const [wrong, hard, medium, easy] = theme.ratings;
    // The total takes the first palette color that no rating color resembles, and the middle
    // ratings are dashed, because Wrong and Hard are close in hue in several themes.
    const [reviewed = theme.palette[0] ?? ""] = distinctPaletteColors(theme, 1, theme.ratings);
    return { labels, datasets: [
      lineDataset("Cards reviewed", series.reviewed, reviewed, buckets, { width: 2.5 }),
      lineDataset("Wrong", series.wrong, wrong, buckets),
      lineDataset("Hard", series.hard, hard, buckets, { dash: [7, 4] }),
      lineDataset("Medium", series.medium, medium, buckets, { dash: [2, 3] }),
      lineDataset("Easy", series.easy, easy, buckets),
    ] };
  }, [buckets, labels, series, theme]);
  const options = useMemo<ChartOptions<"line">>(() => ({
    responsive: true, maintainAspectRatio: false, animation: motionDisabled ? false : { duration: 260 },
    interaction: { mode: "index", intersect: false },
    plugins: {
      legend: { position: "top", align: "start", ...chartLegend(theme) },
      tooltip: {
        ...chartTooltip(theme),
        callbacks: { afterBody(items) {
          const bucket = buckets[items[0]?.dataIndex ?? 0];
          if (!bucket) return [];
          return [String(bucket.newCards) + " new card" + (bucket.newCards === 1 ? "" : "s"),
            "Remembered: " + formatPercent(bucket.correctRate), "Average difficulty: " + formatConfidence(bucket.averageConfidence),
            "Focused time: " + formatDuration(bucket.studyTimeMs)];
        } },
      },
    },
    scales: {
      x: { ...chartAxis(theme, { grid: false }), ticks: { ...chartAxis(theme).ticks, maxRotation: 0, autoSkip: true } },
      y: { ...chartAxis(theme, { title: "Reviews" }), beginAtZero: true, ticks: { ...chartAxis(theme).ticks, precision: 0 } },
    },
  }), [buckets, motionDisabled, theme]);
  return <div className={styles.progressChartCanvas}>
    <Chart type="line" data={data} options={options} role="img" aria-label={buildAriaLabel(buckets)} />
  </div>;
}
function lineDataset(label: string, values: number[], color: string, buckets: TimelineBucket[],
  { width = 2, dash = [] }: { width?: number; dash?: number[] } = {}): ChartDataset<"line", ChartPoint[]> {
  return {
    label, data: values, borderColor: color, backgroundColor: color, pointBackgroundColor: color, borderWidth: width, borderDash: dash,
    pointBorderColor: buckets.map((bucket) => bucket.containsHighlightedSession ? color : "transparent"),
    pointBorderWidth: buckets.map((bucket) => bucket.containsHighlightedSession ? 3 : 0),
    pointRadius: buckets.map((bucket) => bucket.containsHighlightedSession ? 5 : 3), spanGaps: false, tension: 0.24,
  };
}
function formatBucketLabel(value: number, range: AnalyticsRange): string {
  const date = new Date(value); return range === "all" ? monthFormatter.format(date) : compactDateFormatter.format(date);
}
function buildAriaLabel(buckets: TimelineBucket[]): string {
  const active = buckets.filter((bucket) => bucket.reviewCount > 0);
  if (active.length === 0) return "Learning progress chart. No reviews occurred in the selected range.";
  const sum = (read: (bucket: TimelineBucket) => number) => active.reduce((total, bucket) => total + read(bucket), 0);
  const reviews = sum((bucket) => bucket.reviewCount), wrong = sum((bucket) => bucket.wrongCount), hard = sum((bucket) => bucket.hardCount), medium = sum((bucket) => bucket.mediumCount), easy = sum((bucket) => bucket.easyCount);
  return `Learning progress line chart with ${reviews} reviews: ${wrong} Wrong, ${hard} Hard, ${medium} Medium, and ${easy} Easy.`;
}
function formatPercent(value: number | null): string { return value === null ? "no data" : String(Math.round(value * 100)) + "%"; }
function formatConfidence(value: number | null): string { return value === null ? "no data" : value.toFixed(1) + " / 3"; }
function formatDuration(value: number): string {
  if (value < 60_000) return String(Math.round(value / 1_000)) + " sec";
  if (value < 3_600_000) return String(Math.round(value / 60_000)) + " min";
  const hours = value / 3_600_000; return hours.toFixed(hours < 10 ? 1 : 0) + " hr";
}
