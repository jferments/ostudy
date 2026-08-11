#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const read = (path) => readFileSync(join(root, path), "utf8");
const write = (path, text) => writeFileSync(join(root, path), text);

function replaceOnce(path, before, after) {
  const text = read(path);
  const first = text.indexOf(before);
  if (first < 0) throw new Error(`${path}: expected source fragment not found`);
  if (text.indexOf(before, first + before.length) >= 0) {
    throw new Error(`${path}: source fragment was not unique`);
  }
  write(path, `${text.slice(0, first)}${after}${text.slice(first + before.length)}`);
}

function replaceRegex(path, pattern, replacement, expected = 1) {
  const text = read(path);
  const matches = [...text.matchAll(new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : `${pattern.flags}g`))];
  if (matches.length !== expected) {
    throw new Error(`${path}: expected ${expected} regex matches, found ${matches.length} for ${pattern}`);
  }
  write(path, text.replace(pattern, replacement));
}

function walk(directory) {
  const output = [];
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    if (["node_modules", ".git", "target", "dist"].includes(entry)) continue;
    if (statSync(path).isDirectory()) output.push(...walk(path));
    else output.push(path);
  }
  return output;
}

// ---------------------------------------------------------------------------
// Appearance / practice: keep a normal animation preference independent from
// the accessibility-oriented Reduce motion preference.
// ---------------------------------------------------------------------------
replaceOnce(
  "src/ipc/settings.ts",
  "  cardBaseSize: number;\n  reduceMotion: boolean;\n",
  "  cardBaseSize: number;\n  animationsEnabled: boolean;\n  reduceMotion: boolean;\n",
);

for (const absolute of walk(join(root, "src"))) {
  if (!/\.[cm]?[jt]sx?$/u.test(absolute)) continue;
  const text = readFileSync(absolute, "utf8");
  const next = text.replace(
    /(cardBaseSize:\s*\d+,\r?\n)(\s*)reduceMotion:/gu,
    "$1$2animationsEnabled: true,\n$2reduceMotion:",
  );
  if (next !== text) writeFileSync(absolute, next);
}

replaceOnce(
  "src-tauri/src/settings.rs",
  "    pub card_base_size: i64,\n    pub reduce_motion: bool,\n",
  "    pub card_base_size: i64,\n    pub animations_enabled: bool,\n    pub reduce_motion: bool,\n",
);
replaceOnce(
  "src-tauri/src/settings.rs",
  "        card_base_size: get_setting(connection, \"cardBaseSize\", 18_i64)?,\n        reduce_motion: get_setting(connection, \"reduceMotion\", false)?,\n",
  "        card_base_size: get_setting(connection, \"cardBaseSize\", 18_i64)?,\n        animations_enabled: get_setting(connection, \"animationsEnabled\", true)?,\n        reduce_motion: get_setting(connection, \"reduceMotion\", false)?,\n",
);
replaceOnce(
  "src-tauri/src/settings.rs",
  "        (\n            \"cardBaseSize\",\n            serde_json::to_string(&settings.card_base_size),\n        ),\n        (\n            \"reduceMotion\",\n",
  "        (\n            \"cardBaseSize\",\n            serde_json::to_string(&settings.card_base_size),\n        ),\n        (\n            \"animationsEnabled\",\n            serde_json::to_string(&settings.animations_enabled),\n        ),\n        (\n            \"reduceMotion\",\n",
);

write(
  "src/screens/settings/AppearanceSettings.tsx",
  `import type { AppSettings } from "../../ipc/settings";\nimport { FontControl, RangeControl, ThemeOption } from "./SettingsControls";\nimport styles from "./Settings.module.css";\n\ninterface AppearanceSettingsProps {\n  draft: AppSettings;\n  update: (patch: Partial<AppSettings>) => void;\n}\n\nexport function AppearanceSettings({ draft, update }: AppearanceSettingsProps) {\n  return (\n    <div className={styles.panelGrid}>\n      <section className={styles.fullWidthCard}>\n        <div className={styles.sectionHeading}>\n          <div>\n            <h2>Theme</h2>\n            <p>Theme and typography changes preview immediately throughout ostudy.</p>\n          </div>\n        </div>\n        <div className={styles.themeGrid} role="radiogroup" aria-label="Application theme">\n          <ThemeOption name="Light" value="light" selected={draft.theme === "light"} onSelect={() => update({ theme: "light" })} />\n          <ThemeOption name="Dark" value="dark" selected={draft.theme === "dark"} onSelect={() => update({ theme: "dark" })} />\n          <ThemeOption name="Console" value="console" selected={draft.theme === "console"} onSelect={() => update({ theme: "console" })} />\n          <ThemeOption name="Synthwave" value="synthwave" selected={draft.theme === "synthwave"} onSelect={() => update({ theme: "synthwave" })} />\n        </div>\n      </section>\n\n      <section className={styles.settingsCard}>\n        <h2>Typography</h2>\n        <FontControl label="Interface font" value={draft.uiFont} onChange={(uiFont) => update({ uiFont })} />\n        <FontControl label="Default card font" value={draft.cardFont} onChange={(cardFont) => update({ cardFont })} />\n      </section>\n\n      <section className={styles.settingsCard}>\n        <h2>Scale and motion</h2>\n        <RangeControl\n          label="Interface scale"\n          min={80}\n          max={140}\n          step={5}\n          value={draft.uiScale}\n          suffix="%"\n          onChange={(uiScale) => update({ uiScale })}\n        />\n        <RangeControl\n          label="Default card text"\n          min={12}\n          max={32}\n          step={1}\n          value={draft.cardBaseSize}\n          suffix=" px"\n          onChange={(cardBaseSize) => update({ cardBaseSize })}\n        />\n        <label className={styles.switchRow}>\n          <span>\n            <strong>Animations and transitions</strong>\n            <small>Enable card flips and other nonessential motion effects.</small>\n          </span>\n          <input\n            type="checkbox"\n            checked={draft.animationsEnabled}\n            onChange={(event) => update({ animationsEnabled: event.currentTarget.checked })}\n          />\n        </label>\n        <label className={styles.switchRow}>\n          <span>\n            <strong>Reduce motion</strong>\n            <small>Minimize motion even when animations are enabled.</small>\n          </span>\n          <input\n            type="checkbox"\n            checked={draft.reduceMotion}\n            onChange={(event) => update({ reduceMotion: event.currentTarget.checked })}\n          />\n        </label>\n      </section>\n    </div>\n  );\n}\n`,
);

replaceOnce(
  "src/screens/PracticeScreen.tsx",
  "        flipAnimationEnabled={!settings.reduceMotion}\n",
  "        flipAnimationEnabled={settings.animationsEnabled && !settings.reduceMotion}\n",
);

