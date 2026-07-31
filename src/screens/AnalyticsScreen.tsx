import { useCallback, useEffect, useState } from "react";

import { getAnalytics, getErrorMessage, type Analytics } from "../ipc";

interface AnalyticsScreenProps {
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

const dateTimeFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
});

/** Displays deterministic review facts calculated by SQLite. */
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
          <p>Review performance using the same Wrong, Hard, OK, and Easy scale shown during practice.</p>
        </div>
        <button type="button" className="secondary-button" disabled={isLoading} onClick={() => void load()}>
          Refresh
        </button>
      </header>

      {errorMessage ? <div className="inline-alert error-alert" role="alert">{errorMessage}</div> : null}
      {isLoading ? <div className="inline-progress">Calculating analytics…</div> : null}

      {analytics ? (
        <>
          <section className="metrics-grid analytics-metrics-grid" aria-label="Review summary">
            <MetricCard label="Due now" value={analytics.dueNow} detail={`${analytics.cardCount} generated review cards`} />
            <MetricCard label="Reviews" value={analytics.reviewCount} detail={`${analytics.deckCount} active decks`} />
            <MetricCard label="Not wrong" value={formatPercent(analytics.correctRate)} detail={`${analytics.correctCount} rated Hard, OK, or Easy`} />
            <MetricCard label="Average time" value={formatDuration(analytics.averageResponseTimeMs)} detail="per review" />
            <MetricCard label="Struggling" value={analytics.strugglingCount} detail="notes needing attention" />
            <MetricCard label="Suspended" value={analytics.suspendedCount} detail="notes paused from practice" />
          </section>

          <div className="analytics-grid">
            <section className="analytics-panel">
              <div className="panel-heading-row">
                <div>
                  <h2>Difficulty ratings</h2>
                  <p>Undo reversals are excluded.</p>
                </div>
              </div>
              <div className="rating-distribution">
                <RatingBar label="Wrong" count={analytics.ratingDistribution.wrong} total={analytics.reviewCount} className="wrong" />
                <RatingBar label="Hard" count={analytics.ratingDistribution.hard} total={analytics.reviewCount} className="hard" />
                <RatingBar label="OK" count={analytics.ratingDistribution.ok} total={analytics.reviewCount} className="ok" />
                <RatingBar label="Easy" count={analytics.ratingDistribution.easy} total={analytics.reviewCount} className="easy" />
              </div>
            </section>

            <section className="analytics-panel">
              <div className="panel-heading-row"><h2>Deck performance</h2></div>
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
                        <th>Not wrong</th>
                        <th>Struggling</th>
                        <th>Time</th>
                      </tr>
                    </thead>
                    <tbody>
                      {analytics.deckStats.map((deck) => (
                        <tr key={deck.deckId}>
                          <td><strong>{deck.deckName}</strong><small>{deck.cardCount} generated cards</small></td>
                          <td>{deck.dueCount}</td>
                          <td>{deck.reviewCount}</td>
                          <td>{formatPercent(deck.correctRate)}</td>
                          <td>{deck.strugglingCount}</td>
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
            <div className="panel-heading-row"><h2>Recent reviews</h2></div>
            {analytics.recentReviews.length === 0 ? (
              <div className="compact-empty-state"><p>Complete a practice session to begin tracking review results.</p></div>
            ) : (
              <div className="recent-review-list">
                {analytics.recentReviews.map((review) => (
                  <article key={review.id} className="recent-review-row">
                    <span className={`review-result-dot score-${review.score}`} aria-hidden="true" />
                    <div className="recent-review-copy">
                      <strong>{summarize(review.frontText)}</strong>
                      <span>{review.deckName} · {review.label}</span>
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

function MetricCard({ label, value, detail }: { label: string; value: number | string; detail: string }) {
  return <article className="metric-card"><span>{label}</span><strong>{value}</strong><small>{detail}</small></article>;
}

function RatingBar({ label, count, total, className }: { label: string; count: number; total: number; className: string }) {
  const percent = total === 0 ? 0 : Math.round((count / total) * 100);
  return (
    <div className={`rating-bar-row ${className}`}>
      <div><strong>{label}</strong><span>{count} · {percent}%</span></div>
      <div className="rating-bar-track" aria-label={`${label}: ${count} reviews, ${percent}%`}><span style={{ width: `${percent}%` }} /></div>
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

function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 120 ? `${normalized.slice(0, 117)}…` : normalized;
}
