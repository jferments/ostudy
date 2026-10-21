import { Panel } from "../../ui/primitives/Panel";
import type { AppSettings } from "../../ipc/settings";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { HourControl, NumberControl, RangeControl, SwitchRow } from "./SettingsControls";

interface ReviewSchedulingSettingsProps {
  draft: AppSettings;
  update: (patch: Partial<AppSettings>) => void;
}

export function ReviewSchedulingSettings({ draft, update }: ReviewSchedulingSettingsProps) {
  return (
    <div className="tw:grid tw:min-w-0 tw:gap-5 tw:xl:grid-cols-2">
      <Panel className="tw:min-w-0 tw:self-start" title="Remembering"
        actions={<HelpTrigger topicId="settings.retention" label="Help with desired retention" />}>
        <RangeControl
          label="Desired retention"
          min={80}
          max={97}
          step={1}
          value={Math.round(draft.desiredRetention * 100)}
          suffix="%"
          onChange={(value) => update({ desiredRetention: value / 100 })}
        />
      </Panel>

      <Panel className="tw:min-w-0" title="Your day"
        actions={<HelpTrigger topicId="practice.dailyLimits" label="Help with daily limits" />}>
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
            onChange={(newDayStartHour) => update({ newDayStartHour })}
          />
        </div>
      </Panel>

      <Panel className="tw:min-w-0 tw:self-start" title="Answer side"
        actions={<HelpTrigger topicId="settings.reveal" label="Help with the answer side" />}>
        <SwitchRow
          label="Display both question and answer when card is flipped"
          checked={draft.displayQuestionOnAnswer ?? true}
          onChange={(displayQuestionOnAnswer) => update({ displayQuestionOnAnswer })}
        />
      </Panel>
    </div>
  );
}