// Bulk entry belongs to Decks, not the embedded/global Search workflow.
replaceOnce("src/screens/CardsScreen.tsx", 'import { BulkEntryModal } from "./cards/BulkEntryModal";\n', "");
replaceOnce("src/screens/CardsScreen.tsx", "  const [bulkOpen, setBulkOpen] = useState(false);\n", "");
replaceRegex(
  "src/screens/CardsScreen.tsx",
  /\s*<button\n\s*type="button"\n\s*className="secondary-button"\n\s*disabled=\{activeDecks\.length === 0\}\n\s*onClick=\{\(\) => setBulkOpen\(true\)\}\n\s*>\n\s*Bulk add\n\s*<\/button>/u,
  "",
);
replaceRegex(
  "src/screens/CardsScreen.tsx",
  /\n\s*\{bulkOpen \? \(\n\s*<BulkEntryModal[\s\S]*?\n\s*\) : null\}/u,
  "",
);
replaceOnce(
  "src/screens/CardsScreen.tsx",
  '<option value="move_copy">Move / copy source notes</option>',
  '<option value="move_copy">Move / copy cards</option>',
);
replaceOnce(
  "src/screens/CardsScreen.tsx",
  '<option value="delete">Delete source notes</option>',
  '<option value="delete">Delete cards</option>',
);

// Remove remaining learner-facing "source note" terminology from Decks.
{
  const path = "src/screens/DecksScreen.tsx";
  let text = read(path);
  text = text.replaceAll('"source note"', '"card"').replaceAll('"source notes"', '"cards"');
  write(path, text);
}

// ---------------------------------------------------------------------------
// Analytics: line-only count series and exact custom local-calendar ranges.
// ---------------------------------------------------------------------------
replaceOnce(
  "src/ipc/analytics.ts",
  'export type AnalyticsRange = "7d" | "30d" | "90d" | "1y" | "all";\n',
  'export type AnalyticsRange = "7d" | "30d" | "90d" | "1y" | "all" | "custom";\n',
);
replaceOnce(
  "src/ipc/analytics.ts",
  "  tagIds: string[];\n  highlightedSessionId?: string;\n",
  "  tagIds: string[];\n  customStartLocalDate?: string;\n  customEndLocalDate?: string;\n  highlightedSessionId?: string;\n",
);

replaceOnce(
  "src/screens/analytics/AnalyticsFilters.tsx",
  "  range: AnalyticsRange;\n  selectedDeckIds: Set<string>;\n",
  "  range: AnalyticsRange;\n  customStartLocalDate: string;\n  customEndLocalDate: string;\n  selectedDeckIds: Set<string>;\n",
);
replaceOnce(
  "src/screens/analytics/AnalyticsFilters.tsx",
  "  onRangeChange: (range: AnalyticsRange) => void;\n  onDeckIdsChange: (next: Set<string>) => void;\n",
  "  onRangeChange: (range: AnalyticsRange) => void;\n  onCustomStartLocalDateChange: (value: string) => void;\n  onCustomEndLocalDateChange: (value: string) => void;\n  onDeckIdsChange: (next: Set<string>) => void;\n",
);
replaceOnce(
  "src/screens/analytics/AnalyticsFilters.tsx",
  '  { value: "all", label: "All time" },\n];\n',
  '  { value: "all", label: "All time" },\n  { value: "custom", label: "Custom" },\n];\n',
);
replaceOnce(
  "src/screens/analytics/AnalyticsFilters.tsx",
  "  range,\n  selectedDeckIds,\n",
  "  range,\n  customStartLocalDate,\n  customEndLocalDate,\n  selectedDeckIds,\n",
);
replaceOnce(
  "src/screens/analytics/AnalyticsFilters.tsx",
  "  onRangeChange,\n  onDeckIdsChange,\n",
  "  onRangeChange,\n  onCustomStartLocalDateChange,\n  onCustomEndLocalDateChange,\n  onDeckIdsChange,\n",
);
replaceOnce(
  "src/screens/analytics/AnalyticsFilters.tsx",
  "      />\n\n      <details className=\"analytics-scope-details\">\n",
  `      />\n\n      {range === "custom" ? (\n        <div className="analytics-custom-range" aria-label="Custom analytics date range">\n          <label>\n            <span>Start date</span>\n            <input\n              type="date"\n              value={customStartLocalDate}\n              max={customEndLocalDate}\n              onChange={(event) => onCustomStartLocalDateChange(event.currentTarget.value)}\n            />\n          </label>\n          <label>\n            <span>End date</span>\n            <input\n              type="date"\n              value={customEndLocalDate}\n              min={customStartLocalDate}\n              max={localToday()}\n              onChange={(event) => onCustomEndLocalDateChange(event.currentTarget.value)}\n            />\n          </label>\n        </div>\n      ) : null}\n\n      <details className="analytics-scope-details">\n`,
);
{
  const path = "src/screens/analytics/AnalyticsFilters.tsx";
  let text = read(path);
  text += `\nfunction localToday(): string {\n  const now = new Date();\n  const year = now.getFullYear();\n  const month = String(now.getMonth() + 1).padStart(2, "0");\n  const day = String(now.getDate()).padStart(2, "0");\n  return year + "-" + month + "-" + day;\n}\n`;
  write(path, text);
}

