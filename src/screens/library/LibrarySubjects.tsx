import { useMemo, useState } from "react";

import { useOptionalHomeSummary } from "../../controllers/HomeSummaryProvider";
import type { LibrarySubjectScope } from "../../navigation/appDestinations";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Sheet } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { ItemTrigger } from "../../ui/primitives/Item";
import { buildSubjectTiles, countLabel, type SubjectTile } from "../home/homeModel";
import { NewSubjectSheet } from "../home/NewSubjectSheet";
import { SubjectBadge } from "../home/SubjectBadge";
import { MAX_SESSION_DECKS } from "../home/TodayStrip";

const NO_PREPARATIONS = new Map();

export interface LibrarySubjects {
  subjects: readonly SubjectTile[];
  noSubject: SubjectTile | null;
  /** The chosen subject's tile; null for Everything, or while it loads. */
  current: SubjectTile | null;
  /** True once the subject list has loaded. */
  ready: boolean;
}

/** Library's subjects, in the learner's order, from the summary Home reads too. */
export function useLibrarySubjects(scope: LibrarySubjectScope): LibrarySubjects {
  const home = useOptionalHomeSummary();
  const summary = home?.summary ?? null;
  const library = home?.library ?? null;
  return useMemo(() => {
    if (!summary || !library) return { subjects: [], noSubject: null, current: null, ready: false };
    const { subjects, noSubject } = buildSubjectTiles(summary, library, NO_PREPARATIONS);
    const current =
      scope.kind === "subject"
        ? (subjects.find((tile) => tile.subjectId === scope.subjectId) ?? null)
        : scope.kind === "none"
          ? noSubject
          : null;
    return { subjects, noSubject, current, ready: true };
  }, [library, scope, summary]);
}

interface LibrarySubjectListProps {
  scope: LibrarySubjectScope;
  subjects: LibrarySubjects;
  onChoose: (scope: LibrarySubjectScope) => void;
  /** Phones and sheets use larger rows. */
  large?: boolean;
}

/** Everything, each subject with what is due in it, No subject, and New subject. */
export function LibrarySubjectList({ scope, subjects, onChoose, large = false }: LibrarySubjectListProps) {
  const refresh = useOptionalHomeSummary()?.refresh;
  const [creating, setCreating] = useState(false);
  return (
    <nav aria-label="Subjects" data-testid="library-subjects" className="tw:flex tw:min-w-0 tw:flex-col tw:gap-0.5">
      <SubjectEntry
        testId="library-subject-all"
        current={scope.kind === "all"}
        large={large}
        leading={
          <span
            aria-hidden="true"
            className="tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-sm tw:bg-(--ostudy-color-canvas-inset) tw:text-muted-foreground"
          >
            <Icon name="library" size={16} />
          </span>
        }
        label="Everything"
        onSelect={() => onChoose({ kind: "all" })}
      />
      {subjects.subjects.length > 0 ? (
        <p className="tw:m-0 tw:mt-3 tw:mb-1 tw:px-2.5 tw:text-label tw:font-semibold tw:text-muted-foreground">Subjects</p>
      ) : null}
      {subjects.subjects.map((tile) => (
        <SubjectEntry
          key={tile.key}
          testId="library-subject"
          subjectId={tile.subjectId ?? undefined}
          current={scope.kind === "subject" && scope.subjectId === tile.subjectId}
          large={large}
          leading={<SubjectBadge subject={tile} size="small" />}
          label={tile.name}
          count={tile.dueCount}
          onSelect={() => tile.subjectId !== null && onChoose({ kind: "subject", subjectId: tile.subjectId })}
        />
      ))}
      {subjects.noSubject ? (
        <SubjectEntry
          testId="library-subject-none"
          current={scope.kind === "none"}
          large={large}
          leading={<SubjectBadge subject={subjects.noSubject} size="small" />}
          label="No subject"
          count={subjects.noSubject.dueCount}
          onSelect={() => onChoose({ kind: "none" })}
        />
      ) : null}
      <Button
        variant="text"
        data-testid="library-new-subject"
        className="tw:mt-1 tw:justify-start tw:px-2.5"
        onClick={() => setCreating(true)}
      >
        <Icon name="plus" size={16} /> New subject
      </Button>
      {creating ? (
        <NewSubjectSheet
          onClose={() => setCreating(false)}
          onCreated={(library, name) => {
            setCreating(false);
            refresh?.();
            const created = library.subjects.find((subject) => subject.name === name);
            if (created) onChoose({ kind: "subject", subjectId: created.id });
          }}
        />
      ) : null}
    </nav>
  );
}

