import { confidenceAverage, type ConfidenceCounts } from "./confidence";

interface PracticeCompleteProps {
  ratedCount: number;
  confidenceCounts: ConfidenceCounts;
  isStarting: boolean;
  errorMessage: string | null;
  onViewAnalytics: () => void;
  onChooseDecks: () => void;
  onRestart: () => void;
}

export function PracticeComplete({
  ratedCount,
  confidenceCounts,
  isStarting,
  errorMessage,
  onViewAnalytics,
  onChooseDecks,
  onRestart,
}: PracticeCompleteProps) {
  const averageConfidence = confidenceAverage(confidenceCounts, ratedCount);
  return (
    <div className="screen practice-screen">
      <section className="practice-complete-card">
        <span className="completion-mark" aria-hidden="true">✓</span>
        <h1>Session complete</h1>
        <p>
          Average confidence: <strong>{averageConfidence.toFixed(1)} / 3</strong>
        </p>
        <p className="practice-completion-breakdown">
          <span>Wrong {confidenceCounts[0]}</span>
          <span>Hard {confidenceCounts[1]}</span>
          <span>Medium {confidenceCounts[2]}</span>
          <span>Easy {confidenceCounts[3]}</span>
        </p>
        <div className="completion-actions analytics-completion-actions">
          <button type="button" className="primary-button" onClick={onViewAnalytics}>
            View analytics
          </button>
          <button type="button" className="secondary-button" onClick={onChooseDecks}>
            Back to practice
          </button>
          <button type="button" className="text-button" disabled={isStarting} onClick={onRestart}>
            {isStarting ? "Starting…" : "Practice this scope again"}
          </button>
        </div>
        {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      </section>
    </div>
  );
}
