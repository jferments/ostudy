import type { ReactNode } from "react";

import type { CardStrength } from "../../ipc/progress";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { Badge } from "../../ui/primitives/Badge";
import { StrengthBar } from "./StrengthPanel";

const WILL_SHOW: ReadonlyArray<{ icon: IconName; title: string; text: string }> = [
  { icon: "analytics", title: "How much you remember", text: "The share of reviews you got right, and how it moves week to week." },
  { icon: "schedule", title: "Study time and streaks", text: "Minutes studied, the days you studied and your current streak." },
  { icon: "list-checks", title: "Strong and weak topics", text: "Which tags you know well, and which to practice next." },
  { icon: "calendar-check", title: "What is coming up", text: "How many reviews fall due each day, so no day surprises you." },
];

// Reviews per day in the preview: an ordinary first three weeks.
const PREVIEW_REVIEWS = [12, 30, 26, 0, 41, 38, 22, 45, 52, 0, 0, 48, 60, 44, 58, 63, 40, 0, 55, 70, 66];
const PREVIEW_FORECAST = [64, 48, 52, 30, 41, 27, 35];
const PREVIEW_STRENGTH: CardStrength = {
  newCount: 60,
  learningCount: 70,
  gettingThereCount: 110,
  solidCount: 236,
  solidStabilityDays: 21,
  estimatedRecall: 0.91,
  estimatedRecallCardCount: 416,
};

/**
 * Progress before anything has been studied: what the page will show, a
 * drawn preview of it after a few weeks, and how spaced repetition with FSRS
 * decides when each card comes back.
 */
export function ProgressWelcome({ action }: { action?: ReactNode }) {
  return (
    <div data-testid="progress-welcome" className="tw:grid tw:min-w-0 tw:gap-8 tw:wide:grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)] tw:wide:items-start">
      <div className="tw:grid tw:min-w-0 tw:gap-6">
        <div className="tw:grid tw:gap-2">
          <h2 className="tw:m-0 tw:text-section tw:font-semibold">Your progress shows up here once you review</h2>
          <p className="tw:m-0 tw:text-muted-foreground">
            Every card you answer adds to this page. After a few days of study it shows:
          </p>
        </div>
        <ul aria-label="What Progress shows" className="tw:m-0 tw:grid tw:list-none tw:gap-3 tw:p-0 tw:medium:grid-cols-2">
          {WILL_SHOW.map((item) => (
            <li key={item.title} className="tw:flex tw:min-w-0 tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-3.5">
              <Icon name={item.icon} size={20} className="tw:mt-0.5 tw:shrink-0 tw:text-primary" />
              <span className="tw:grid tw:min-w-0 tw:gap-0.5">
                <span className="tw:text-label tw:font-semibold">{item.title}</span>
                <span className="tw:text-small tw:text-muted-foreground">{item.text}</span>
              </span>
            </li>
          ))}
        </ul>
        <section aria-labelledby="progress-welcome-spacing" className="tw:grid tw:gap-2">
          <h3 id="progress-welcome-spacing" className="tw:m-0 tw:text-body tw:font-semibold">How spaced repetition works</h3>
          <p className="tw:m-0 tw:text-muted-foreground">
            Instead of cramming, you see each card again just before you would forget it. Remember
            it and the wait before its next review grows, from a day to a week to months; miss it and
            it comes back soon. A few minutes a day keeps thousands of cards fresh.
          </p>
          <p className="tw:m-0 tw:text-muted-foreground">
            oStudy schedules every card with <b className="tw:text-foreground">FSRS</b>, the Free Spaced
            Repetition Scheduler, a modern open-source method fitted to hundreds of millions of real
            reviews. It estimates how likely you are to recall each card and brings it back when that
            falls to your goal: 90% unless you change <b className="tw:text-foreground">Desired retention</b> in
            Settings → Review.
          </p>
        </section>
        {action ? <div className="tw:flex tw:flex-wrap tw:gap-2">{action}</div> : null}
      </div>
      <ProgressPreview />
    </div>
  );
}

/** A drawing of Progress after three weeks of study, built from the page's own shapes. */
function ProgressPreview() {
  const peak = Math.max(...PREVIEW_REVIEWS);
  const forecastPeak = Math.max(...PREVIEW_FORECAST);
  return (
    <figure
      aria-label="A preview of Progress after three weeks of study"
      data-testid="progress-preview"
      className="tw:relative tw:m-0 tw:grid tw:min-w-0 tw:gap-3 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-muted/40 tw:p-4"
    >
      <Badge className="tw:absolute tw:end-3 tw:top-3">Preview</Badge>
      <div aria-hidden="true" className="tw:grid tw:grid-cols-2 tw:gap-2 tw:pe-16 tw:medium:grid-cols-4 tw:medium:pe-0 tw:medium:pt-7">
        {[
          ["Remembered", "91%"],
          ["Study time", "4h 10m"],
          ["Days studied", "17 of 21"],
          ["Solid cards", "236"],
        ].map(([label, value]) => (
          <div key={label} className="tw:grid tw:gap-0.5 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-2.5">
            <span className="tw:truncate tw:text-caption tw:text-muted-foreground">{label}</span>
            <span className="tw:text-body tw:font-semibold tw:tabular-nums">{value}</span>
          </div>
        ))}
      </div>
      <div aria-hidden="true" className="tw:grid tw:gap-2 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-3">
        <span className="tw:text-caption tw:font-semibold">Reviews per day</span>
        <div className="tw:flex tw:h-24 tw:items-end tw:gap-1">
          {PREVIEW_REVIEWS.map((count, index) => (
            <span
              key={index}
              className={count === 0 ? "tw:h-0.5 tw:flex-1 tw:rounded-sm tw:bg-border" : "tw:flex-1 tw:rounded-sm tw:bg-chart-1"}
              style={count === 0 ? undefined : { height: `${Math.round((count / peak) * 100)}%` }}
            />
          ))}
        </div>
      </div>
      <div aria-hidden="true" className="tw:grid tw:gap-3 tw:medium:grid-cols-2">
        <div className="tw:grid tw:gap-2 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-3">
          <span className="tw:text-caption tw:font-semibold">Card strength</span>
          <StrengthBar strength={PREVIEW_STRENGTH} />
          <span className="tw:flex tw:justify-between tw:text-caption tw:text-muted-foreground">
            <span>Solid</span><span>Getting there</span><span>Learning</span><span>New</span>
          </span>
        </div>
        <div className="tw:grid tw:gap-2 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-3">
          <span className="tw:text-caption tw:font-semibold">Due this week</span>
          <div className="tw:flex tw:h-10 tw:items-end tw:gap-1.5">
            {PREVIEW_FORECAST.map((count, index) => (
              <span key={index} className="tw:flex-1 tw:rounded-sm tw:bg-chart-1/40" style={{ height: `${Math.round((count / forecastPeak) * 100)}%` }} />
            ))}
          </div>
        </div>
      </div>
      <figcaption className="tw:text-caption tw:text-muted-foreground">What Progress looks like after three weeks of study.</figcaption>
    </figure>
  );
}
