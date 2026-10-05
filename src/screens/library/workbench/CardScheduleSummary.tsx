import type { CardBrowserRow } from "../../../ipc/cards";
import { Button } from "../../../ui/primitives/Button";
import { dueLabel, intervalLabel } from "./workbenchModel";

interface CardScheduleSummaryProps {
  row: CardBrowserRow;
  todayStartAt: number;
  busy: boolean;
  onSuspendToggle: () => void;
  onReset: () => void;
}

/**
 * The open card's schedule beside its editor: when it is due and what the
 * scheduler knows about it, read from the card list's own row. Suspend and
 * Reset act on this one card.
 */
export function CardScheduleSummary({ row, todayStartAt, busy, onSuspendToggle, onReset }: CardScheduleSummaryProps) {
  const due = dueLabel(row, todayStartAt);
  const studied = row.reviewCount > 0;
  const facts: Array<[string, string]> = studied
    ? [
        ["Due", due.tone === "due" ? "Today" : capitalize(due.text)],
        ["Interval", intervalLabel(row)],
        ["Stability", `${formatNumber(row.stability)} d`],
        ["Difficulty", `${formatNumber(row.difficulty)} / 10`],
        ["Reviews", row.reviewCount.toLocaleString()],
        ["Lapses", row.lapses.toLocaleString()],
      ]
    : [["Due", row.suspendedAt !== null ? "Suspended" : "New, not studied yet"]];
  return (
    <section aria-label="Schedule" className="tw:grid tw:gap-2 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-3">
      <div className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-2">
        <h2 className="tw:m-0 tw:text-small tw:font-semibold">Schedule</h2>
        <span className="tw:flex tw:flex-wrap tw:gap-1.5">
          <Button size="small" variant="secondary" disabled={busy} onClick={onSuspendToggle}>
            {row.suspendedAt !== null ? "Resume" : "Suspend"}
          </Button>
          {studied ? (
            <Button size="small" variant="secondary" disabled={busy} onClick={onReset}>Reset</Button>
          ) : null}
        </span>
      </div>
      <dl className="tw:m-0 tw:grid tw:grid-cols-[repeat(auto-fill,minmax(7rem,1fr))] tw:gap-x-4 tw:gap-y-1.5">
        {facts.map(([label, value]) => (
          <div key={label} className="tw:grid tw:min-w-0">
            <dt className="tw:text-caption tw:text-muted-foreground">{label}</dt>
            <dd className="tw:m-0 tw:text-small tw:tabular-nums">{value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

function capitalize(value: string): string {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

function formatNumber(value: number): string {
  return value.toLocaleString(undefined, { maximumFractionDigits: 1 });
}
