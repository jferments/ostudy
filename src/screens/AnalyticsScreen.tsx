import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AnalyticsLaunchRequest } from "../App";
import { AnalyticsController } from "../controllers/analyticsController";
import type { Analytics, AnalyticsRange, TagAnalytics } from "../ipc/analytics";
import { getErrorMessage } from "../ipc/common";
import type { DeckSummary } from "../ipc/decks";
import type { ReviewOutcome } from "../ipc/practice";
import { AnalyticsFilters } from "./analytics/AnalyticsFilters";
import { ProgressChart } from "./analytics/ProgressChart";
import { QuestionTypeBreakdown } from "./analytics/QuestionTypeBreakdown";
import { PageScaffold, PageHeader } from "../ui/patterns/WorkspacePatterns";
import { MetricTile } from "../ui/patterns/Metrics";
import { Button } from "../ui/primitives/Button";
import { Badge } from "../ui/primitives/Badge";
import { Surface } from "../ui/primitives/Surface";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { LoadingState } from "../ui/primitives/EmptyState";
import { cn } from "../ui/internal/cn";
import { styles, outcomeColors } from "./analytics/presentation";

interface AnalyticsScreenProps {
  decks: DeckSummary[];
  launchRequest: AnalyticsLaunchRequest | null;
  onLaunchConsumed: () => void;
  onPracticeScope: (deckIds: string[], tagIds: string[]) => void;
  onSearchTag: (tagId: string) => void | Promise<void>;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

const dateTimeFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });
const dateFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });
const compactDateFormatter = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });

