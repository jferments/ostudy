import { useState } from "react";

import type { SubjectProgress } from "../../ipc/progress";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { cn } from "../../ui/internal/cn";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Panel } from "../../ui/primitives/Panel";
import { StatDelta } from "../../ui/primitives/StatTile";
import { STRENGTH_COLORS } from "./StrengthPanel";
import { formatPercent, formatStudyTime, plural, pointsDeltaFrom } from "./progressFormat";
import { subjectInitials } from "./subjectBadges";

type SubjectSort = "due" | "weakest" | "name";
const NO_SUBJECT = "No subject";

function sortSubjects(rows: SubjectProgress[], sort: SubjectSort): SubjectProgress[] {
  const name = (row: SubjectProgress) => row.name ?? NO_SUBJECT;
  return [...rows].sort((left, right) => {
    if (sort === "due" && right.dueTodayCount !== left.dueTodayCount) return right.dueTodayCount - left.dueTodayCount;
    if (sort === "weakest" && left.trueRetention !== right.trueRetention) return (left.trueRetention ?? 2) - (right.trueRetention ?? 2);
    return name(left).localeCompare(name(right));
  });
}

interface SubjectsPanelProps {
  subjects: SubjectProgress[] | null;
  colors: Map<string, string>;
  periodLabel: string;
  loading: boolean;
  onOpenSubject: (subjectId: string) => void;
}

/** One row per subject in scope; choosing one narrows the page to it. */
export function SubjectsPanel({ subjects, colors, periodLabel, loading, onOpenSubject }: SubjectsPanelProps) {
  const [sort, setSort] = useState<SubjectSort>("due");
  const rows = sortSubjects(subjects ?? [], sort);
  return (
    <Panel
      headingLevel={2}
      title="By subject"
      description={`${periodLabel}. Remembered counts due reviews. Choose a subject to see only its progress.`}
      aria-busy={loading || undefined}
      actions={<NativeSelect aria-label="Sort subjects" value={sort} onChange={(event) => setSort(event.currentTarget.value as SubjectSort)}>
        <option value="due">Most to review first</option>
        <option value="weakest">Weakest first</option>
        <option value="name">Name</option>
      </NativeSelect>}
    >
      <div className="tw:min-w-0 tw:overflow-x-auto">
        <table className="tw:w-full tw:min-w-[40rem] tw:border-collapse tw:text-left tw:text-small tw:tabular-nums tw:[&_td]:border-0 tw:[&_td]:border-t tw:[&_td]:border-solid tw:[&_td]:border-border tw:[&_td]:px-2 tw:[&_td]:py-2.5 tw:[&_th]:px-2 tw:[&_th]:pb-2 tw:[&_th]:text-caption tw:[&_th]:font-semibold tw:[&_th]:uppercase tw:[&_th]:tracking-wide tw:[&_th]:text-muted-foreground">
          <thead>
            <tr><th scope="col">Subject</th><th scope="col">Card strength</th><th scope="col">Remembered</th><th scope="col">Due today</th><th scope="col">Time</th></tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const { subjectId } = row;
              const name = row.name ?? NO_SUBJECT;
              const solid = row.cardCount === 0 ? 0 : row.solidCount / row.cardCount;
              const delta = pointsDeltaFrom(row.retentionDelta);
              const label = <>
                <span aria-hidden="true" className={cn("tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-md tw:text-caption tw:font-bold tw:text-primary-foreground",
                  (subjectId && colors.get(subjectId)) || "tw:bg-muted-foreground")}>
                  {subjectId ? subjectInitials(name) : <Icon name="cards" size={14} />}
                </span>
                <span className="tw:min-w-0">
                  <strong className="tw:block tw:break-words tw:text-foreground">{name}</strong>
                  <span className="tw:text-caption tw:text-muted-foreground">{plural(row.cardCount, "card")}</span>
                </span>
              </>;
              return (
                <tr key={subjectId ?? "none"}>
                  <th scope="row" className="tw:p-0 tw:font-normal">
                    {subjectId ? (
                      <Button variant="ghost" onClick={() => onOpenSubject(subjectId)} aria-label={`Show only ${name}`}
                        className="tw:h-auto tw:min-h-touch tw:w-full tw:justify-start tw:gap-3 tw:p-1 tw:text-left tw:font-normal">
                        {label}
                      </Button>
                    ) : <span className="tw:flex tw:min-h-touch tw:items-center tw:gap-3 tw:p-1">{label}</span>}
                  </th>
                  <td className="tw:w-[28%]">
                    <span aria-hidden="true" className="tw:flex tw:h-2 tw:overflow-hidden tw:rounded-full tw:bg-progress-track">
                      <span className={STRENGTH_COLORS.solid} style={{ width: `${solid * 100}%` }} />
                    </span>
                    <span className="tw:text-caption tw:text-muted-foreground">{Math.round(solid * 100)}% solid</span>
                  </td>
                  <td>
                    {row.trueRetention === null ? (
                      <span className="tw:grid">
                        <span className="tw:font-semibold">{row.reviewCount === 0 ? "No reviews" : "Building data"}</span>
                        {row.reviewCount > 0 ? <span className="tw:text-caption tw:text-muted-foreground">{plural(row.reviewCount, "review")} so far</span> : null}
                      </span>
                    ) : (
                      <span className="tw:inline-flex tw:flex-wrap tw:items-center tw:gap-1.5">
                        <strong>{formatPercent(row.trueRetention)}</strong>
                        {delta ? <StatDelta {...delta} /> : null}
                      </span>
                    )}
                  </td>
                  <td>{row.dueTodayCount === 0 ? <span className="tw:text-muted-foreground">None</span> : <strong>{row.dueTodayCount.toLocaleString()}</strong>}</td>
                  <td>{formatStudyTime(row.studyTimeMs)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </Panel>
  );
}
