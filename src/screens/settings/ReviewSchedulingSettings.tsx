import type { AppSettings } from "../../ipc/settings";
import { Button } from "../../ui/primitives/Button";
import { NumberControl, RangeControl } from "./SettingsControls";
import styles from "./Settings.module.css";

interface ReviewSchedulingSettingsProps {
  draft: AppSettings;
  update: (patch: Partial<AppSettings>) => void;
  onRestoreDefaults: () => void;
  restoreDisabled?: boolean;
}

export function ReviewSchedulingSettings({
  draft,
  update,
  onRestoreDefaults,
  restoreDisabled = false,
}: ReviewSchedulingSettingsProps) {
  return (
    <div className={styles.panelGrid}>
      <section className={styles.settingsCard}>
        <h2>FSRS scheduling</h2>
        <RangeControl
          label="Desired retention"
          min={80}
          max={97}
          step={1}
          value={Math.round(draft.desiredRetention * 100)}
          suffix="%"
          onChange={(value) => update({ desiredRetention: value / 100 })}
        />
        <p className={styles.note}>
          Higher retention creates shorter intervals and more reviews. ostudy keeps the review ledger and current schedule separate.
        </p>
      </section>

      <section className={styles.settingsCard}>
        <h2>Daily workload</h2>
        <div className={styles.dailyLimitGrid}>
          <NumberControl
            label="New cards per day"
            value={draft.dailyNewLimit}
            onChange={(dailyNewLimit) => update({ dailyNewLimit })}
          />
          <NumberControl
            label="Reviews per day"
            value={draft.dailyReviewLimit}
            onChange={(dailyReviewLimit) => update({ dailyReviewLimit })}
          />
        </div>
        <p className={styles.note}>
          A value of 0 pauses that category in normal scheduled sessions. Explicit extra and filtered sessions remain available.
        </p>
        <p className={styles.note}>
          Session-specific options, including shuffling, are chosen on the Practice screen.
        </p>
        <div className={styles.actionRow}>
          <Button
            type="button"
            variant="secondary"
            disabled={restoreDisabled}
            onClick={onRestoreDefaults}
          >
            Restore scheduling defaults
          </Button>
        </div>
      </section>

      <section className={styles.settingsCard}>
        <h2>Card reveal</h2>
        <label className={styles.switchRow}>
          <span>
            <strong>Display both question and answer when card is flipped</strong>
            <small>
              Basic cards show only the answer by default. Fill in the blank cards always keep
              their question context visible after reveal so multiple blanks remain understandable.
            </small>
          </span>
          <input
            type="checkbox"
            checked={draft.displayQuestionOnAnswer ?? false}
            onChange={(event) =>
              update({ displayQuestionOnAnswer: event.currentTarget.checked })
            }
          />
        </label>
      </section>
    </div>
  );
}