/** Bounded current-inventory projections and immutable review evidence share presentation, not data ownership. */
export function AnalyticsScreen({ decks, launchRequest, onLaunchConsumed, onPracticeScope, onSearchTag, showToast }: AnalyticsScreenProps) {
  const [range, setRange] = useState<AnalyticsRange>("7d");
  const [customStartLocalDate, setCustomStartLocalDate] = useState(() => localDateDaysAgo(6));
  const [customEndLocalDate, setCustomEndLocalDate] = useState(() => localDateDaysAgo(0));
  const [customDatesChosen, setCustomDatesChosen] = useState(false);
  const [selectedDeckIds, setSelectedDeckIds] = useState<Set<string>>(new Set());
  const [selectedTagIds, setSelectedTagIds] = useState<Set<string>>(new Set());
  const [highlightedSessionId, setHighlightedSessionId] = useState<string | undefined>();
  const [analytics, setAnalytics] = useState<Analytics | null>(null);
  const [showChartData, setShowChartData] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [practicingTagId, setPracticingTagId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const handledLaunchRef = useRef<number | null>(null);
  const controllerRef = useRef<AnalyticsController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new AnalyticsController();
  const controller = controllerRef.current;
  const deckIds = useMemo(() => [...selectedDeckIds].sort(), [selectedDeckIds]);
  const tagIds = useMemo(() => [...selectedTagIds].sort(), [selectedTagIds]);

  const load = useCallback(async (): Promise<void> => {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const result = await controller.load({
        range,
        deckIds,
        tagIds,
        ...(range === "custom" ? { customStartLocalDate, customEndLocalDate } : {}),
        highlightedSessionId,
      });
      if (!result) return;
      setAnalytics(result);
      const availableDeckIds = new Set(result.dimensionOptions.decks.map((deck) => deck.id));
      const availableTagIds = new Set(result.dimensionOptions.tags.map((tag) => tag.id));
      setSelectedDeckIds((current) => filteredSet(current, availableDeckIds));
      setSelectedTagIds((current) => filteredSet(current, availableTagIds));
      setIsLoading(false);
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
      setIsLoading(false);
    }
  }, [controller, customEndLocalDate, customStartLocalDate, deckIds, highlightedSessionId, range, showToast, tagIds]);

  useEffect(() => {
    void load();
    return () => controller.invalidate();
  }, [controller, load]);

  useEffect(() => {
    if (!launchRequest || handledLaunchRef.current === launchRequest.requestId) return;
    handledLaunchRef.current = launchRequest.requestId;
    setRange(launchRequest.range);
    setSelectedDeckIds(new Set(launchRequest.deckIds));
    setSelectedTagIds(new Set(launchRequest.tagIds));
    setHighlightedSessionId(launchRequest.highlightedSessionId);
    onLaunchConsumed();
  }, [launchRequest, onLaunchConsumed]);

  function practiceReadyTag(tag: TagAnalytics): void {
    if (practicingTagId !== null || tag.studyableDueCount === 0) return;
    setPracticingTagId(tag.tagId);
    onPracticeScope(deckIds, [...new Set([...tagIds, tag.tagId])]);
  }

  const deckOptions = analytics?.dimensionOptions.decks ?? [];
  const tagOptions = analytics?.dimensionOptions.tags ?? [];
  // The study date comes from native: it starts at the "New day starts at"
  // hour, so this device's calendar date can be a day ahead after midnight.
  const todayLocalDate = analytics?.scope.todayLocalDate ?? null;

  function changeRange(next: AnalyticsRange): void {
    if (next === "custom" && !customDatesChosen && todayLocalDate !== null) {
      setCustomStartLocalDate(shiftLocalDate(todayLocalDate, -6));
      setCustomEndLocalDate(todayLocalDate);
    }
    setRange(next);
  }

  return (
    <PageScaffold width="full" className="tw:[&>section]:mb-6 tw:[&>div]:mb-6">
      <PageHeader
        title="Analytics"
        icon="analytics"
        eyebrow="Learning evidence"
        description="Understand your learning progress, find topics that need attention, and choose what to study next."
        actions={<Button variant="secondary" disabled={isLoading} onClick={() => void load()}>{isLoading ? "Analyzing…" : "Refresh"}</Button>}
      />
      <AnalyticsFilters
        range={range}
        customStartLocalDate={customStartLocalDate}
        customEndLocalDate={customEndLocalDate}
        todayLocalDate={todayLocalDate ?? localDateDaysAgo(0)}
        selectedDeckIds={selectedDeckIds}
        selectedTagIds={selectedTagIds}
        highlightedSessionId={highlightedSessionId}
        deckOptions={deckOptions}
        tagOptions={tagOptions}
        decks={decks}
        onRangeChange={changeRange}
        onCustomStartLocalDateChange={(value) => {
          setCustomDatesChosen(true);
          setCustomStartLocalDate(value);
        }}
        onCustomEndLocalDateChange={(value) => {
          setCustomDatesChosen(true);
          setCustomEndLocalDate(value);
        }}
        onDeckIdsChange={setSelectedDeckIds}
        onTagIdsChange={setSelectedTagIds}
        onClearHighlightedSession={() => setHighlightedSessionId(undefined)}
        onClearScope={() => {
          setSelectedDeckIds(new Set());
          setSelectedTagIds(new Set());
          setHighlightedSessionId(undefined);
        }}
      />
      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}

      <Surface
        as="section"
        className={styles.analyticsPanelProgressChartPanel}
        aria-labelledby="progress-heading"
        aria-busy={isLoading || undefined}
      >
        <div className={styles.panelHeadingRowAnalyticsChartHeading}>
          <div>
            <h2 id="progress-heading">Learning progress</h2>
            <p className={styles.analyticsPanelNote}>Buckets use the local review dates captured when each answer was submitted.</p>
          </div>
          {analytics ? (
            <div className={styles.analyticsChartActions}>
              <Button variant="text" aria-expanded={showChartData} disabled={isLoading} onClick={() => setShowChartData((value) => !value)}>
                {showChartData ? "Hide chart data" : "View chart data"}
              </Button>
            </div>
          ) : null}
        </div>
        {isLoading ? (
          <div className="tw:grid tw:min-h-72 tw:place-items-center">
            <LoadingState label="Analyzing data…" />
          </div>
        ) : analytics ? (
          <ProgressChart buckets={analytics.timeline} range={analytics.scope.range} />
        ) : (
          <div className="tw:grid tw:min-h-40 tw:place-items-center tw:text-sm tw:text-muted-foreground">
            Analytics will appear here when the data can be analyzed.
          </div>
        )}
        {analytics && !isLoading && showChartData ? <TimelineTable analytics={analytics} /> : null}
      </Surface>

      {analytics ? <>
        <section className={styles.metricsGridAnalyticsMetrics} aria-label="Learning summary">
          <MetricTile label="Cards practiced" value={analytics.history.cardsPracticed} detail={`${analytics.history.reviewCount} total reviews`} />
          <MetricTile label="Remembered" value={formatPercent(analytics.history.correctRate)} detail={`${analytics.history.correctCount} remembered answers`} />
          <MetricTile label="Focused study" value={formatStudyTime(analytics.history.totalStudyTimeMs)} detail={`${analytics.history.sessionCount} session${analytics.history.sessionCount === 1 ? "" : "s"}`} />
          <MetricTile label="Average confidence" value={formatConfidence(analytics.history.averageConfidence)} detail="0 Wrong · 3 Easy" />
          <MetricTile label="Ready now" value={analytics.currentInventory.studyableDueCount} detail={`${analytics.currentInventory.studyableCardCount} studyable generated cards`} />
          <MetricTile label="Needs more practice" value={analytics.currentInventory.needsMorePracticeCount} detail="Repeatedly missed current cards" />
        </section>
        <section className={styles.analyticsWorkloadStrip} aria-label="Today's workload">
          <div><strong>Today</strong><span>{analytics.dailyLimits.reviewsCompleted} / {analytics.dailyLimits.reviewLimit} reviews</span>
            <span>{analytics.dailyLimits.newCompleted} / {analytics.dailyLimits.newLimit} new cards</span></div>
          <p>{workloadMessage(analytics)}</p>
        </section>
        <QuestionTypeBreakdown rows={analytics.questionTypes} />
        <Surface as="section" className={styles.analyticsPanelTagInsightsPanel}>
          <div className={styles.panelHeadingRow}><div><h2>Topic insights</h2>
            <p className={styles.analyticsPanelNote}>Current counts use current tags. Review evidence uses captured tag identity and displays the current tag name after renames.</p>
          </div></div>
          {analytics.tagStats.length === 0 ? <p className={styles.mutedCopy}>Add tags and complete reviews to see topic-level patterns.</p> : (
            <div className={styles.analyticsTableWrap}><table className={styles.analyticsTableTopicTable}>
              <thead><tr><th>Topic</th><th>Last practiced</th><th>Practice</th><th>Remembered</th><th>Trend</th><th>Ready</th><th>Needs practice</th><th>Status</th><th /></tr></thead>
              <tbody>{analytics.tagStats.map((tag) => <tr key={tag.tagId}>
                <td><strong>{tag.tagName}</strong><small>{tag.generatedCardCount} cards</small></td>
                <td>{formatLastPracticed(tag.lastPracticedAt)}</td>
                <td>{tag.cardsPracticed} cards<small>{tag.reviewCount} reviews · {formatStudyTime(tag.totalStudyTimeMs)}</small></td>
                <td>{formatPercent(tag.correctRate)}<small>{formatConfidence(tag.averageConfidence)}</small></td>
                <td><Trend value={tag.trendDelta} /></td><td>{tag.studyableDueCount}</td><td>{tag.needsMorePracticeCount}</td>
                <td><StatusBadge status={tag.status} /></td>
                <td><Button variant="primary" size="small" disabled={tag.studyableDueCount === 0 || practicingTagId !== null} onClick={() => practiceReadyTag(tag)}>{practicingTagId === tag.tagId ? "Starting…" : "Practice"}</Button></td>
              </tr>)}</tbody>
            </table></div>
          )}
        </Surface>
        <div className={styles.analyticsGridAnalyticsDetailGrid}>
          <Surface as="section" className={styles.analyticsPanel}>
            <div className={styles.panelHeadingRow}><h2>Deck performance</h2></div>
            {analytics.deckStats.length === 0 ? <p className={styles.mutedCopy}>No current inventory or history in this scope.</p> : (
              <div className={styles.analyticsTableWrap}><table className={styles.analyticsTable}>
                <thead><tr><th>Deck</th><th>Ready</th><th>Practice</th><th>Remembered</th><th>Time</th></tr></thead>
                <tbody>{analytics.deckStats.map((deck) => <tr key={deck.deckId}>
                  <td><strong>{deck.deckName}</strong><small>{deck.isArchived ? "Archived · " : ""}{deck.generatedCardCount} generated · {deck.needsMorePracticeCount} need practice</small></td>
                  <td>{deck.studyableDueCount}</td><td>{deck.cardsPracticed} cards<small>{deck.reviewCount} reviews</small></td>
                  <td>{formatPercent(deck.correctRate)}<small>{formatConfidence(deck.averageConfidence)}</small></td>
                  <td>{formatStudyTime(deck.totalStudyTimeMs)}<small>{formatDuration(deck.averageResponseTimeMs)} avg</small></td>
                </tr>)}</tbody>
              </table></div>
            )}
          </Surface>
          <Surface as="section" className={styles.analyticsPanel}>
            <div className={styles.panelHeadingRow}><h2>Recent sessions</h2></div>
            {analytics.recentSessions.length === 0 ? <p className={styles.mutedCopy}>Complete a practice session to track session time.</p> : (
              <div className={styles.analyticsSessionList}>{analytics.recentSessions.map((session) => <article key={session.id} className={cn(session.highlighted && styles.highlighted)}>
                <div><strong>{session.deckNames.join(", ") || "Custom selection"}</strong><span>{session.mode} · {dateTimeFormatter.format(new Date(session.startedAt))}</span></div>
                <div><strong>{formatStudyTime(session.studyTimeMs)}</strong><span>{session.ratedCount} reviewed · {formatPercent(session.correctRate)}</span><small>{formatStudyTime(session.durationMs)} elapsed</small></div>
              </article>)}</div>
            )}
          </Surface>
        </div>
        <Surface as="section" className={styles.analyticsPanelRecentReviewPanel}>
          <div className={styles.panelHeadingRow}><h2>Recent reviews</h2></div>
          {analytics.recentReviews.length === 0 ? <div className={styles.compactEmptyState}><p>Complete a practice session to begin tracking review results.</p></div> : (
            <div className={styles.recentReviewList}>{analytics.recentReviews.map((review) => <article key={review.id} className={styles.recentReviewRow}>
              <span className={cn(styles.reviewDot, outcomeColors[(review.rating - 1) as keyof typeof outcomeColors])} aria-hidden="true" />
              <div className={styles.recentReviewCopy}>
                <strong>{summarize(review.questionExcerpt)}</strong><span>{review.deckName} · {outcomeLabel(review.outcome)}</span>
                {review.capturedDeckName !== review.deckName ? <small>Reviewed while named “{review.capturedDeckName}”</small> : null}
                {review.tags.length > 0 ? <small className={styles.recentReviewTags}>{review.tags.map((tag, index) => <span key={tag.tagId}>
                  {index > 0 ? " · " : null}{tag.isCurrent ? <Button variant="text" className={styles.recentReviewTagAction}
                    aria-label={`Search cards tagged ${tag.tagName}`} onClick={() => void onSearchTag(tag.tagId)}>{tag.tagName}</Button> : tag.tagName}
                </span>)}</small> : null}
              </div>
              <div className={styles.recentReviewMeta}>
                <strong>{formatDuration(review.responseTimeMs)}</strong><span>{dateTimeFormatter.format(new Date(review.reviewedAt))}</span>
                <small>Next: {formatDueDate(review.dueAfter)}</small>
                {review.attributionSource === "legacy_current_state_backfill" ? <small>Legacy attribution reconstructed</small> : null}
              </div>
            </article>)}</div>
          )}
        </Surface>
        {analytics.attributionCoverage.legacyBackfilledReviewCount > 0 ? <InlineNotice>
          {analytics.attributionCoverage.legacyBackfilledReviewCount} older review{analytics.attributionCoverage.legacyBackfilledReviewCount === 1 ? " was" : "s were"} reconstructed from the card’s state during the one-time schema repair.
        </InlineNotice> : null}
      </> : null}
    </PageScaffold>
  );
}

