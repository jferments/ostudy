import { useCallback, useEffect, useMemo, useState } from "react";

import { useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getPracticeWorkload } from "../../ipc/practice";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { Checkbox } from "../../ui/primitives/FormControls";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Panel } from "../../ui/primitives/Panel";
import { countLabel, type SubjectTile } from "./homeModel";
import { SubjectBadge } from "./SubjectBadge";
import { MAX_SESSION_DECKS } from "./TodayStrip";

type SelectionCount =
  | { key: string; status: "loading" }
  | { key: string; status: "ready"; cards: number }
  | { key: string; status: "error"; message: string };

interface ChooseSubjectsSheetProps {
  /** Subjects with cards due today, in the order Home shows them. */
  subjects: readonly SubjectTile[];
  onClose: () => void;
  /** Starts today's scheduled review over these decks. */
  onStart: (deckIds: string[]) => void;
}

/**
 * Narrows today's review to some subjects. Every subject starts checked, so
 * starting at once studies what the Today strip counts. The count is native:
 * daily limits apply to the selection as a whole, so it is not a sum of the
 * subjects' own counts.
 */
export function ChooseSubjectsSheet({ subjects, onClose, onStart }: ChooseSubjectsSheetProps) {
  const [selected, setSelected] = useState<ReadonlySet<string>>(
    () => new Set(subjects.map((subject) => subject.key)),
  );
  const [count, setCount] = useState<SelectionCount | null>(null);
  const lane = useRequestLane();

  const deckIds = useMemo(() => {
    const ids = new Set<string>();
    for (const subject of subjects) {
      if (!selected.has(subject.key)) continue;
      for (const deckId of subject.summary.dueDeckIds) ids.add(deckId);
    }
    return [...ids].sort();
  }, [selected, subjects]);
  const key = deckIds.join(",");
  const tooManyDecks = deckIds.length > MAX_SESSION_DECKS;
  const allSelected = subjects.every((subject) => selected.has(subject.key));

  const loadCount = useCallback(() => {
    if (deckIds.length === 0 || tooManyDecks) {
      lane.invalidate();
      setCount(null);
      return;
    }
    const token = lane.begin();
    setCount({ key, status: "loading" });
    void getPracticeWorkload(deckIds).then(
      (workload) => {
        if (!lane.isCurrent(token)) return;
        setCount({ key, status: "ready", cards: workload.scheduledReviews + workload.scheduledNew });
      },
      (error: unknown) => {
        if (!lane.isCurrent(token)) return;
        setCount({ key, status: "error", message: getErrorMessage(error) });
      },
    );
  }, [deckIds, key, lane, tooManyDecks]);

  useEffect(() => loadCount(), [loadCount]);

  const current = count?.key === key ? count : null;
  const canStart =
    deckIds.length > 0 && !tooManyDecks && !(current?.status === "ready" && current.cards === 0);

  function toggle(subjectKey: string, checked: boolean): void {
    setSelected((previous) => {
      const next = new Set(previous);
      if (checked) next.add(subjectKey);
      else next.delete(subjectKey);
      return next;
    });
  }

  return (
    <Sheet
      title="Choose subjects to review"
      description="Review some subjects now. The others stay due."
      width="small"
      onClose={onClose}
      onSubmit={() => {
        if (canStart) onStart(deckIds);
      }}
      actions={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={!canStart}>
            Start review
          </Button>
        </>
      }
    >
      <div className="tw:grid tw:min-w-0 tw:gap-5">
        <Panel
          title="Subjects with cards due"
          actions={
            <Button
              size="small"
              variant="text"
              onClick={() =>
                setSelected(allSelected ? new Set() : new Set(subjects.map((subject) => subject.key)))
              }
            >
              {allSelected ? "Clear all" : "Select all"}
            </Button>
          }
        >
          <ul className="tw:m-0 tw:grid tw:list-none tw:gap-2 tw:p-0">
            {subjects.map((subject) => (
              <li key={subject.key}>
                <label className="tw:flex tw:min-h-11 tw:cursor-pointer tw:items-center tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:px-3 tw:py-2 tw:has-[[data-checked]]:border-primary tw:has-[[data-checked]]:bg-accent">
                  <Checkbox
                    checked={selected.has(subject.key)}
                    onCheckedChange={(checked) => toggle(subject.key, checked)}
                  />
                  <SubjectBadge subject={subject} size="small" />
                  <span className="tw:min-w-0 tw:flex-1 tw:font-medium tw:break-words">{subject.name}</span>
                  <span className="tw:shrink-0 tw:text-label tw:text-muted-foreground tw:tabular-nums">
                    {subject.dueCount.toLocaleString()} due
                  </span>
                </label>
              </li>
            ))}
          </ul>
        </Panel>
        <div aria-live="polite" className="tw:grid tw:gap-2">
          {deckIds.length === 0 ? (
            <p className="tw:m-0 tw:text-small tw:text-muted-foreground">Choose at least one subject.</p>
          ) : null}
          {tooManyDecks ? (
            <InlineNotice kind="warning">
              These subjects have cards due in more than {MAX_SESSION_DECKS} decks. Choose fewer
              subjects to review them in one session.
            </InlineNotice>
          ) : null}
          {current?.status === "loading" ? <InlineProgress>Counting cards…</InlineProgress> : null}
          {current?.status === "ready" ? (
            <p className="tw:m-0 tw:text-body tw:font-semibold">
              {current.cards > 0
                ? `${countLabel(current.cards, "card")} to review`
                : "Nothing left to review in these subjects today"}
            </p>
          ) : null}
          {current?.status === "error" ? (
            <InlineNotice kind="error">
              The count is unavailable: {current.message}{" "}
              <Button size="small" variant="text" onClick={loadCount}>
                Retry
              </Button>
            </InlineNotice>
          ) : null}
        </div>
      </div>
    </Sheet>
  );
}
