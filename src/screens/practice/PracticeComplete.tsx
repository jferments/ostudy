import { Artwork } from "../../ui/artwork/Artwork";
import { Icon } from "../../ui/icons/Icon";
import {
  DistributionBar,
  MetricGrid,
  MetricTile,
  type DistributionSegment,
} from "../../ui/patterns/Metrics";
import { Surface } from "../../ui/primitives/Surface";
import { confidenceAverage, type ConfidenceCounts } from "./confidence";
import styles from "./PracticeComplete.module.css";

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
  const rememberedCount = confidenceCounts[2] + confidenceCounts[3];
  const rememberedRate = ratedCount === 0 ? 0 : rememberedCount / ratedCount;
  const needsAnotherLook = confidenceCounts[0] + confidenceCounts[1];
  const distribution: DistributionSegment[] = [
    { id: "wrong", label: "Wrong", value: confidenceCounts[0], tone: "danger" },
    { id: "hard", label: "Hard", value: confidenceCounts[1], tone: "warning" },
    { id: "medium", label: "Medium", value: confidenceCounts[2], tone: "accent" },
    { id: "easy", label: "Easy", value: confidenceCounts[3], tone: "success" },
  ];

  return (
    <div className={`screen practice-screen ${styles.screen}`} data-testid="practice-complete">
      <Surface level={3} material="solid" className={styles.hero}>
        <div className={styles.artworkWrap}>
          <Artwork slot="practice.complete" />
        </div>
        <div className={styles.heroCopy}>
          <span className={styles.completionMark} aria-hidden="true">
            <Icon name="check" size={24} />
          </span>
          <span className={styles.eyebrow}>Study session saved</span>
          <h1>Practice complete</h1>
          <p>
            You reviewed <strong>{ratedCount}</strong>{" "}
            {ratedCount === 1 ? "card" : "cards"}. Every submitted answer and schedule change is
            already stored in your local library.
          </p>
        </div>
      </Surface>

      <section className={styles.analytics} aria-labelledby="completion-analytics-heading">
        <div className={styles.sectionHeading}>
          <div>
            <span className={styles.eyebrow}>Session analytics</span>
            <h2 id="completion-analytics-heading">How this session went</h2>
          </div>
        </div>

        <MetricGrid>
          <MetricTile
            label="Cards studied"
            value={ratedCount}
            detail="Submitted answers"
            tone="accent"
          />
          <MetricTile
            label="Average confidence"
            value={`${averageConfidence.toFixed(1)} / 3`}
            detail="0 Wrong · 3 Easy"
          />
          <MetricTile
            label="Remembered"
            value={`${Math.round(rememberedRate * 100)}%`}
            detail={`${rememberedCount} medium or easy answers`}
            tone={
              rememberedRate >= 0.8
                ? "success"
                : rememberedRate >= 0.5
                  ? "warning"
                  : "danger"
            }
          />
          <MetricTile
            label="Needs another look"
            value={needsAnotherLook}
            detail="Wrong or hard answers"
            tone={needsAnotherLook > 0 ? "warning" : "success"}
          />
        </MetricGrid>

        <Surface level={2} material="soft" className={styles.distributionCard}>
          <div>
            <h3>Confidence distribution</h3>
            <p>The shape of this session is shown with labels and counts, not color alone.</p>
          </div>
          <DistributionBar
            label="Confidence results for this session"
            segments={distribution}
          />
        </Surface>
      </section>

      <div className={styles.actions}>
        <button type="button" className="primary-button" onClick={onViewAnalytics}>
          View analytics
        </button>
        <button type="button" className="secondary-button" onClick={onChooseDecks}>
          Back to practice
        </button>
        <button
          type="button"
          className="text-button"
          disabled={isStarting}
          onClick={onRestart}
        >
          {isStarting ? "Starting…" : "Practice this scope again"}
        </button>
      </div>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
    </div>
  );
}