function TimelineTable({ analytics }: { analytics: Analytics }) {
  return <div className={styles.analyticsTableWrapChartDataTable}><table className={styles.analyticsTable}>
    <caption>Data represented in the learning progress chart</caption>
    <thead><tr><th>Local period</th><th>Cards</th><th>Reviews</th><th>Wrong</th><th>Hard</th><th>Medium</th><th>Easy</th><th>Remembered</th><th>Study time</th></tr></thead>
    <tbody>{analytics.timeline.map((bucket) => <tr key={`${bucket.startLocalDate}-${bucket.endLocalDate}`} className={cn(bucket.containsHighlightedSession && styles.highlighted)}>
      <td>{compactDateFormatter.format(new Date(bucket.startAt))}</td><td>{bucket.cardsPracticed}</td><td>{bucket.reviewCount}</td>
      <td>{bucket.wrongCount}</td><td>{bucket.hardCount}</td><td>{bucket.mediumCount}</td><td>{bucket.easyCount}</td>
      <td>{formatPercent(bucket.correctRate)}</td><td>{formatStudyTime(bucket.studyTimeMs)}</td>
    </tr>)}</tbody>
  </table></div>;
}

function Trend({ value }: { value: number | null }) {
  if (value === null) return <span className={cn(styles.trend, styles.neutral)}>—</span>;
  const points = Math.round(value * 100);
  return <span className={cn(styles.trend, points > 0 ? styles.positive : points < 0 ? styles.negative : styles.neutral)}>
    {points > 0 ? "↑" : points < 0 ? "↓" : "→"} {Math.abs(points)} pt
  </span>;
}