write(
  "src/screens/analytics/ProgressChart.tsx",
  `import { useMemo } from "react";\nimport {\n  CategoryScale,\n  Chart as ChartJS,\n  Legend,\n  LinearScale,\n  LineController,\n  LineElement,\n  PointElement,\n  Tooltip,\n  type ChartData,\n  type ChartDataset,\n  type ChartOptions,\n} from "chart.js";\nimport { Chart } from "react-chartjs-2";\n\nimport type { AnalyticsRange, TimelineBucket } from "../../ipc/analytics";\n\nChartJS.register(CategoryScale, LinearScale, LineController, LineElement, PointElement, Tooltip, Legend);\n\ninterface ProgressChartProps {\n  buckets: TimelineBucket[];\n  range: AnalyticsRange;\n  reduceMotion: boolean;\n}\n\nconst compactDateFormatter = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });\nconst monthFormatter = new Intl.DateTimeFormat(undefined, { month: "short", year: "2-digit" });\n\ntype ChartPoint = number | null;\ntype LineChartData = ChartData<"line", ChartPoint[], string>;\n\nexport function ProgressChart({ buckets, range, reduceMotion }: ProgressChartProps) {\n  const colors = readChartColors();\n  const labels = useMemo(\n    () => buckets.map((bucket) => formatBucketLabel(bucket.startAt, range)),\n    [buckets, range],\n  );\n  const data = useMemo<LineChartData>(() => {\n    const practiced: ChartDataset<"line", ChartPoint[]> = {\n      label: "Cards practiced",\n      data: buckets.map((bucket) => bucket.cardsPracticed),\n      borderColor: colors.accent,\n      backgroundColor: colors.accent,\n      pointBackgroundColor: buckets.map((bucket) =>\n        bucket.containsHighlightedSession ? colors.warning : colors.accent,\n      ),\n      pointBorderColor: colors.surface,\n      pointBorderWidth: 2,\n      pointRadius: buckets.map((bucket) => (bucket.containsHighlightedSession ? 5 : 3)),\n      spanGaps: false,\n      tension: 0.24,\n    };\n    const remembered: ChartDataset<"line", ChartPoint[]> = {\n      label: "Remembered",\n      data: buckets.map((bucket) => bucket.rememberedCount),\n      borderColor: colors.success,\n      backgroundColor: colors.success,\n      pointBackgroundColor: buckets.map((bucket) =>\n        bucket.containsHighlightedSession ? colors.warning : colors.success,\n      ),\n      pointBorderColor: colors.surface,\n      pointBorderWidth: 2,\n      pointRadius: buckets.map((bucket) => (bucket.containsHighlightedSession ? 5 : 3)),\n      spanGaps: false,\n      tension: 0.24,\n    };\n    return { labels, datasets: [practiced, remembered] };\n  }, [buckets, colors, labels]);\n  const options = useMemo<ChartOptions<"line">>(\n    () => ({\n      responsive: true,\n      maintainAspectRatio: false,\n      animation: reduceMotion ? false : { duration: 260 },\n      interaction: { mode: "index", intersect: false },\n      plugins: {\n        legend: {\n          position: "top",\n          align: "start",\n          labels: { color: colors.textSoft, usePointStyle: true, boxWidth: 9 },\n        },\n        tooltip: {\n          backgroundColor: colors.surfaceRaised,\n          titleColor: colors.text,\n          bodyColor: colors.textSoft,\n          borderColor: colors.border,\n          borderWidth: 1,\n          callbacks: {\n            afterBody(items) {\n              const bucket = buckets[items[0]?.dataIndex ?? 0];\n              if (!bucket) return [];\n              return [\n                String(bucket.reviewCount) + " review" + (bucket.reviewCount === 1 ? "" : "s"),\n                String(bucket.newCards) + " new card" + (bucket.newCards === 1 ? "" : "s"),\n                "Recall rate: " + formatPercent(bucket.correctRate),\n                "Confidence: " + formatConfidence(bucket.averageConfidence),\n                "Focused time: " + formatDuration(bucket.studyTimeMs),\n              ];\n            },\n          },\n        },\n      },\n      scales: {\n        x: {\n          grid: { display: false },\n          ticks: { color: colors.muted, maxRotation: 0, autoSkip: true },\n          border: { color: colors.border },\n        },\n        y: {\n          beginAtZero: true,\n          grid: { color: colors.grid },\n          ticks: { color: colors.muted, precision: 0 },\n          border: { color: colors.border },\n          title: { display: true, text: "Cards", color: colors.muted },\n        },\n      },\n    }),\n    [buckets, colors, reduceMotion],\n  );\n\n  return (\n    <div className="progress-chart-canvas">\n      <Chart type="line" data={data} options={options} role="img" aria-label={buildAriaLabel(buckets)} />\n    </div>\n  );\n}\n\ninterface ChartColors {\n  accent: string;\n  success: string;\n  warning: string;\n  surface: string;\n  surfaceRaised: string;\n  text: string;\n  textSoft: string;\n  muted: string;\n  border: string;\n  grid: string;\n}\n\nfunction readChartColors(): ChartColors {\n  const style = getComputedStyle(document.documentElement);\n  const value = (name: string, fallback: string) => style.getPropertyValue(name).trim() || fallback;\n  return {\n    accent: value("--accent", "#78a9ff"),\n    success: value("--success", "#55d79a"),\n    warning: value("--warning", "#f5c56b"),\n    surface: value("--surface", "#121a28"),\n    surfaceRaised: value("--surface-raised", "#182235"),\n    text: value("--text", "#edf3fb"),\n    textSoft: value("--text-soft", "#b6c1d2"),\n    muted: value("--muted", "#7f8da4"),\n    border: value("--border", "#2b3950"),\n    grid: value("--border", "#2b3950"),\n  };\n}\n\nfunction formatBucketLabel(value: number, range: AnalyticsRange): string {\n  const date = new Date(value);\n  return range === "all" ? monthFormatter.format(date) : compactDateFormatter.format(date);\n}\n\nfunction buildAriaLabel(buckets: TimelineBucket[]): string {\n  const active = buckets.filter((bucket) => bucket.reviewCount > 0);\n  if (active.length === 0) return "Learning progress chart. No reviews occurred in the selected range.";\n  const reviews = active.reduce((total, bucket) => total + bucket.reviewCount, 0);\n  const practiced = active.reduce((total, bucket) => total + bucket.cardsPracticed, 0);\n  const remembered = active.reduce((total, bucket) => total + bucket.rememberedCount, 0);\n  return (\n    "Learning progress line chart with " +\n    reviews +\n    " reviews. One line shows " +\n    practiced +\n    " bucket-level cards practiced and the second shows " +\n    remembered +\n    " remembered answers."\n  );\n}\n\nfunction formatPercent(value: number | null): string {\n  return value === null ? "no data" : String(Math.round(value * 100)) + "%";\n}\n\nfunction formatConfidence(value: number | null): string {\n  return value === null ? "no data" : value.toFixed(1) + " / 3";\n}\n\nfunction formatDuration(value: number): string {\n  if (value < 60_000) return String(Math.round(value / 1_000)) + " sec";\n  if (value < 3_600_000) return String(Math.round(value / 60_000)) + " min";\n  const hours = value / 3_600_000;\n  return hours.toFixed(hours < 10 ? 1 : 0) + " hr";\n}\n`,
);

replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  '  const [range, setRange] = useState<AnalyticsRange>("7d");\n',
  '  const [range, setRange] = useState<AnalyticsRange>("7d");\n  const [customStartLocalDate, setCustomStartLocalDate] = useState(() => localDateDaysAgo(6));\n  const [customEndLocalDate, setCustomEndLocalDate] = useState(() => localDateDaysAgo(0));\n',
);
replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  '  const [chartMetric, setChartMetric] = useState<"cards" | "time">("cards");\n',
  "",
);
replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  "        tagIds,\n        highlightedSessionId,\n",
  "        tagIds,\n        ...(range === \"custom\"\n          ? { customStartLocalDate, customEndLocalDate }\n          : {}),\n        highlightedSessionId,\n",
);
replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  "  }, [controller, deckIds, highlightedSessionId, range, showToast, tagIds]);\n",
  "  }, [\n    controller,\n    customEndLocalDate,\n    customStartLocalDate,\n    deckIds,\n    highlightedSessionId,\n    range,\n    showToast,\n    tagIds,\n  ]);\n",
);
replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  "        range={range}\n        selectedDeckIds={selectedDeckIds}\n",
  "        range={range}\n        customStartLocalDate={customStartLocalDate}\n        customEndLocalDate={customEndLocalDate}\n        selectedDeckIds={selectedDeckIds}\n",
);
replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  "        onRangeChange={setRange}\n        onDeckIdsChange={setSelectedDeckIds}\n",
  "        onRangeChange={setRange}\n        onCustomStartLocalDateChange={setCustomStartLocalDate}\n        onCustomEndLocalDateChange={setCustomEndLocalDate}\n        onDeckIdsChange={setSelectedDeckIds}\n",
);
replaceRegex(
  "src/screens/AnalyticsScreen.tsx",
  /\n\s*<div className="segmented-control" role="group" aria-label="Chart volume metric">[\s\S]*?<\/div>\n\s*<button/u,
  "\n                <button",
);
replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  "              metric={chartMetric}\n              reduceMotion={settings.reduceMotion}\n",
  "              reduceMotion={settings.reduceMotion}\n",
);
replaceOnce(
  "src/screens/AnalyticsScreen.tsx",
  "                            {tag.generatedCardCount} generated · {tag.sourceNoteCount} source notes\n",
  "                            {tag.generatedCardCount} cards\n",
);
{
  const path = "src/screens/AnalyticsScreen.tsx";
  let text = read(path);
  text += `\nfunction localDateDaysAgo(days: number): string {\n  const value = new Date();\n  value.setHours(12, 0, 0, 0);\n  value.setDate(value.getDate() - days);\n  const year = value.getFullYear();\n  const month = String(value.getMonth() + 1).padStart(2, "0");\n  const day = String(value.getDate()).padStart(2, "0");\n  return year + "-" + month + "-" + day;\n}\n`;
  write(path, text);
}

write(
  "src/screens/analytics/ProgressChart.test.ts",
  `import { Chart as ChartJS } from "chart.js";\nimport { describe, expect, it } from "vitest";\n\nimport "./ProgressChart";\n\ndescribe("ProgressChart Chart.js registration", () => {\n  it("registers the line controller used by the progress chart", () => {\n    expect(() => ChartJS.registry.getController("line")).not.toThrow();\n  });\n});\n`,
);

replaceOnce(
  "src/screens/AnalyticsScreen.test.tsx",
  '      bucketGranularity: range === "7d" || range === "30d" ? "day" : "week",\n',
  '      bucketGranularity:\n        range === "7d" || range === "30d" || range === "custom"\n          ? "day"\n          : range === "all"\n            ? "month"\n            : "week",\n',
);
replaceOnce(
  "src/screens/AnalyticsScreen.test.tsx",
  "  it(\"discards a slower response for an obsolete range\", async () => {\n",
  `  it("submits an exact custom local date range", async () => {\n    renderAnalytics();\n    await screen.findByText("Cards practiced");\n\n    fireEvent.click(screen.getByRole("radio", { name: "Custom" }));\n    fireEvent.change(screen.getByLabelText("Start date"), { target: { value: "2026-07-01" } });\n    fireEvent.change(screen.getByLabelText("End date"), { target: { value: "2026-07-15" } });\n\n    await waitFor(() => {\n      expect(analyticsIpc.getAnalytics).toHaveBeenLastCalledWith(\n        expect.objectContaining({\n          range: "custom",\n          customStartLocalDate: "2026-07-01",\n          customEndLocalDate: "2026-07-15",\n        }),\n      );\n    });\n  });\n\n  it("discards a slower response for an obsolete range", async () => {\n`,
);

{
  const path = "src/styles/features/analytics.css";
  let text = read(path);
  if (!text.includes(".analytics-custom-range")) {
    text += `\n.analytics-custom-range {\n  display: flex;\n  flex-wrap: wrap;\n  gap: 0.75rem;\n  align-items: end;\n}\n\n.analytics-custom-range label {\n  display: grid;\n  gap: 0.35rem;\n}\n\n.analytics-custom-range label > span {\n  color: var(--muted);\n  font-size: 0.78rem;\n  font-weight: 700;\n}\n`;
  }
  write(path, text);
}

replaceOnce(
  "src-tauri/src/analytics/mod.rs",
  "    pub tag_ids: Vec<String>,\n    #[serde(default)]\n    pub highlighted_session_id: Option<String>,\n",
  "    pub tag_ids: Vec<String>,\n    #[serde(default)]\n    pub custom_start_local_date: Option<String>,\n    #[serde(default)]\n    pub custom_end_local_date: Option<String>,\n    #[serde(default)]\n    pub highlighted_session_id: Option<String>,\n",
);
replaceOnce(
  "src-tauri/src/analytics/mod.rs",
  "            tag_ids: Vec::new(),\n            highlighted_session_id: None,\n",
  "            tag_ids: Vec::new(),\n            custom_start_local_date: None,\n            custom_end_local_date: None,\n            highlighted_session_id: None,\n",
);

replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  "const MAX_ANALYTICS_TAGS: usize = 64;\n",
  "const MAX_ANALYTICS_TAGS: usize = 64;\nconst MAX_CUSTOM_RANGE_DAYS: i64 = 3_660;\n",
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  "    Year1,\n    All,\n",
  "    Year1,\n    All,\n    Custom,\n",
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  '            "all" => Ok(Self::All),\n',
  '            "all" => Ok(Self::All),\n            "custom" => Ok(Self::Custom),\n',
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  '                "Choose 7 days, 30 days, 90 days, one year, or all time.",\n',
  '                "Choose 7 days, 30 days, 90 days, one year, all time, or a custom range.",\n',
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  '            Self::All => "all",\n',
  '            Self::All => "all",\n            Self::Custom => "custom",\n',
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  '            Self::All => "month",\n',
  '            Self::All => "month",\n            Self::Custom => "day",\n',
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  "            Self::All => None,\n",
  "            Self::All => None,\n            Self::Custom => Some(1),\n",
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  `        let time = StudyTimeContext::capture()?;\n        let current = match range {\n            AnalyticsRange::Days7 => Some(time.trailing_local_dates(7)?),\n            AnalyticsRange::Days30 => Some(time.trailing_local_dates(30)?),\n            AnalyticsRange::Days90 => Some(time.trailing_local_dates(90)?),\n            AnalyticsRange::Year1 => Some(time.one_year_window()?),\n            AnalyticsRange::All => None,\n        };\n`,
  `        let time = StudyTimeContext::capture()?;\n        let current = match range {\n            AnalyticsRange::Days7 => Some(time.trailing_local_dates(7)?),\n            AnalyticsRange::Days30 => Some(time.trailing_local_dates(30)?),\n            AnalyticsRange::Days90 => Some(time.trailing_local_dates(90)?),\n            AnalyticsRange::Year1 => Some(time.one_year_window()?),\n            AnalyticsRange::All => None,\n            AnalyticsRange::Custom => Some(custom_window(\n                &time,\n                input.custom_start_local_date.as_deref(),\n                input.custom_end_local_date.as_deref(),\n            )?),\n        };\n`,
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  "                end_local_date: time.local_date,\n",
  "                end_local_date: current\n                    .as_ref()\n                    .map_or(time.local_date, |window| window.end_local_date),\n",
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  "            end_at: self.time.now_utc_millis,\n",
  "            end_at: self.window.current.as_ref().map_or(self.time.now_utc_millis, |window| {\n                window.end_utc_millis.saturating_sub(1)\n            }),\n",
);
replaceOnce(
  "src-tauri/src/analytics/scope.rs",
  "fn normalize_ids(\n",
  `fn custom_window(\n    time: &StudyTimeContext,\n    start: Option<&str>,\n    end: Option<&str>,\n) -> Result<CalendarWindow, CommandError> {\n    let parse = |value: Option<&str>| -> Result<NaiveDate, CommandError> {\n        let value = value.ok_or_else(|| {\n            CommandError::new(\n                "invalid_analytics_custom_range",\n                "Choose both a start date and an end date for the custom analytics range.",\n            )\n        })?;\n        NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {\n            CommandError::new(\n                "invalid_analytics_custom_range",\n                "Choose valid custom analytics dates.",\n            )\n        })\n    };\n    let start_local_date = parse(start)?;\n    let end_local_date = parse(end)?;\n    if start_local_date > end_local_date || end_local_date > time.local_date {\n        return Err(CommandError::new(\n            "invalid_analytics_custom_range",\n            "The custom analytics start date must be on or before the end date, and the end date cannot be in the future.",\n        ));\n    }\n    let day_count = end_local_date\n        .signed_duration_since(start_local_date)\n        .num_days()\n        .saturating_add(1);\n    if !(1..=MAX_CUSTOM_RANGE_DAYS).contains(&day_count) {\n        return Err(CommandError::new(\n            "invalid_analytics_custom_range",\n            "Choose a custom analytics range no longer than 3660 local calendar days.",\n        ));\n    }\n    let end_utc_millis = if end_local_date == time.local_date {\n        time.now_utc_millis.saturating_add(1)\n    } else {\n        let next = end_local_date.succ_opt().ok_or_else(|| {\n            CommandError::new(\n                "invalid_analytics_custom_range",\n                "The custom analytics end date is outside the supported calendar.",\n            )\n        })?;\n        time.first_valid_instant(next)?\n    };\n    Ok(CalendarWindow {\n        start_local_date,\n        end_local_date,\n        start_utc_millis: time.first_valid_instant(start_local_date)?,\n        end_utc_millis,\n    })\n}\n\nfn normalize_ids(\n`,
);

// ---------------------------------------------------------------------------
// Documents: durable last-run duration + provider-reported token usage.
// ---------------------------------------------------------------------------
const migration = `-- Record bounded metrics for the most recent managed-document processing run.\n-- Provider token totals are stored only when the provider reports usage; ostudy\n-- never substitutes local token estimates for an API-reported total.\n\nALTER TABLE documents ADD COLUMN processing_started_at INTEGER;\nALTER TABLE documents ADD COLUMN processing_duration_ms INTEGER\n    CHECK(processing_duration_ms IS NULL OR processing_duration_ms >= 0);\nALTER TABLE documents ADD COLUMN api_token_count INTEGER\n    CHECK(api_token_count IS NULL OR api_token_count >= 0);\nALTER TABLE documents ADD COLUMN api_token_usage_complete INTEGER NOT NULL DEFAULT 0\n    CHECK(api_token_usage_complete IN (0, 1));\n`;
write("src-tauri/migrations/0022_document_processing_metrics.sql", migration);
const migrationChecksum = createHash("sha256").update(migration).digest("hex");
replaceOnce(
  "src-tauri/src/database.rs",
  `    Migration {\n        version: 21,\n        name: "document_ocr_fallback",\n        checksum: "31466a2bec357f46ee1e05680f3b26d5a297445bf686bfc2afd5aef470206e5d",\n        sql: include_str!("../migrations/0021_document_ocr_fallback.sql"),\n    },\n];\n\npub(crate) const LATEST_SCHEMA_VERSION: i64 = 21;\n`,
  `    Migration {\n        version: 21,\n        name: "document_ocr_fallback",\n        checksum: "31466a2bec357f46ee1e05680f3b26d5a297445bf686bfc2afd5aef470206e5d",\n        sql: include_str!("../migrations/0021_document_ocr_fallback.sql"),\n    },\n    Migration {\n        version: 22,\n        name: "document_processing_metrics",\n        checksum: "${migrationChecksum}",\n        sql: include_str!("../migrations/0022_document_processing_metrics.sql"),\n    },\n];\n\npub(crate) const LATEST_SCHEMA_VERSION: i64 = 22;\n`,
);

replaceOnce(
  "src-tauri/src/document_store.rs",
  "                 SET lifecycle_state = 'processing', safe_error_code = '', safe_error_message = '',\n                     updated_at = ?1, completed_at = NULL, revision = revision + 1\n",
  "                 SET lifecycle_state = 'processing', safe_error_code = '', safe_error_message = '',\n                     processing_started_at = ?1, processing_duration_ms = NULL,\n                     api_token_count = NULL, api_token_usage_complete = 0,\n                     updated_at = ?1, completed_at = NULL, revision = revision + 1\n",
);

