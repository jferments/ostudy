import { Artwork } from "../../ui/artwork/Artwork";
import { Icon } from "../../ui/icons/Icon";
import { DistributionBar, type DistributionSegment } from "../../ui/patterns/Metrics";
import { SectionHeading } from "../../ui/primitives/SectionHeading";
import { StatGrid, StatTile } from "../../ui/primitives/StatTile";
import { Surface } from "../../ui/primitives/Surface";
import { confidenceAverage, type ConfidenceCounts } from "./confidence";
import { completionStyles as styles } from "./presentation";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";

interface PracticeCompleteProps {
  ratedCount: number; confidenceCounts: ConfidenceCounts; isStarting: boolean; canRestart: boolean; errorMessage: string | null;
  onViewAnalytics: () => void; onChooseDecks: () => void; onRestart: () => void;
}
export function PracticeComplete({ ratedCount, confidenceCounts, isStarting, canRestart, errorMessage, onViewAnalytics, onChooseDecks, onRestart }: PracticeCompleteProps) {
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
  return <PageScaffold className={styles.screen} data-testid="practice-complete">
    <Surface level={3} material="solid" className={styles.hero}>
      <div className={styles.artworkWrap}><Artwork slot="practice.complete" /></div>
      <div className={styles.heroCopy}>
        <span className={styles.completionMark} aria-hidden="true"><Icon name="check" size={24} /></span>
        <span className={styles.eyebrow}>Study session saved</span><h1>Practice complete</h1>
        <p>You reviewed <strong>{ratedCount}</strong> {ratedCount === 1 ? "card" : "cards"}. Every submitted answer and schedule change is already stored in your local library.</p>
      </div>
    </Surface>
    <section className={styles.analytics} aria-labelledby="completion-analytics-heading">
      <SectionHeading id="completion-analytics-heading" eyebrow="Session analytics" title="How this session went" />
      <StatGrid>
        <StatTile label="Cards studied" value={ratedCount} hint="Submitted answers" tone="accent" />
        <StatTile label="Average confidence" value={`${averageConfidence.toFixed(1)} / 3`} hint="0 Wrong · 3 Easy" />
        <StatTile label="Remembered" value={`${Math.round(rememberedRate * 100)}%`} hint={`${rememberedCount} medium or easy answers`}
          tone={rememberedRate >= 0.8 ? "success" : rememberedRate >= 0.5 ? "warning" : "danger"} />
        <StatTile label="Needs another look" value={needsAnotherLook} hint="Wrong or hard answers" tone={needsAnotherLook > 0 ? "warning" : "success"} />
      </StatGrid>
      <Surface level={2} material="soft" className={styles.distributionCard}>
        <SectionHeading level={3} title="Confidence distribution"
          description="The shape of this session is shown with labels and counts, not color alone." />
        <DistributionBar label="Confidence results for this session" segments={distribution} />
      </Surface>
    </section>
    <div className={styles.actions}>
      <Button variant="primary" disabled={isStarting} onClick={onViewAnalytics}>View analytics</Button>
      <Button variant="secondary" disabled={isStarting} onClick={onChooseDecks}>Choose decks</Button>
      <Button variant="text" disabled={isStarting || !canRestart} onClick={onRestart}>{isStarting ? "Starting…" : "Practice this scope again"}</Button>
    </div>
    {!canRestart ? <InlineNotice kind="info">This restored session does not retain a repeatable launch request. Choose decks to start a new session.</InlineNotice> : null}
    {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
  </PageScaffold>;
}
