import { Checkbox } from "../../ui/primitives/FormControls";
import { Panel } from "../../ui/primitives/Panel";
import type { AppSettings } from "../../ipc/settings";
import { Button } from "../../ui/primitives/Button";
import { HourControl, NumberControl, RangeControl } from "./SettingsControls";

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
    <div className="tw:grid tw:min-w-0 tw:gap-6 tw:xl:grid-cols-2">
      <Panel className="tw:min-w-0">
        <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">FSRS scheduling</h2>
        <RangeControl
          label="Desired retention"
          min={80}
          max={97}
          step={1}
          value={Math.round(draft.desiredRetention * 100)}
          suffix="%"
          onChange={(value) => update({ desiredRetention: value / 100 })}
        />
        <p className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">
          Higher retention creates shorter intervals and more reviews. ostudy keeps the review ledger and current schedule separate.
        </p>
      </Panel>

      <Panel className="tw:min-w-0">
        <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Daily workload</h2>
        <div className="tw:grid tw:gap-5 tw:sm:grid-cols-2">
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
          <HourControl
            label="New day starts at"
            value={draft.newDayStartHour}
            description="Reviews before this time count toward the previous day."
            onChange={(newDayStartHour) => update({ newDayStartHour })}
          />
        </div>
        <p className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">
          A value of 0 pauses that category in normal scheduled sessions. Explicit extra and filtered sessions remain available.
        </p>
        <p className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">
          Session-specific options, including shuffling, are chosen on the Practice screen.
        </p>
        <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-3">
          <Button
            type="button"
            variant="secondary"
            disabled={restoreDisabled}
            onClick={onRestoreDefaults}
          >
            Restore scheduling defaults
          </Button>
        </div>
      </Panel>

      <Panel className="tw:min-w-0">
        <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Card reveal</h2>
        <label className="tw:flex tw:min-h-11 tw:items-start tw:justify-between tw:gap-6 tw:text-label tw:[&>span:first-child]:grid tw:[&>span:first-child]:gap-1 tw:[&_small]:text-muted-foreground tw:[&_small]:font-normal tw:[&_small]:leading-relaxed">
          <span>
            <strong>Display both question and answer when card is flipped</strong>
            <small>
              Basic cards show only the answer by default. Fill in the blank cards always keep
              their question context visible after reveal so multiple blanks remain understandable.
            </small>
          </span>
          <Checkbox
            checked={draft.displayQuestionOnAnswer ?? false}
            onCheckedChange={(checked) =>
              update({ displayQuestionOnAnswer: checked })
            }
          />
        </label>
      </Panel>
    </div>
  );
}