replaceOnce(
  "src-tauri/src/ai/document_intelligence.rs",
  "use std::{cmp::Reverse, collections::HashSet};\n",
  "use std::{\n    cmp::Reverse,\n    collections::HashSet,\n    sync::{Arc, Mutex},\n};\n",
);
replaceOnce(
  "src-tauri/src/ai/document_intelligence.rs",
  "    credential_storage: CredentialStorage,\n}\n",
  `    credential_storage: CredentialStorage,\n    usage: Arc<Mutex<DocumentUsageAccumulator>>,\n}\n\n#[derive(Clone, Copy, Debug, Default)]\nstruct DocumentUsageAccumulator {\n    total_tokens: i64,\n    reported_requests: i64,\n    unreported_requests: i64,\n}\n\nimpl DocumentTaskContext {\n    pub(crate) fn usage_summary(&self) -> (Option<i64>, bool) {\n        let Ok(usage) = self.usage.lock() else {\n            return (None, false);\n        };\n        let tokens = (usage.reported_requests > 0).then_some(usage.total_tokens);\n        let complete = usage.reported_requests > 0 && usage.unreported_requests == 0;\n        (tokens, complete)\n    }\n}\n`,
);
replaceOnce(
  "src-tauri/src/ai/document_intelligence.rs",
  "            credential_storage,\n        })\n",
  "            credential_storage,\n            usage: Arc::new(Mutex::new(DocumentUsageAccumulator::default())),\n        })\n",
);
replaceOnce(
  "src-tauri/src/ai/document_intelligence.rs",
  "    log_document_usage(task, &response);\n",
  "    record_document_usage(task, &response);\n",
);
replaceRegex(
  "src-tauri/src/ai/document_intelligence.rs",
  /fn log_document_usage\(task: &DocumentTaskContext, response: &Value\) \{[\s\S]*?\n\}\n\nfn deepseek_strict_completion_endpoint/u,
  `fn record_document_usage(task: &DocumentTaskContext, response: &Value) {\n    let usage = response.get("usage");\n    let prompt = usage\n        .and_then(|value| value.get("prompt_tokens"))\n        .and_then(Value::as_u64);\n    let completion = usage\n        .and_then(|value| value.get("completion_tokens"))\n        .and_then(Value::as_u64);\n    let reported_total = usage\n        .and_then(|value| value.get("total_tokens"))\n        .and_then(Value::as_u64)\n        .or_else(|| prompt.zip(completion).map(|(prompt, completion)| prompt.saturating_add(completion)));\n    if let Ok(mut accumulator) = task.usage.lock() {\n        if let Some(total) = reported_total {\n            accumulator.total_tokens = accumulator\n                .total_tokens\n                .saturating_add(i64::try_from(total).unwrap_or(i64::MAX));\n            accumulator.reported_requests = accumulator.reported_requests.saturating_add(1);\n        } else {\n            accumulator.unreported_requests = accumulator.unreported_requests.saturating_add(1);\n        }\n    }\n\n    let prompt = prompt.unwrap_or(0);\n    let completion = completion.unwrap_or(0);\n    let cache_hit = usage\n        .and_then(|value| value.get("prompt_cache_hit_tokens"))\n        .and_then(Value::as_u64)\n        .unwrap_or(0);\n    let cache_miss = usage\n        .and_then(|value| value.get("prompt_cache_miss_tokens"))\n        .and_then(Value::as_u64)\n        .unwrap_or(0);\n    let reasoning = usage\n        .and_then(|value| value.pointer("/completion_tokens_details/reasoning_tokens"))\n        .and_then(Value::as_u64)\n        .unwrap_or(0);\n    crate::logging::info(\n        "document_provider_usage",\n        format!(\n            "provider={} model={} prompt_tokens={prompt} completion_tokens={completion} cache_hit_tokens={cache_hit} cache_miss_tokens={cache_miss} reasoning_tokens={reasoning}",\n            task.provider_kind, task.model_id,\n        ),\n    );\n}\n\nfn deepseek_strict_completion_endpoint`,
);

replaceOnce(
  "src-tauri/src/documents.rs",
  "    pub archived_at: Option<i64>,\n    pub revision: i64,\n",
  "    pub archived_at: Option<i64>,\n    pub revision: i64,\n    pub processing_duration_ms: Option<i64>,\n    pub api_token_count: Option<i64>,\n    pub api_token_usage_complete: bool,\n",
);
replaceOnce(
  "src-tauri/src/documents.rs",
  "    let payload = build_generated_import_json(&stored, generated_cards)?;\n    let result = import_generated_flashcards_bytes_with_commit(\n",
  "    let payload = build_generated_import_json(&stored, generated_cards)?;\n    let (api_token_count, api_token_usage_complete) = task.usage_summary();\n    let result = import_generated_flashcards_bytes_with_commit(\n",
);
replaceOnce(
  "src-tauri/src/documents.rs",
  `                         safe_error_code = '', safe_error_message = '',\n                         updated_at = ?4, completed_at = ?4, revision = revision + 1\n                     WHERE id = ?5 AND lifecycle_state = 'processing'\",\n                    params![\n                        import_result.created_note_count,\n                        import_result.created_card_count,\n                        import_result.deck_id,\n                        now,\n                        document_id,\n                    ],\n`,
  `                         safe_error_code = '', safe_error_message = '',\n                         processing_duration_ms = CASE\n                             WHEN processing_started_at IS NULL THEN NULL\n                             ELSE MAX(0, ?4 - processing_started_at)\n                         END,\n                         api_token_count = ?5, api_token_usage_complete = ?6,\n                         updated_at = ?4, completed_at = ?4, revision = revision + 1\n                     WHERE id = ?7 AND lifecycle_state = 'processing'\",\n                    params![\n                        import_result.created_note_count,\n                        import_result.created_card_count,\n                        import_result.deck_id,\n                        now,\n                        api_token_count,\n                        if api_token_usage_complete { 1_i64 } else { 0_i64 },\n                        document_id,\n                    ],\n`,
);
replaceOnce(
  "src-tauri/src/documents.rs",
  "                     safe_error_code = ?1, safe_error_message = ?2,\n                     updated_at = ?3, revision = revision + 1\n",
  "                     safe_error_code = ?1, safe_error_message = ?2,\n                     processing_duration_ms = CASE\n                         WHEN processing_started_at IS NULL THEN processing_duration_ms\n                         ELSE MAX(0, ?3 - processing_started_at)\n                     END,\n                     updated_at = ?3, revision = revision + 1\n",
);
replaceOnce(
  "src-tauri/src/documents.rs",
  "                    completed_at, archived_at, revision\n             FROM documents\n",
  "                    completed_at, archived_at, revision,\n                    processing_duration_ms, api_token_count, api_token_usage_complete\n             FROM documents\n",
);
replaceOnce(
  "src-tauri/src/documents.rs",
  "                    completed_at, archived_at, revision, source_relative_path,\n                    extraction_backend, render_backend, semantic_chunk_count,\n",
  "                    completed_at, archived_at, revision,\n                    processing_duration_ms, api_token_count, api_token_usage_complete,\n                    source_relative_path, extraction_backend, render_backend, semantic_chunk_count,\n",
);
replaceOnce(
  "src-tauri/src/documents.rs",
  "                    source_relative_path: row.get(30)?,\n                    extraction_backend: row.get(31)?,\n                    render_backend: row.get(32)?,\n                    semantic_chunk_count: row.get(33)?,\n                    source_format: row.get(34)?,\n                    source_unit_kind: row.get(35)?,\n",
  "                    source_relative_path: row.get(33)?,\n                    extraction_backend: row.get(34)?,\n                    render_backend: row.get(35)?,\n                    semantic_chunk_count: row.get(36)?,\n                    source_format: row.get(37)?,\n                    source_unit_kind: row.get(38)?,\n",
);
replaceOnce(
  "src-tauri/src/documents.rs",
  "        archived_at: row.get(28)?,\n        revision: row.get(29)?,\n",
  "        archived_at: row.get(28)?,\n        revision: row.get(29)?,\n        processing_duration_ms: row.get(30)?,\n        api_token_count: row.get(31)?,\n        api_token_usage_complete: row.get::<_, i64>(32)? != 0,\n",
);
replaceOnce(
  "src-tauri/src/documents.rs",
  "                archived_at: None,\n                revision: 1,\n",
  "                archived_at: None,\n                revision: 1,\n                processing_duration_ms: None,\n                api_token_count: None,\n                api_token_usage_complete: false,\n",
);