function SubjectEntry({
  testId,
  subjectId,
  current,
  large,
  leading,
  label,
  count = 0,
  onSelect,
}: {
  testId: string;
  subjectId?: string;
  current: boolean;
  large: boolean;
  leading: React.ReactNode;
  label: string;
  count?: number;
  onSelect: () => void;
}) {
  return (
    <ItemTrigger
      data-testid={testId}
      data-subject-id={subjectId}
      aria-current={current ? "page" : undefined}
      onClick={onSelect}
      className={cn(
        "tw:w-full tw:items-center tw:gap-2.5 tw:rounded-md tw:px-2.5 tw:text-start tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:reduce-motion:transition-none",
        large ? "tw:min-h-13 tw:py-2" : "tw:min-h-11 tw:py-1.5",
        current
          ? "tw:bg-background tw:font-semibold tw:text-foreground tw:shadow-xs"
          : "tw:text-foreground tw:hover:bg-foreground/6",
      )}
    >
      {leading}
      <span className="tw:min-w-0 tw:flex-1 tw:truncate tw:text-body">{label}</span>
      {count > 0 ? (
        <span className="tw:shrink-0 tw:text-label tw:text-muted-foreground tw:tabular-nums">
          <span className="tw:sr-only">, due </span>
          {count.toLocaleString()}
        </span>
      ) : null}
    </ItemTrigger>
  );
}

interface LibrarySubjectHeaderProps {
  scope: LibrarySubjectScope;
  subjects: LibrarySubjects;
  /** Opens the subject list where the side pane does not fit. */
  onSwitch?: () => void;
  onStartReview: (deckIds: string[]) => void;
  /** One extra session mixing cards from every deck in the subject. */
  onPracticeSubject: (subjectId: string) => void;
}

/**
 * The chosen subject's name, what it holds and its Review button; Everything
 * totals every subject.
 */
export function LibrarySubjectHeader({ scope, subjects, onSwitch, onStartReview, onPracticeSubject }: LibrarySubjectHeaderProps) {
  const all = [...subjects.subjects, ...(subjects.noSubject ? [subjects.noSubject] : [])];
  const shown = scope.kind === "all" ? all : subjects.current ? [subjects.current] : [];
  const sources = shown.reduce((sum, tile) => sum + tile.summary.sourceCount, 0);
  const cards = shown.reduce((sum, tile) => sum + tile.summary.cardCount, 0);
  const due = shown.reduce((sum, tile) => sum + tile.dueCount, 0);
  const dueDeckIds = [...new Set(shown.flatMap((tile) => tile.summary.dueDeckIds))];
  const name = scope.kind === "all" ? "Everything" : scope.kind === "none" ? "No subject" : (subjects.current?.name ?? "");
  const counts = [
    ...(scope.kind === "all" ? [countLabel(subjects.subjects.length, "subject")] : []),
    countLabel(sources, "document"),
    countLabel(cards, "card"),
  ].join(" · ");

  // Each page under the tabs names itself in its h1; the subject is the page's context.
  const title = (
    <h2
      data-testid="library-subject-name"
      className="tw:m-0 tw:truncate tw:text-title tw:leading-tight tw:font-semibold tw:tracking-tight tw:compact:text-section"
    >
      {name}
    </h2>
  );
  return (
    <header data-testid="library-subject-header" className="tw:flex tw:flex-wrap tw:items-center tw:gap-x-4 tw:gap-y-3">
      {subjects.current ? <SubjectBadge subject={subjects.current} size="large" className="tw:compact:size-11" /> : null}
      <div className="tw:min-w-0 tw:flex-[1_1_12rem]">
        {onSwitch ? (
          <ItemTrigger
            data-testid="library-switch-subject"
            aria-label={`${name}, choose a subject`}
            onClick={onSwitch}
            className="tw:max-w-full tw:min-h-11 tw:gap-1.5 tw:text-foreground tw:outline-none tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50"
          >
            {title}
            <Icon name="chevron-down" size={18} className="tw:shrink-0 tw:text-muted-foreground" />
          </ItemTrigger>
        ) : (
          title
        )}
        {subjects.ready ? <p className="tw:m-0 tw:text-small tw:text-muted-foreground">{counts}</p> : null}
      </div>
      {scope.kind === "subject" && cards > 0 ? (
        <Button data-testid="library-practice" onClick={() => onPracticeSubject(scope.subjectId)}>
          Practice a mix
        </Button>
      ) : null}
      {due > 0 ? (
        <Button
          variant="primary"
          data-testid="library-review"
          disabled={dueDeckIds.length > MAX_SESSION_DECKS}
          onClick={() => onStartReview(dueDeckIds)}
        >
          <Icon name="practice" size={15} /> Review {due.toLocaleString()} due
        </Button>
      ) : null}
    </header>
  );
}

/** The subject list in a sheet, for screens too narrow for the side pane. */
export function LibrarySubjectSheet({
  scope,
  subjects,
  onChoose,
  onClose,
}: {
  scope: LibrarySubjectScope;
  subjects: LibrarySubjects;
  onChoose: (scope: LibrarySubjectScope) => void;
  onClose: () => void;
}) {
  return (
    <Sheet title="Subjects" width="small" onClose={onClose}>
      <LibrarySubjectList
        scope={scope}
        subjects={subjects}
        large
        onChoose={(next) => {
          onClose();
          onChoose(next);
        }}
      />
    </Sheet>
  );
}

export function subjectCountsLabel(tile: SubjectTile): string {
  return [countLabel(tile.summary.sourceCount, "document"), countLabel(tile.summary.cardCount, "card")].join(" · ");
}
