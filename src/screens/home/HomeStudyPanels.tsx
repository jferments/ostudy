import { useEffect, useState } from "react";

import { listSubjectLessons, type LessonSummary } from "../../ipc/lessons";
import { getProgressSubjects, type SubjectProgress } from "../../ipc/progress";
import { Button } from "../../ui/primitives/Button";
import { ListRow, ListRowMedia, ListRows } from "../../ui/primitives/ListRow";
import { Panel } from "../../ui/primitives/Panel";
import { Progress } from "../../ui/primitives/Progress";
import type { SubjectTile } from "./homeModel";

/** The most rows either panel shows; Library has the rest. */
const MAX_ROWS = 3;

/** A subject's remembered rate this week has to fall this far before Home calls it slipping. */
const SLIPPING_DROP = 0.1;
/** Fewer due reviews than this in a week say too little to call a subject slipping. */
const SLIPPING_MIN_REVIEWS = 10;

export interface HomeLessons {
  /** Every lesson by subject id; a subject whose lessons could not load is absent. */
  bySubject: ReadonlyMap<string, readonly LessonSummary[]>;
}

/** Reads each subject's lessons, one independent read per subject. */
export function useHomeLessons(subjectIds: readonly string[], refreshKey: number): HomeLessons | null {
  const [lessons, setLessons] = useState<HomeLessons | null>(null);
  const key = subjectIds.join("|");

  useEffect(() => {
    let current = true;
    const ids = key === "" ? [] : key.split("|");
    void Promise.allSettled(ids.map((subjectId) => listSubjectLessons(subjectId))).then((results) => {
      if (!current) return;
      const bySubject = new Map<string, readonly LessonSummary[]>();
      for (const result of results) {
        if (result.status === "fulfilled") bySubject.set(result.value.subjectId, result.value.lessons);
      }
      setLessons({ bySubject });
    });
    return () => {
      current = false;
    };
  }, [key, refreshKey]);

  return lessons;
}

interface ContinuePanelProps {
  lessons: HomeLessons | null;
  subjects: readonly SubjectTile[];
  onOpenSubject: (subjectId: string) => void;
}

/** Lessons started and not finished, most recent first. Hidden when there are none. */
export function ContinuePanel({ lessons, subjects, onOpenSubject }: ContinuePanelProps) {
  if (!lessons) return null;
  const names = new Map(subjects.map((tile) => [tile.subjectId, tile.name]));
  const started = [...lessons.bySubject.values()]
    .flat()
    .filter((lesson) => lesson.written && lesson.progress && lesson.progress.finishedAt === null)
    .sort((a, b) => (b.progress?.updatedAt ?? 0) - (a.progress?.updatedAt ?? 0))
    .slice(0, MAX_ROWS);
  if (started.length === 0) return null;

  return (
    <Panel title="Pick up where you left off" data-testid="home-continue">
      <ListRows>
        {started.map((lesson) => {
          const steps = lesson.written?.stepCount ?? 0;
          const step = Math.min(steps, (lesson.progress?.currentStep ?? 0) + 1);
          return (
            <ListRow
              key={lesson.id}
              leading={<ListRowMedia icon="book" />}
              title={lesson.title}
              meta={
                <>
                  <span>
                    Lesson · {names.get(lesson.subjectId) ?? "Subject"} · step {step} of {steps}
                  </span>
                  <Progress
                    size="small"
                    className="tw:mt-1.5 tw:max-w-64"
                    label={`${lesson.title} progress`}
                    value={step}
                    max={Math.max(1, steps)}
                  />
                </>
              }
              trailing={
                <Button variant="secondary" aria-label={`Resume ${lesson.title}`} onClick={() => onOpenSubject(lesson.subjectId)}>
                  Resume
                </Button>
              }
            />
          );
        })}
      </ListRows>
    </Panel>
  );
}

interface Suggestion {
  key: string;
  icon: "sparkles" | "book";
  title: string;
  meta: string;
  action: string;
  onSelect: () => void;
}

interface SuggestionsPanelProps {
  lessons: HomeLessons | null;
  subjects: readonly SubjectTile[];
  refreshKey: number;
  onStartReview: (deckIds: string[]) => void;
  onPracticeSubject: (subjectId: string) => void;
  onOpenSubject: (subjectId: string) => void;
}

/**
 * What to do next, from real state: subjects remembered less this week than the
 * week before, and subjects with sources but no lessons yet. Hidden when there is
 * nothing to suggest.
 */
export function SuggestionsPanel({
  lessons,
  subjects,
  refreshKey,
  onStartReview,
  onPracticeSubject,
  onOpenSubject,
}: SuggestionsPanelProps) {
  const [progress, setProgress] = useState<SubjectProgress[] | null>(null);

  useEffect(() => {
    let current = true;
    void getProgressSubjects({ range: "7d", subjectIds: [], deckIds: [], tagIds: [] })
      .then((rows) => {
        if (current) setProgress(rows);
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [refreshKey]);

  const suggestions: Suggestion[] = [];
  for (const row of progress ?? []) {
    if (row.subjectId === null || row.trueRetention === null || row.retentionDelta === null) continue;
    if (row.retentionDelta > -SLIPPING_DROP || row.retentionReviewCount < SLIPPING_MIN_REVIEWS) continue;
    const tile = subjects.find((candidate) => candidate.subjectId === row.subjectId);
    if (!tile) continue;
    const now = Math.round(row.trueRetention * 100);
    const before = Math.round((row.trueRetention - row.retentionDelta) * 100);
    const subjectId = row.subjectId;
    suggestions.push({
      key: `slipping-${subjectId}`,
      icon: "sparkles",
      title: `${tile.name} is slipping`,
      meta: `${now}% remembered this week, down from ${before}%`,
      action: tile.dueCount > 0 ? `Review ${tile.dueCount.toLocaleString()}` : "Practice",
      onSelect: () =>
        tile.dueCount > 0 ? onStartReview(tile.summary.dueDeckIds) : onPracticeSubject(subjectId),
    });
  }
  if (lessons) {
    for (const tile of subjects) {
      const subjectId = tile.subjectId;
      if (subjectId === null || tile.summary.sourceCount === 0) continue;
      const subjectLessons = lessons.bySubject.get(subjectId);
      if (!subjectLessons || subjectLessons.length > 0) continue;
      suggestions.push({
        key: `lesson-${subjectId}`,
        icon: "book",
        title: `${tile.name} is ready for a lesson`,
        meta: tile.summary.sourceCount === 1 ? "1 source" : `${tile.summary.sourceCount.toLocaleString()} sources`,
        action: "Create lessons",
        onSelect: () => onOpenSubject(subjectId),
      });
    }
  }
  if (suggestions.length === 0) return null;

  return (
    <Panel title="Suggested next" data-testid="home-suggestions">
      <ListRows>
        {suggestions.slice(0, MAX_ROWS).map((suggestion) => (
          <ListRow
            key={suggestion.key}
            leading={<ListRowMedia icon={suggestion.icon} />}
            title={suggestion.title}
            meta={suggestion.meta}
            trailing={
              <Button variant="text" onClick={suggestion.onSelect}>
                {suggestion.action}
              </Button>
            }
          />
        ))}
      </ListRows>
    </Panel>
  );
}