function StatusBadge({ status }: { status: TagAnalytics["status"] }) {
  const labels = { building_data: "Building data", needs_attention: "Needs attention", on_track: "On track", strong: "Strong" };
  const tones = { building_data: "neutral", needs_attention: "warning", on_track: "accent", strong: "success" } as const;
  return <Badge tone={tones[status]}>{labels[status]}</Badge>;
}

function workloadMessage(analytics: Analytics): string {
  if (analytics.dailyLimits.reviewsRemaining === 0) return "Today's review limit is reached. Ready cards remain scheduled; extra study is still available from a deck’s Practice action in Flashcards.";
  if (analytics.currentInventory.studyableDueCount > analytics.dailyLimits.reviewsRemaining) return `${analytics.currentInventory.studyableDueCount - analytics.dailyLimits.reviewsRemaining} ready cards exceed today's remaining review capacity.`;
  return `${analytics.dailyLimits.reviewsRemaining} reviews and ${analytics.dailyLimits.newRemaining} new cards remain in today's limits.`;
}

function formatPercent(value: number | null): string { return value === null ? "—" : `${Math.round(value * 100)}%`; }
function formatConfidence(value: number | null): string { return value === null ? "—" : `${value.toFixed(1)} / 3`; }
function formatDuration(value: number | null): string {
  if (value === null) return "—";
  if (value < 1_000) return `${Math.round(value)} ms`;
  return `${(value / 1_000).toFixed(value < 10_000 ? 1 : 0)} s`;
}
function formatStudyTime(value: number): string {
  if (value < 60_000) return value === 0 ? "0 min" : "<1 min";
  if (value < 3_600_000) return `${Math.round(value / 60_000)} min`;
  const hours = value / 3_600_000;
  return `${hours.toFixed(hours < 10 ? 1 : 0)} hr`;
}
function formatDueDate(value: number): string { return value <= Date.now() ? "ready now" : dateFormatter.format(new Date(value)); }
function formatLastPracticed(value: number | null): string { return value === null ? "Never" : dateFormatter.format(new Date(value)); }
function outcomeLabel(value: ReviewOutcome): string {
  switch (value) {
    case "wrong": return "0 · Wrong";
    case "not_confident": return "1 · Hard";
    case "somewhat_confident": return "2 · Medium";
    case "very_confident": return "3 · Easy";
  }
}
function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 120 ? `${normalized.slice(0, 117)}…` : normalized;
}
function filteredSet(current: Set<string>, available: Set<string>): Set<string> {
  const next = new Set([...current].filter((id) => available.has(id)));
  return sameSet(current, next) ? current : next;
}
function sameSet(left: Set<string>, right: Set<string>): boolean {
  return left.size === right.size && [...left].every((value) => right.has(value));
}
/** A `YYYY-MM-DD` date moved by whole dates, independent of any time zone. */
function shiftLocalDate(value: string, days: number): string {
  const date = new Date(`${value}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}
function localDateDaysAgo(days: number): string {
  const value = new Date();
  value.setHours(12, 0, 0, 0);
  value.setDate(value.getDate() - days);
  const year = value.getFullYear();
  const month = String(value.getMonth() + 1).padStart(2, "0");
  const day = String(value.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}