replaceOnce(
  "src-tauri/src/document_more.rs",
  "    let payload = build_append_payload(&context.deck_name, &context.source_format, generated)?;\n    let result = import_generated_flashcards_bytes_with_commit(\n",
  "    let payload = build_append_payload(&context.deck_name, &context.source_format, generated)?;\n    let (api_token_count, api_token_usage_complete) = task.usage_summary();\n    let result = import_generated_flashcards_bytes_with_commit(\n",
);
replaceOnce(
  "src-tauri/src/document_more.rs",
  `                         safe_error_code = '', safe_error_message = '',\n                         updated_at = ?3, completed_at = ?3, revision = revision + 1\n                     WHERE id = ?4 AND generated_deck_id = ?5 AND lifecycle_state = 'processing'\",\n                    params![\n                        import_result.created_note_count,\n                        import_result.created_card_count,\n                        now,\n                        input.document_id,\n                        context.deck_id,\n                    ],\n`,
  `                         safe_error_code = '', safe_error_message = '',\n                         processing_duration_ms = CASE\n                             WHEN processing_started_at IS NULL THEN NULL\n                             ELSE MAX(0, ?3 - processing_started_at)\n                         END,\n                         api_token_count = ?4, api_token_usage_complete = ?5,\n                         updated_at = ?3, completed_at = ?3, revision = revision + 1\n                     WHERE id = ?6 AND generated_deck_id = ?7 AND lifecycle_state = 'processing'\",\n                    params![\n                        import_result.created_note_count,\n                        import_result.created_card_count,\n                        now,\n                        api_token_count,\n                        if api_token_usage_complete { 1_i64 } else { 0_i64 },\n                        input.document_id,\n                        context.deck_id,\n                    ],\n`,
);

