import { useCallback, useEffect, useState } from "react";

import {
  getAnalytics,
  getErrorMessage,
  type Analytics,
  type ReviewOutcome,
} from "../ipc";

interface AnalyticsScreenProps {
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

const dateTimeFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

/** Displays review facts calculated by SQLite rather than inferred by AI. */
export function AnalyticsScreen({ showToast }: AnalyticsScreenProps) {
  const [analytics, setAnalytics] = useState<Analytics | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const load = useCallback(async (): Promise<void> => {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      setAnalytics(await getAnalytics());
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setIsLoading(false);
    }
  }, [showToast]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="screen analytics-screen">
      <header className="screen-header compact-screen-header">
        <div>
          <h1>Analytics</h1>
          <p>Review performance and current workload.</p>
        </div>
        <button type="button" className="secondary-button" disabled={isLoading} onClick={() => void load()}>
          Refresh
        </button>
      </header>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {isLoading ? <div className="inline-progress">Calculating analytics…</div> : null}

      {analytics ? (
        <>
          <section className="metrics-grid" aria-label="Review summary">
            <MetricCard label="Due now" value={analytics.dueNow} detail={`${analytics.cardCount} total cards`} />
            <MetricCard label="Reviews" value={analytics.reviewCount} detail={`${analytics.deckCount} decks`} />
            <MetricCard label="Correct" value={formatPercent(analytics.correctRate)} detail={`${analytics.correctCount} remembered`} />
            <MetricCard label="Average time" value={formatDuration(analytics.averageResponseTimeMs)} detail="per review" />
          </section>

          <div className="analytics-grid">
            <section className="analytics-panel">
              <div className="panel-heading-row">
                <h2>Rating distribution</h2>
              </div>
              <div className="rating-distribution">
                <RatingBar label="Again" count={analytics.ratingDistribution.again} total={analytics.reviewCount} className="again" />
                <RatingBar label="Hard" count={analytics.ratingDistribution.hard} total={analytics.reviewCount} className="hard" />
                <RatingBar label="Good" count={analytics.ratingDistribution.good} total={analytics.reviewCount} className="good" />
                <RatingBar label="Easy" count={analytics.ratingDistribution.easy} total={analytics.reviewCount} className="easy" />
              </div>
            </section>

            <section className="analytics-panel">
              <div className="panel-heading-row">
                <h2>Deck performance</h2>
              </div>
              {analytics.deckStats.length === 0 ? (
                <p className="muted-copy">No decks yet.</p>
              ) : (
                <div className="analytics-table-wrap">
                  <table className="analytics-table">
                    <thead>
                      <tr>
                        <th>Deck</th>
                        <th>Due</th>
                        <th>Reviews</th>
                        <th>Correct</th>
                        <th>Time</th>
                      </tr>
                    </thead>
                    <tbody>
                      {analytics.deckStats.map((deck) => (
                        <tr key={deck.deckId}>
                          <td>
                            <strong>{deck.deckName}</strong>
                            <small>{deck.cardCount} cards</small>
                          </td>
                          <td>{deck.dueCount}</td>
                          <td>{deck.reviewCount}</td>
                          <td>{formatPercent(deck.correctRate)}</td>
                          <td>{formatDuration(deck.averageResponseTimeMs)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
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
                    <span className={`review-result-dot rating-${review.rating}`} aria-hidden="true" />
                    <div className="recent-review-copy">
                      <strong>{summarize(review.frontText)}</strong>
                      <span>{review.deckName} · {outcomeLabel(review.outcome)}</span>
                    </div>
                    <div className="recent-review-meta">
                      <strong>{formatDuration(review.responseTimeMs)}</strong>
                      <span>{dateTimeFormatter.format(new Date(review.reviewedAt))}</span>
                      <small>Next: {formatDueDate(review.dueAfter)}</small>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>
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
    <article className="metric-card">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </article>
  );
}

interface RatingBarProps {
  label: string;
  count: number;
  total: number;
  className: string;
}

function RatingBar({ label, count, total, className }: RatingBarProps) {
  const percent = total === 0 ? 0 : Math.round((count / total) * 100);
  return (
    <div className={`rating-bar-row ${className}`}>
      <div>
        <strong>{label}</strong>
        <span>{count}</span>
      </div>
      <div className="rating-bar-track" aria-label={`${label}: ${count}`}>
        <span style={{ width: `${percent}%` }} />
      </div>
    </div>
  );
}

function formatPercent(value: number | null): string {
  return value === null ? "—" : `${Math.round(value * 100)}%`;
}

function formatDuration(value: number | null): string {
  if (value === null) return "—";
  if (value < 1_000) return `${Math.round(value)} ms`;
  return `${(value / 1_000).toFixed(value < 10_000 ? 1 : 0)} s`;
}

function formatDueDate(value: number): string {
  const now = Date.now();
  if (value <= now) return "now";
  const days = Math.ceil((value - now) / 86_400_000);
  if (days <= 1) return "tomorrow";
  if (days < 14) return `${days} days`;
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value));
}

function outcomeLabel(value: ReviewOutcome): string {
  switch (value) {
    case "wrong":
      return "Wrong";
    case "not_confident":
      return "Right · not confident";
    case "somewhat_confident":
      return "Right · somewhat confident";
    case "very_confident":
      return "Right · very confident";
  }
}

function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 120 ? `${normalized.slice(0, 117)}…` : normalized;
}
