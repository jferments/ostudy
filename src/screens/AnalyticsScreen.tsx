import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { AnalyticsLaunchRequest } from "../App";
import {
  getAnalytics,
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
import { ProgressChart } from "./analytics/ProgressChart";

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
  const [selectedDeckIds, setSelectedDeckIds] = useState<Set<string>>(new Set());
  const [selectedTagIds, setSelectedTagIds] = useState<Set<string>>(new Set());
  const [highlightedSessionId, setHighlightedSessionId] = useState<string | undefined>();
  const [analytics, setAnalytics] = useState<Analytics | null>(null);
  const [chartMetric, setChartMetric] = useState<"cards" | "time">("cards");
  const [showChartData, setShowChartData] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [practicingTagId, setPracticingTagId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const handledLaunchRef = useRef<number | null>(null);
  const requestSequence = useRef(0);

  const deckIds = useMemo(() => [...selectedDeckIds].sort(), [selectedDeckIds]);
  const tagIds = useMemo(() => [...selectedTagIds].sort(), [selectedTagIds]);

  const load = useCallback(async (): Promise<void> => {
    const requestId = requestSequence.current + 1;
    requestSequence.current = requestId;
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const result = await getAnalytics({
        range,
        deckIds,
        tagIds,
        highlightedSessionId,
      });
      if (requestSequence.current !== requestId) return;
      setAnalytics(result);
      const availableDeckIds = new Set(result.dimensionOptions.decks.map((deck) => deck.id));
      const availableTagIds = new Set(result.dimensionOptions.tags.map((tag) => tag.id));
      setSelectedDeckIds((current) => filteredSet(current, availableDeckIds));
      setSelectedTagIds((current) => filteredSet(current, availableTagIds));
    } catch (error: unknown) {
      if (requestSequence.current !== requestId) return;
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      if (requestSequence.current === requestId) setIsLoading(false);
    }
  }, [deckIds, highlightedSessionId, range, showToast, tagIds]);

  useEffect(() => {
    void load();
  }, [load]);

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
  const selectedDeckNames = deckIds
    .map((id) => deckOptions.find((deck) => deck.id === id)?.name ?? decks.find((deck) => deck.id === id)?.name)
    .filter((name): name is string => Boolean(name));
  const selectedTagNames = tagIds
    .map((id) => tagOptions.find((tag) => tag.id === id)?.name)
    .filter((name): name is string => Boolean(name));

  return (
    <div className="screen analytics-screen">
      <header className="screen-header compact-screen-header">
        <div>
          <span className="eyebrow">Learning evidence</span>
          <h1>Analytics</h1>
          <p>
            Current inventory follows your present organization. Historical evidence follows card
            moves and current deck/tag names while retaining immutable review presentation details.
          </p>
        </div>
        <button
          type="button"
          className="secondary-button"
          disabled={isLoading}
          onClick={() => void load()}
        >
          {isLoading ? "Calculating…" : "Refresh"}
        </button>
      </header>

      <section className="analytics-filter-panel" aria-label="Analytics filters">
        <div className="analytics-range-control" role="group" aria-label="Time range">
          {([
            ["7d", "7 days"],
            ["30d", "30 days"],
            ["90d", "90 days"],
            ["1y", "1 year"],
            ["all", "All time"],
          ] as const).map(([value, label]) => (
            <button
              key={value}
              type="button"
              className={
                range === value ? "analytics-filter-button active" : "analytics-filter-button"
              }
              aria-pressed={range === value}
              onClick={() => {
                setRange(value);
                setHighlightedSessionId(undefined);
              }}
            >
              {label}
            </button>
          ))}
        </div>
        <details className="analytics-scope-details">
          <summary>
            Scope: {selectedDeckNames.length === 0 ? "All decks" : `${selectedDeckNames.length} deck${selectedDeckNames.length === 1 ? "" : "s"}`}
            {selectedTagNames.length > 0
              ? ` · ${selectedTagNames.length} tag${selectedTagNames.length === 1 ? "" : "s"}`
              : ""}
          </summary>
          <div className="analytics-scope-grid">
            <FilterChecklist
              label="Decks"
              emptyLabel="No decks available"
              options={deckOptions.map((deck) => ({
                id: deck.id,
                name: deck.name,
                detail: deck.isArchived
                  ? `Archived · ${deck.hasHistory ? "history retained" : "no reviews"}`
                  : deck.hasCurrentCards
                    ? "Active inventory"
                    : deck.hasHistory
                      ? "Historical evidence"
                      : "Empty",
              }))}
              selected={selectedDeckIds}
              onChange={(next) => {
                setSelectedDeckIds(next);
                setHighlightedSessionId(undefined);
              }}
            />
            <FilterChecklist
              label="Tags"
              emptyLabel="No current or historical tags"
              options={tagOptions.map((tag) => ({
                id: tag.id,
                name: tag.name,
                detail: tag.isCurrent
                  ? tag.hasHistory
                    ? "Current · reviewed"
                    : "Current"
                  : "Historical evidence",
              }))}
              selected={selectedTagIds}
              onChange={(next) => {
                setSelectedTagIds(next);
                setHighlightedSessionId(undefined);
              }}
            />
          </div>
        </details>
        {selectedDeckNames.length > 0 || selectedTagNames.length > 0 || highlightedSessionId ? (
          <div className="analytics-filter-chips" aria-label="Active analytics filters">
            {selectedDeckNames.map((name) => (
              <span key={`deck-${name}`}>Deck: {name}</span>
            ))}
            {selectedTagNames.map((name) => (
              <span key={`tag-${name}`}>Tag: {name}</span>
            ))}
            {highlightedSessionId ? <span>Completed session highlighted</span> : null}
            <button
              type="button"
              className="text-button"
              onClick={() => {
                setSelectedDeckIds(new Set());
                setSelectedTagIds(new Set());
                setHighlightedSessionId(undefined);
              }}
            >
              Clear scope
            </button>
          </div>
        ) : null}
      </section>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {isLoading && analytics === null ? (
        <div className="inline-progress">Calculating learning analytics…</div>
      ) : null}

      {analytics ? (
        <>
          <section
            className="analytics-panel progress-chart-panel"
            aria-labelledby="progress-heading"
          >
            <div className="panel-heading-row analytics-chart-heading">
              <div>
                <h2 id="progress-heading">Learning progress</h2>
                <p className="analytics-panel-note">
                  Buckets use the local review dates captured when each answer was submitted.
                </p>
              </div>
              <div className="analytics-chart-actions">
                <div className="segmented-control" role="group" aria-label="Chart volume metric">
                  <button
                    type="button"
                    aria-pressed={chartMetric === "cards"}
                    className={chartMetric === "cards" ? "active" : ""}
                    onClick={() => setChartMetric("cards")}
                  >
                    Cards
                  </button>
                  <button
                    type="button"
                    aria-pressed={chartMetric === "time"}
                    className={chartMetric === "time" ? "active" : ""}
                    onClick={() => setChartMetric("time")}
                  >
                    Study time
                  </button>
                </div>
                <button
                  type="button"
                  className="text-button"
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
              metric={chartMetric}
              reduceMotion={settings.reduceMotion}
            />
            {showChartData ? <TimelineTable analytics={analytics} /> : null}
          </section>

          <section className="metrics-grid analytics-metrics" aria-label="Learning summary">
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

          <section className="analytics-workload-strip" aria-label="Today's workload">
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

          <section className="analytics-panel tag-insights-panel">
            <div className="panel-heading-row">
              <div>
                <h2>Topic insights</h2>
                <p className="analytics-panel-note">
                  Current counts use current tags. Review evidence uses captured tag identity and
                  displays the current tag name after renames.
                </p>
              </div>
            </div>
            {analytics.tagStats.length === 0 ? (
              <p className="muted-copy">Add tags and complete reviews to see topic-level patterns.</p>
            ) : (
              <div className="analytics-table-wrap">
                <table className="analytics-table topic-table">
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
                            {tag.generatedCardCount} generated · {tag.sourceNoteCount} source notes
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
                            className="small-primary-button"
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

          <div className="analytics-grid analytics-detail-grid">
            <section className="analytics-panel">
              <div className="panel-heading-row">
                <h2>Deck performance</h2>
              </div>
              {analytics.deckStats.length === 0 ? (
                <p className="muted-copy">No current inventory or history in this scope.</p>
              ) : (
                <div className="analytics-table-wrap">
                  <table className="analytics-table">
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

            <section className="analytics-panel">
              <div className="panel-heading-row">
                <h2>Recent sessions</h2>
              </div>
              {analytics.recentSessions.length === 0 ? (
                <p className="muted-copy">Complete a practice session to track session time.</p>
              ) : (
                <div className="analytics-session-list">
                  {analytics.recentSessions.map((session) => (
                    <article
                      key={session.id}
                      className={session.highlighted ? "highlighted" : ""}
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

          <section className="analytics-panel recent-review-panel">
            <div className="panel-heading-row">
              <h2>Recent reviews</h2>
            </div>
            {analytics.recentReviews.length === 0 ? (
              <div className="compact-empty-state">
                <p>Complete a practice session to begin tracking review results.</p>
              </div>
            ) : (
              <div className="recent-review-list">
                {analytics.recentReviews.map((review) => (
                  <article key={review.id} className="recent-review-row">
                    <span
                      className={`review-result-dot confidence-${review.rating - 1}`}
                      aria-hidden="true"
                    />
                    <div className="recent-review-copy">
                      <strong>{summarize(review.questionExcerpt)}</strong>
                      <span>
                        {review.deckName} · {outcomeLabel(review.outcome)}
                      </span>
                      {review.capturedDeckName !== review.deckName ? (
                        <small>Reviewed while named “{review.capturedDeckName}”</small>
                      ) : null}
                      {review.tags.length > 0 ? <small>{review.tags.join(" · ")}</small> : null}
                    </div>
                    <div className="recent-review-meta">
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
            <div className="inline-alert info-alert">
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

interface FilterOption {
  id: string;
  name: string;
  detail: string;
}
interface FilterChecklistProps {
  label: string;
  emptyLabel: string;
  options: FilterOption[];
  selected: Set<string>;
  onChange: (next: Set<string>) => void;
}
function FilterChecklist({
  label,
  emptyLabel,
  options,
  selected,
  onChange,
}: FilterChecklistProps) {
  return (
    <fieldset className="analytics-filter-checklist">
      <legend>{label}</legend>
      {options.length === 0 ? (
        <p>{emptyLabel}</p>
      ) : (
        <div>
          {options.map((option) => (
            <label key={option.id}>
              <input
                type="checkbox"
                checked={selected.has(option.id)}
                onChange={(event) => {
                  const next = new Set(selected);
                  if (event.currentTarget.checked) next.add(option.id);
                  else next.delete(option.id);
                  onChange(next);
                }}
              />
              <span>
                <strong>{option.name}</strong>
                <small>{option.detail}</small>
              </span>
            </label>
          ))}
        </div>
      )}
    </fieldset>
  );
}

interface MetricCardProps {
  label: string;
  value: number | string;
  detail: string;
}
function MetricCard({ label, value, detail }: MetricCardProps) {
  return (
    <article className="metric-card">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}

function TimelineTable({ analytics }: { analytics: Analytics }) {
  return (
    <div className="analytics-table-wrap chart-data-table">
      <table className="analytics-table">
        <caption>Data represented in the learning progress chart</caption>
        <thead>
          <tr>
            <th>Local period</th>
            <th>Cards</th>
            <th>Reviews</th>
            <th>New</th>
            <th>Remembered</th>
            <th>Confidence</th>
            <th>Study time</th>
          </tr>
        </thead>
        <tbody>
          {analytics.timeline.map((bucket) => (
            <tr
              key={`${bucket.startLocalDate}-${bucket.endLocalDate}`}
              className={bucket.containsHighlightedSession ? "highlighted" : ""}
            >
              <td>{compactDateFormatter.format(new Date(bucket.startAt))}</td>
              <td>{bucket.cardsPracticed}</td>
              <td>{bucket.reviewCount}</td>
              <td>{bucket.newCards}</td>
              <td>{formatPercent(bucket.correctRate)}</td>
              <td>{formatConfidence(bucket.averageConfidence)}</td>
              <td>{formatStudyTime(bucket.studyTimeMs)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Trend({ value }: { value: number | null }) {
  if (value === null) return <span className="trend neutral">—</span>;
  const points = Math.round(value * 100);
  return (
    <span
      className={points > 0 ? "trend positive" : points < 0 ? "trend negative" : "trend neutral"}
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
  return <span className={`analytics-status ${status}`}>{labels[status]}</span>;
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