replaceOnce(
  "src/ipc/documents.ts",
  "  revision: number;\n}\n",
  "  revision: number;\n  processingDurationMs: number | null;\n  apiTokenCount: number | null;\n  apiTokenUsageComplete: boolean;\n}\n",
);
replaceOnce(
  "src/screens/DocumentsScreen.tsx",
  `                    <div>\n                      <dt>Cards</dt>\n                      <dd>{document.generatedCardCount}</dd>\n                    </div>\n                    <div>\n                      <dt>Updated</dt>\n`,
  `                    <div>\n                      <dt>Cards</dt>\n                      <dd>{document.generatedCardCount}</dd>\n                    </div>\n                    <div>\n                      <dt>Processing</dt>\n                      <dd>{formatProcessingDuration(document.processingDurationMs)}</dd>\n                    </div>\n                    <div>\n                      <dt>API tokens</dt>\n                      <dd>{formatApiTokens(document)}</dd>\n                    </div>\n                    <div>\n                      <dt>Updated</dt>\n`,
);
replaceRegex(
  "src/screens/DocumentsScreen.tsx",
  /\n\s*\{hasCompletedOcrRecovery\(document\) \? \([\s\S]*?\) : null\}\n\s*\{document\.generatedDeckId \? \([\s\S]*?\) : document\.generatedCardCount > 0 && document\.lifecycleState === "ready" \? \(/u,
  `\n                  {!document.generatedDeckId &&\n                  document.generatedCardCount > 0 &&\n                  document.lifecycleState === "ready" ? (`,
);
replaceOnce(
  "src/screens/DocumentsScreen.tsx",
  "function formatDate(timestamp: number): string {\n",
  `function formatProcessingDuration(value: number | null): string {\n  if (value === null) return "—";\n  if (value < 1_000) return "<1 sec";\n  if (value < 60_000) return Math.round(value / 1_000) + " sec";\n  if (value < 3_600_000) return (value / 60_000).toFixed(value < 600_000 ? 1 : 0) + " min";\n  return (value / 3_600_000).toFixed(1) + " hr";\n}\n\nfunction formatApiTokens(document: DocumentSummary): string {\n  if (document.apiTokenCount === null) return "Not reported";\n  const formatted = document.apiTokenCount.toLocaleString();\n  return document.apiTokenUsageComplete ? formatted : formatted + "+ reported";\n}\n\nfunction formatDate(timestamp: number): string {\n`,
);

replaceOnce(
  "src/screens/DocumentsScreen.module.css",
  "  grid-template-columns: repeat(4, minmax(6rem, 1fr));\n",
  "  grid-template-columns: repeat(3, minmax(6rem, 1fr));\n",
);
{
  const path = "src/screens/DocumentsScreen.module.css";
  let text = read(path);
  text = text.replace(/\n\.extractionWarning \{[\s\S]*?\n\}\n/u, "\n");
  write(path, text);
}

for (const path of ["src/screens/DocumentsScreen.test.tsx", "src/screens/DocumentsScreen.ocrFailure.test.tsx"]) {
  let text = read(path);
  text = text.replace(
    /(\s+revision:\s*\d+,\n)(\s*)(\.\.\.overrides,\n)?/u,
    (match, revision, indent, overrides = "") =>
      `${revision}${indent}processingDurationMs: 12_345,\n${indent}apiTokenCount: 45_678,\n${indent}apiTokenUsageComplete: true,\n${indent}${overrides}`,
  );
  write(path, text);
}
replaceOnce(
  "src/screens/DocumentsScreen.test.tsx",
  "    expect(screen.getByText(/normal editing, scheduling, archive, and deletion tools/u)).toBeTruthy();\n",
  "    expect(screen.queryByText(/normal editing, scheduling, archive, and deletion tools/u)).toBeNull();\n    expect(screen.getByText(\"12.3 sec\")).toBeTruthy();\n    expect(screen.getByText(\"45,678\")).toBeTruthy();\n",
);
replaceRegex(
  "src/screens/DocumentsScreen.test.tsx",
  /\n\s*it\("warns persistently when OCR recovered an unreliable PDF text layer", async \(\) => \{[\s\S]*?\n\s*\}\);\n/u,
  `\n\n  it("keeps completed OCR recovery out of permanent document-list copy", async () => {\n    vi.mocked(documentIpc.listDocuments).mockResolvedValue([\n      makeDocument({\n        extractionMode: "ocr_fallback",\n        ocrPageCount: 18,\n        ocrCompletedPageCount: 18,\n        ocrMeanConfidence: 0.91,\n        ocrEngine: "Tesseract CLI via oxidize-pdf OcrProvider",\n      }),\n    ]);\n    renderDocuments();\n\n    await screen.findByRole("heading", { name: "Cardiac physiology" });\n    expect(screen.queryByText(/Native PDF text was unreliable/u)).toBeNull();\n    expect(screen.queryByText(/Mean OCR confidence/u)).toBeNull();\n  });\n`,
);

// OCR is a recovery path for failed/unusable native text, not a response to
// harmless isolated glyph loss or degraded semantic layout.
replaceOnce(
  "src-tauri/src/pdf_backend.rs",
  '    "oxidize-pdf-4.2.2-rag-quality-ocr-v1"\n',
  '    "oxidize-pdf-4.2.2-rag-quality-ocr-v2"\n',
);
replaceOnce(
  "src-tauri/src/pdf_backend.rs",
  "    if replacement > 0 {\n        reasons.push(\"unicode_replacement_character\");\n    }\n",
  "    if replacement >= 3\n        && replacement.saturating_mul(100) >= nonspace.max(1)\n    {\n        reasons.push(\"unicode_replacement_character\");\n    }\n",
);
replaceOnce(
  "src-tauri/src/pdf_backend.rs",
  '        assert_eq!(active_backend_id(), "oxidize-pdf-4.2.2-rag-quality-ocr-v1");\n',
  '        assert_eq!(active_backend_id(), "oxidize-pdf-4.2.2-rag-quality-ocr-v2");\n',
);
replaceOnce(
  "src-tauri/src/pdf_backend.rs",
  "    #[test]\n    fn corrupt_semantics_with_good_raw_text_use_native_page_fallback_not_ocr() {\n",
  `    #[test]\n    fn isolated_replacement_glyph_does_not_force_ocr_on_readable_native_text() {\n        let assessment = assess_text_quality(\n            "A readable native text layer may contain one unsupported glyph � without making the page unusable for study.",\n        );\n        assert!(!assessment.suspicious);\n    }\n\n    #[test]\n    fn repeated_replacement_glyphs_still_fail_native_text_quality() {\n        let assessment = assess_text_quality("Cardiac �� �� �� �� �� physiology text");\n        assert!(assessment.suspicious);\n        assert!(assessment.reasons.contains(&"unicode_replacement_character"));\n    }\n\n    #[test]\n    fn empty_native_page_still_selects_ocr_recovery() {\n        let document = base_document("", "");\n        let plan = plan_text_recovery(&document);\n        assert_eq!(plan.ocr_pages.len(), 1);\n        assert_eq!(plan.ocr_pages[0].page_number, 1);\n    }\n\n    #[test]\n    fn corrupt_semantics_with_good_raw_text_use_native_page_fallback_not_ocr() {\n`,
);

// ---------------------------------------------------------------------------
// Documentation and an explicit platform dependency guard.
// ---------------------------------------------------------------------------
replaceOnce(
  "docs/user-guide/getting-started.md",
  "Attach human-readable tags for concepts that cross deck boundaries. **Search/Filter** can query the whole library and narrow results by deck, tags, due state, or study history.",
  "Attach human-readable tags for concepts that cross deck boundaries. The **Search and filter cards** workspace at the bottom of **Decks** can query the whole library and narrow results by deck, tags, due state, or study history.",
);
replaceOnce(
  "docs/user-guide/getting-started.md",
  "ostudy maps these choices to the FSRS scheduling engine and records an immutable review event together with the new current schedule. Interval previews show the effect of each choice before it is submitted.\n",
  "ostudy maps these choices to the FSRS scheduling engine and records an immutable review event together with the new current schedule. FSRS computes the next interval after each result; the rating buttons stay intentionally compact rather than repeating interval estimates. Press **Space** or **Enter** to reveal an answer, then **0**, **1**, **2**, or **3** for Wrong, Hard, Medium, or Easy.\n",
);
replaceOnce(
  "docs/user-guide/getting-started.md",
  "2. Add two tags and find the cards through Search/Filter.\n",
  "2. Add two tags and find the cards through the Search and filter cards workspace in Decks.\n",
);
{
  const path = "docs/architecture/overview.md";
  let text = read(path);
  if (!text.includes("## Cross-platform UI dependency boundary")) {
    text += `\n## Cross-platform UI dependency boundary\n\nostudy's product UI is intentionally platform-neutral. React screens, feature controllers, responsive layouts, and shared UI primitives must not branch on desktop/mobile operating-system identity. Native differences are exposed through typed IPC under \`src/ipc/\` or capability-focused adapters under \`src/platform/\`. Responsive presentation is selected from viewport, pointer, safe-area, and accessibility capabilities rather than user-agent strings.\n\nThe dependency direction is one-way: shared UI primitives do not import feature controllers or IPC; feature screens may depend on controllers/shared UI; controllers may depend on typed IPC; direct Tauri imports are confined to the IPC/platform boundary. This lets Android, iOS, Windows, macOS, and Linux supply platform integrations without forking the study workflows themselves.\n`;
  }
  write(path, text);
}
replaceOnce(
  "scripts/verify-repository-policy.mjs",
  `  if (\n    /from\\s+["']@tauri-apps\\/(?:plugin-dialog|plugin-opener)["']/u.test(text)\n    && !displayPath.startsWith("src/platform/")\n  ) {\n    failures.push(\`${displayPath}: platform-specific Tauri plugins may only be imported by src/platform adapters\`);\n  }\n`,
  `  if (\n    /from\\s+["']@tauri-apps\\//u.test(text)\n    && !displayPath.startsWith("src/platform/")\n    && !displayPath.startsWith("src/ipc/")\n  ) {\n    failures.push(\`${displayPath}: direct Tauri imports are confined to typed IPC and platform adapters\`);\n  }\n`,
);

console.log("Final UI/product overhaul transformations applied successfully.");
