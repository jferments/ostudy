import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { AnalyticsLaunchRequest } from "../App";
import { AnalyticsController } from "../controllers/analyticsController";
import {
  type Analytics,
  type AnalyticsRange,
  type TagAnalytics,
} from "../ipc/analytics";
import {
  getErrorMessage,
} from "../ipc/common";
import {
  type DeckSummary,
} from "../ipc/decks";
import {
  type ReviewOutcome,
} from "../ipc/practice";
import {
  type AppSettings,
} from "../ipc/settings";
import { AnalyticsFilters } from "./analytics/AnalyticsFilters";
import { ProgressChart } from "./analytics/ProgressChart";
import { Icon } from "../ui/icons/Icon";
import { ownedClassNames, sharedClassMaps } from "../ui/styles/classOwnership";
import ownershipStyles from "./AnalyticsScreen.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;


interface AnalyticsScreenProps {
  decks: DeckSummary[];
  settings: AppSettings;
  launchRequest: AnalyticsLaunchRequest | null;
  onLaunchConsumed: () => void;
  onPracticeScope: (deckIds: string[], tagIds: string[]) => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

const dateTimeFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});
const dateFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });
const compactDateFormatter = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
});

/** Displays bounded current-inventory projections and immutable review evidence. */
export function AnalyticsScreen({
  decks,
  settings,
  launchRequest,
  onLaunchConsumed,
  onPracticeScope,
  showToast,
}: AnalyticsScreenProps) {
  const [range, setRange] = useState<AnalyticsRange>("7d");
  const [customStartLocalDate, setCustomStartLocalDate] = useState(() => localDateDaysAgo(6));
  const [customEndLocalDate, setCustomEndLocalDate] = useState(() => localDateDaysAgo(0));
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
        ...(range === "custom"
          ? { customStartLocalDate, customEndLocalDate }
          : {}),
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
  }, [
    controller,
    customEndLocalDate,
    customStartLocalDate,
    deckIds,
    highlightedSessionId,
    range,
    showToast,
    tagIds,
  ]);

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
    const scopedTags = [...new Set([...tagIds, tag.tagId])];
    onPracticeScope(deckIds, scopedTags);
  }

  const deckOptions = analytics?.dimensionOptions.decks ?? [];
  const tagOptions = analytics?.dimensionOptions.tags ?? [];

  return (
    <div className={ownedClassNames(ownedStyleMaps, "screen analytics-screen")}>
      <header className={ownedClassNames(ownedStyleMaps, "screen-header compact-screen-header")}>
        <div>
          <span className={ownershipStyles.screenIcon} aria-hidden="true">
            <Icon name="analytics" size={22} />
          </span>
          <span className={ownedClassNames(ownedStyleMaps, "eyebrow")}>Learning evidence</span>
          <h1>Analytics</h1>
          <p>
            Current inventory follows your present organization. Historical evidence follows card
            moves and current deck/tag names while retaining immutable review presentation details.
          </p>
        </div>
        <button
          type="button"
          className={ownedClassNames(ownedStyleMaps, "secondary-button")}
          disabled={isLoading}
          onClick={() => void load()}
        >
          {isLoading ? "Calculating…" : "Refresh"}
        </button>
      </header>

      <AnalyticsFilters
        range={range}
        customStartLocalDate={customStartLocalDate}
        customEndLocalDate={customEndLocalDate}
        selectedDeckIds={selectedDeckIds}
        selectedTagIds={selectedTagIds}
        highlightedSessionId={highlightedSessionId}
        deckOptions={deckOptions}
        tagOptions={tagOptions}
        decks={decks}
        onRangeChange={setRange}
        onCustomStartLocalDateChange={setCustomStartLocalDate}
        onCustomEndLocalDateChange={setCustomEndLocalDate}
        onDeckIdsChange={setSelectedDeckIds}
        onTagIdsChange={setSelectedTagIds}
        onClearHighlightedSession={() => setHighlightedSessionId(undefined)}
        onClearScope={() => {
          setSelectedDeckIds(new Set());
          setSelectedTagIds(new Set());
          setHighlightedSessionId(undefined);
        }}
      />

      {errorMessage ? <div className={ownedClassNames(ownedStyleMaps, "inline-alert error-alert")}>{errorMessage}</div> : null}
      {isLoading && analytics === null ? (
        <div className={ownedClassNames(ownedStyleMaps, "inline-progress")}>Calculating learning analytics…</div>
      ) : null}

      {analytics ? (
        <>
          <section
            className={ownedClassNames(ownedStyleMaps, "analytics-panel progress-chart-panel")}
            aria-labelledby="progress-heading"
          >
            <div className={ownedClassNames(ownedStyleMaps, "panel-heading-row analytics-chart-heading")}>
              <div>
                <h2 id="progress-heading">Learning progress</h2>
                <p className={ownedClassNames(ownedStyleMaps, "analytics-panel-note")}>
                  Buckets use the local review dates captured when each answer was submitted.
                </p>
              </div>
              <div className={ownedClassNames(ownedStyleMaps, "analytics-chart-actions")}>
                <button
                  type="button"
                  className={ownedClassNames(ownedStyleMaps, "text-button")}
                  aria-expanded={showChartData}
                  onClick={() => setShowChartData((value) => !value)}
                >
                  {showChartData ? "Hide chart data" : "View chart data"}
                </button>
              </div>
            </div>
            <ProgressChart
              buckets={analytics.timeline}
              range={analytics.scope.range}
              reduceMotion={settings.reduceMotion}
            />
            {showChartData ? <TimelineTable analytics={analytics} /> : null}
          </section>

          <section className={ownedClassNames(ownedStyleMaps, "metrics-grid analytics-metrics")} aria-label="Learning summary">
            <MetricCard
              label="Cards practiced"
              value={analytics.history.cardsPracticed}
              detail={`${analytics.history.reviewCount} total reviews`}
            />
            <MetricCard
              label="Remembered"
              value={formatPercent(analytics.history.correctRate)}
              detail={`${analytics.history.correctCount} remembered answers`}
            />
            <MetricCard
              label="Focused study"
              value={formatStudyTime(analytics.history.totalStudyTimeMs)}
              detail={`${analytics.history.sessionCount} session${analytics.history.sessionCount === 1 ? "" : "s"}`}
            />
            <MetricCard
              label="Average confidence"
              value={formatConfidence(analytics.history.averageConfidence)}
              detail="0 Wrong · 3 Easy"
            />
            <MetricCard
              label="Ready now"
              value={analytics.currentInventory.studyableDueCount}
              detail={`${analytics.currentInventory.studyableCardCount} studyable generated cards`}
            />
            <MetricCard
              label="Needs more practice"
              value={analytics.currentInventory.needsMorePracticeCount}
              detail="Repeatedly missed current cards"
            />
          </section>

          <section className={ownedClassNames(ownedStyleMaps, "analytics-workload-strip")} aria-label="Today's workload">
            <div>
              <strong>Today</strong>
              <span>
                {analytics.dailyLimits.reviewsCompleted} / {analytics.dailyLimits.reviewLimit} reviews
              </span>
              <span>
                {analytics.dailyLimits.newCompleted} / {analytics.dailyLimits.newLimit} new cards
              </span>
            </div>
            <p>{workloadMessage(analytics)}</p>
          </section>

          <section className={ownedClassNames(ownedStyleMaps, "analytics-panel tag-insights-panel")}>
            <div className={ownedClassNames(ownedStyleMaps, "panel-heading-row")}>
              <div>
                <h2>Topic insights</h2>
                <p className={ownedClassNames(ownedStyleMaps, "analytics-panel-note")}>
                  Current counts use current tags. Review evidence uses captured tag identity and
                  displays the current tag name after renames.
                </p>
              </div>
            </div>
            {analytics.tagStats.length === 0 ? (
              <p className={ownedClassNames(ownedStyleMaps, "muted-copy")}>Add tags and complete reviews to see topic-level patterns.</p>
            ) : (
              <div className={ownedClassNames(ownedStyleMaps, "analytics-table-wrap")}>
                <table className={ownedClassNames(ownedStyleMaps, "analytics-table topic-table")}>
                  <thead>
                    <tr>
                      <th>Topic</th>
                      <th>Last practiced</th>
                      <th>Practice</th>
                      <th>Remembered</th>
                      <th>Trend</th>
                      <th>Ready</th>
                      <th>Needs practice</th>
                      <th>Status</th>
                      <th />
                    </tr>
                  </thead>
                  <tbody>
                    {analytics.tagStats.map((tag) => (
                      <tr key={tag.tagId}>
                        <td>
                          <strong>{tag.tagName}</strong>
                          <small>
                            {tag.generatedCardCount} cards
                          </small>
                        </td>
                        <td>{formatLastPracticed(tag.lastPracticedAt)}</td>
                        <td>
                          {tag.cardsPracticed} cards
                          <small>
                            {tag.reviewCount} reviews · {formatStudyTime(tag.totalStudyTimeMs)}
                          </small>
                        </td>
                        <td>
                          {formatPercent(tag.correctRate)}
                          <small>{formatConfidence(tag.averageConfidence)}</small>
                        </td>
                        <td>
                          <Trend value={tag.trendDelta} />
                        </td>
                        <td>{tag.studyableDueCount}</td>
                        <td>{tag.needsMorePracticeCount}</td>
                        <td>
                          <StatusBadge status={tag.status} />
                        </td>
                        <td>
                          <button
                            type="button"
                            className={ownedClassNames(ownedStyleMaps, "small-primary-button")}
                            disabled={tag.studyableDueCount === 0 || practicingTagId !== null}
                            onClick={() => practiceReadyTag(tag)}
                          >
                            {practicingTagId === tag.tagId ? "Starting…" : "Practice ready"}
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          <div className={ownedClassNames(ownedStyleMaps, "analytics-grid analytics-detail-grid")}>
            <section className={ownedClassNames(ownedStyleMaps, "analytics-panel")}>
              <div className={ownedClassNames(ownedStyleMaps, "panel-heading-row")}>
                <h2>Deck performance</h2>
              </div>
              {analytics.deckStats.length === 0 ? (
                <p className={ownedClassNames(ownedStyleMaps, "muted-copy")}>No current inventory or history in this scope.</p>
              ) : (
                <div className={ownedClassNames(ownedStyleMaps, "analytics-table-wrap")}>
                  <table className={ownedClassNames(ownedStyleMaps, "analytics-table")}>
                    <thead>
                      <tr>
                        <th>Deck</th>
                        <th>Ready</th>
                        <th>Practice</th>
                        <th>Remembered</th>
                        <th>Time</th>
                      </tr>
                    </thead>
                    <tbody>
                      {analytics.deckStats.map((deck) => (
                        <tr key={deck.deckId}>
                          <td>
                            <strong>{deck.deckName}</strong>
                            <small>
                              {deck.isArchived ? "Archived · " : ""}
                              {deck.generatedCardCount} generated · {deck.needsMorePracticeCount} need
                              practice
                            </small>
                          </td>
                          <td>{deck.studyableDueCount}</td>
                          <td>
                            {deck.cardsPracticed} cards
                            <small>{deck.reviewCount} reviews</small>
                          </td>
                          <td>
                            {formatPercent(deck.correctRate)}
                            <small>{formatConfidence(deck.averageConfidence)}</small>
                          </td>
                          <td>
                            {formatStudyTime(deck.totalStudyTimeMs)}
                            <small>{formatDuration(deck.averageResponseTimeMs)} avg</small>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </section>

            <section className={ownedClassNames(ownedStyleMaps, "analytics-panel")}>
              <div className={ownedClassNames(ownedStyleMaps, "panel-heading-row")}>
                <h2>Recent sessions</h2>
              </div>
              {analytics.recentSessions.length === 0 ? (
                <p className={ownedClassNames(ownedStyleMaps, "muted-copy")}>Complete a practice session to track session time.</p>
              ) : (
                <div className={ownedClassNames(ownedStyleMaps, "analytics-session-list")}>
                  {analytics.recentSessions.map((session) => (
                    <article
                      key={session.id}
                      className={ownedClassNames(ownedStyleMaps, session.highlighted ? "highlighted" : "")}
                    >
                      <div>
                        <strong>{session.deckNames.join(", ") || "Custom selection"}</strong>
                        <span>
                          {session.mode} · {dateTimeFormatter.format(new Date(session.startedAt))}
                        </span>
                      </div>
                      <div>
                        <strong>{formatStudyTime(session.studyTimeMs)}</strong>
                        <span>
                          {session.ratedCount} reviewed · {formatPercent(session.correctRate)}
                        </span>
                        <small>{formatStudyTime(session.durationMs)} elapsed</small>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </section>
          </div>

          <section className={ownedClassNames(ownedStyleMaps, "analytics-panel recent-review-panel")}>
            <div className={ownedClassNames(ownedStyleMaps, "panel-heading-row")}>
              <h2>Recent reviews</h2>
            </div>
            {analytics.recentReviews.length === 0 ? (
              <div className={ownedClassNames(ownedStyleMaps, "compact-empty-state")}>
                <p>Complete a practice session to begin tracking review results.</p>
              </div>
            ) : (
              <div className={ownedClassNames(ownedStyleMaps, "recent-review-list")}>
                {analytics.recentReviews.map((review) => (
                  <article key={review.id} className={ownedClassNames(ownedStyleMaps, "recent-review-row")}>
                    <span
                      className={ownedClassNames(ownedStyleMaps, `review-result-dot confidence-${review.rating - 1}`)}
                      aria-hidden="true"
                    />
                    <div className={ownedClassNames(ownedStyleMaps, "recent-review-copy")}>
                      <strong>{summarize(review.questionExcerpt)}</strong>
                      <span>
                        {review.deckName} · {outcomeLabel(review.outcome)}
                      </span>
                      {review.capturedDeckName !== review.deckName ? (
                        <small>Reviewed while named “{review.capturedDeckName}”</small>
                      ) : null}
                      {review.tags.length > 0 ? <small>{review.tags.join(" · ")}</small> : null}
                    </div>
                    <div className={ownedClassNames(ownedStyleMaps, "recent-review-meta")}>
                      <strong>{formatDuration(review.responseTimeMs)}</strong>
                      <span>{dateTimeFormatter.format(new Date(review.reviewedAt))}</span>
                      <small>Next: {formatDueDate(review.dueAfter)}</small>
                      {review.attributionSource === "legacy_current_state_backfill" ? (
                        <small>Legacy attribution reconstructed</small>
                      ) : null}
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>

          {analytics.attributionCoverage.legacyBackfilledReviewCount > 0 ? (
            <div className={ownedClassNames(ownedStyleMaps, "inline-alert info-alert")}>
              {analytics.attributionCoverage.legacyBackfilledReviewCount} older review
              {analytics.attributionCoverage.legacyBackfilledReviewCount === 1 ? " was" : "s were"}
              reconstructed from the card’s state during the one-time schema repair.
            </div>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

interface MetricCardProps {
  label: string;
  value: number | string;
  detail: string;
}
function MetricCard({ label, value, detail }: MetricCardProps) {
  return (
    <article className={ownedClassNames(ownedStyleMaps, "metric-card")}>
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}

function TimelineTable({ analytics }: { analytics: Analytics }) {
  return (
    <div className={ownedClassNames(ownedStyleMaps, "analytics-table-wrap chart-data-table")}>
      <table className={ownedClassNames(ownedStyleMaps, "analytics-table")}>
        <caption>Data represented in the learning progress chart</caption>
        <thead>
          <tr>
            <th>Local period</th>
            <th>Cards</th>
            <th>Reviews</th>
            <th>Wrong</th>
            <th>Hard</th>
            <th>Medium</th>
            <th>Easy</th>
            <th>Remembered</th>
            <th>Study time</th>
          </tr>
        </thead>
        <tbody>
          {analytics.timeline.map((bucket) => (
            <tr
              key={`${bucket.startLocalDate}-${bucket.endLocalDate}`}
              className={ownedClassNames(ownedStyleMaps, bucket.containsHighlightedSession ? "highlighted" : "")}
            >
              <td>{compactDateFormatter.format(new Date(bucket.startAt))}</td>
              <td>{bucket.cardsPracticed}</td>
              <td>{bucket.reviewCount}</td>
              <td>{bucket.wrongCount}</td>
              <td>{bucket.hardCount}</td>
              <td>{bucket.mediumCount}</td>
              <td>{bucket.easyCount}</td>
              <td>{formatPercent(bucket.correctRate)}</td>
              <td>{formatStudyTime(bucket.studyTimeMs)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Trend({ value }: { value: number | null }) {
  if (value === null) return <span className={ownedClassNames(ownedStyleMaps, "trend neutral")}>—</span>;
  const points = Math.round(value * 100);
  return (
    <span
      className={ownedClassNames(ownedStyleMaps, points > 0 ? "trend positive" : points < 0 ? "trend negative" : "trend neutral")}
    >
      {points > 0 ? "↑" : points < 0 ? "↓" : "→"} {Math.abs(points)} pt
    </span>
  );
}
function StatusBadge({ status }: { status: TagAnalytics["status"] }) {
  const labels = {
    building_data: "Building data",
    needs_attention: "Needs attention",
    on_track: "On track",
    strong: "Strong",
  };
  return <span className={ownedClassNames(ownedStyleMaps, `analytics-status ${status}`)}>{labels[status]}</span>;
}
function workloadMessage(analytics: Analytics): string {
  if (analytics.dailyLimits.reviewsRemaining === 0) {
    return "Today's review limit is reached. Ready cards remain scheduled; extra study is still available from Practice.";
  }
  if (analytics.currentInventory.studyableDueCount > analytics.dailyLimits.reviewsRemaining) {
    return `${
      analytics.currentInventory.studyableDueCount - analytics.dailyLimits.reviewsRemaining
    } ready cards exceed today's remaining review capacity.`;
  }
  return `${analytics.dailyLimits.reviewsRemaining} reviews and ${analytics.dailyLimits.newRemaining} new cards remain in today's limits.`;
}
function formatPercent(value: number | null): string {
  return value === null ? "—" : `${Math.round(value * 100)}%`;
}
function formatConfidence(value: number | null): string {
  return value === null ? "—" : `${value.toFixed(1)} / 3`;
}
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
function formatDueDate(value: number): string {
  if (value <= Date.now()) return "ready now";
  return dateFormatter.format(new Date(value));
}
function formatLastPracticed(value: number | null): string {
  return value === null ? "Never" : dateFormatter.format(new Date(value));
}
function outcomeLabel(value: ReviewOutcome): string {
  switch (value) {
    case "wrong":
      return "0 · Wrong";
    case "not_confident":
      return "1 · Hard";
    case "somewhat_confident":
      return "2 · Medium";
    case "very_confident":
      return "3 · Easy";
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

function localDateDaysAgo(days: number): string {
  const value = new Date();
  value.setHours(12, 0, 0, 0);
  value.setDate(value.getDate() - days);
  const year = value.getFullYear();
  const month = String(value.getMonth() + 1).padStart(2, "0");
  const day = String(value.getDate()).padStart(2, "0");
  return year + "-" + month + "-" + day;
}
