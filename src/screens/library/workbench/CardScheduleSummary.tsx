import type { CardBrowserRow } from "../../../ipc/cards";
import { dueLabel, intervalLabel } from "./workbenchModel";

interface CardScheduleSummaryProps {
  row: CardBrowserRow;
  todayStartAt: number;
}

/**
 * The open card's schedule below its editor: when it is due and what the
 * scheduler knows about it, read from the card list's own row. Suspend and
 * Reset live in the editor's card menu.
 */
export function CardScheduleSummary({ row, todayStartAt }: CardScheduleSummaryProps) {
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
    <section aria-label="Schedule" className="tw:grid tw:gap-2 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted tw:px-4 tw:py-3">
      <h2 className="tw:m-0 tw:text-caption tw:font-semibold tw:tracking-wide tw:text-muted-foreground tw:uppercase">Schedule</h2>
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
