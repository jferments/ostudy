import { useId, useMemo, useState } from "react";

import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { useHomeSummary } from "../../controllers/HomeSummaryProvider";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../ui/primitives/Item";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { VisuallyHidden } from "../../ui/primitives/VisuallyHidden";
import { processingPhaseLabel } from "../documents/DocumentProgress";
import { ChooseSubjectsSheet } from "./ChooseSubjectsSheet";
import { CheckTheseSheet } from "./CheckTheseSheet";
import { HomeFirstRun } from "./HomeFirstRun";
import {
  buildSubjectTiles,
  countLabel,
  sortSubjectTiles,
  SUBJECT_SORT_OPTIONS,
  type DocumentPreparations,
  type SubjectPreparation,
  type SubjectSort,
  type SubjectTile,
} from "./homeModel";
import { NewSubjectSheet } from "./NewSubjectSheet";
import { SubjectTileCard } from "./SubjectTileCard";
import { TodayStrip } from "./TodayStrip";

/** The order chosen on Home lasts until the app closes. */
let sessionSort: SubjectSort = "most-due";

type HomeSheet = "new-subject" | "choose-subjects" | "check-these" | null;

interface HomeScreenProps {
  /** Starts today's scheduled review over these decks. */
  onStartReview: (deckIds: string[]) => void;
  /**
   * Opens a subject's page. No subject (`null`) opens in the Library view that
   * shows it best.
   */
  onOpenSubject: (subjectId: string | null, noSubjectView: "decks" | "documents") => void;
  onOpenLibrary: () => void;
}

/**
 * Home: the slim Today strip, then the learner's subjects with what is due in
 * each. A library with nothing in it shows a welcome instead.
 */
export function HomeScreen({ onStartReview, onOpenSubject, onOpenLibrary }: HomeScreenProps) {
  const {
    summary,
    library,
    summaryError,
    libraryError,
    isLoading,
    hasArchivedDecks,
    refresh,
    acceptLibrary,
  } = useHomeSummary();
  const jobs = useDocumentJobs();
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const [sort, setSortState] = useState<SubjectSort>(sessionSort);
  const [sheet, setSheet] = useState<HomeSheet>(null);
  const [announcement, setAnnouncement] = useState("");
  const todayHeadingId = useId();
  const subjectsHeadingId = useId();

  function setSort(next: SubjectSort): void {
    sessionSort = next;
    setSortState(next);
  }

  const preparations = useMemo<DocumentPreparations>(() => {
    const running = new Map<string, SubjectPreparation>();
    if (!jobs) return running;
    for (const document of jobs.documents) {
      if (document.lifecycleState === "processing") {
        running.set(document.id, {
          step: processingPhaseLabel(
            document,
            jobs.cancelingDocumentIds.has(document.id),
            jobs.generationProgress[document.id] ?? null,
          ),
          documentTitle: document.title,
        });
      } else if (jobs.guideGeneratingDocumentIds.has(document.id)) {
        running.set(document.id, { step: "Writing a study guide", documentTitle: document.title });
      }
    }
    return running;
  }, [jobs]);

  const tiles = useMemo(
    () => (summary && library ? buildSubjectTiles(summary, library, preparations) : null),
    [library, preparations, summary],
  );
  const sortedSubjects = useMemo(
    () => (tiles ? sortSubjectTiles(tiles.subjects, sort) : []),
    [sort, tiles],
  );
  const shownTiles = useMemo(
    () => (tiles?.noSubject ? [...sortedSubjects, tiles.noSubject] : sortedSubjects),
    [sortedSubjects, tiles],
  );

  function openTile(tile: SubjectTile): void {
    const { deckCount, sourceCount } = tile.summary;
    onOpenSubject(tile.subjectId, deckCount === 0 && sourceCount > 0 ? "documents" : "decks");
  }

  const readError = summaryError ?? libraryError;
  const firstRun = Boolean(tiles && tiles.subjects.length === 0 && tiles.noSubject === null);

  return (
    <PageScaffold width="workspace" data-testid="home-screen">
      <h1 className="tw:sr-only">Home</h1>
      <VisuallyHidden as="div" aria-live="polite">
        {announcement}
      </VisuallyHidden>
      {readError && (!summary || !library) ? (
        <InlineNotice kind="error">
          Home could not load: {readError}{" "}
          <Button size="small" variant="text" onClick={refresh}>
            Retry
          </Button>
        </InlineNotice>
      ) : null}
      {isLoading && !tiles ? <LoadingState label="Loading your library…" /> : null}
      {summary && library && tiles ? (
        firstRun ? (
          <HomeFirstRun
            hasArchivedDecks={hasArchivedDecks}
            importing={jobs?.isUploading ?? false}
            onCreateSubject={() => setSheet("new-subject")}
            onImportDocument={() => void jobs?.uploadDocument()}
            onOpenLibrary={onOpenLibrary}
          />
        ) : (
          <div className="tw:flex tw:flex-col tw:gap-8 tw:compact:gap-6">
            <TodayStrip
              studyDate={summary.studyDate}
              today={summary.today}
              headingId={todayHeadingId}
              onStartReview={() => onStartReview(summary.today.deckIds)}
              onChooseSubjects={() => setSheet("choose-subjects")}
            />
            {readError ? (
              <InlineNotice kind="warning">
                Home may be out of date: {readError}{" "}
                <Button size="small" variant="text" onClick={refresh}>
                  Retry
                </Button>
              </InlineNotice>
            ) : null}
            {library.checkTheseCount > 0 ? (
              <p
                data-testid="home-check-these"
                className="tw:m-0 tw:flex tw:flex-wrap tw:items-center tw:gap-x-2 tw:gap-y-1 tw:text-small tw:text-muted-foreground"
              >
                <Icon name="information" size={16} className="tw:shrink-0" />
                <span>
                  {countLabel(library.checkTheseCount, "item")} had more than one sensible place
                  when subjects arrived.
                </span>
                <Button size="small" variant="text" onClick={() => setSheet("check-these")}>
                  Check these
                </Button>
              </p>
            ) : null}
            <section
              aria-labelledby={subjectsHeadingId}
              data-tour-anchor="home.subjects"
              className="tw:flex tw:flex-col tw:gap-4.5 tw:compact:gap-3"
            >
              <SubjectsHeader
                headingId={subjectsHeadingId}
                compact={compact}
                subjectCount={tiles.subjects.length}
                sourceCount={shownTiles.reduce((sum, tile) => sum + tile.summary.sourceCount, 0)}
                cardCount={shownTiles.reduce((sum, tile) => sum + tile.summary.cardCount, 0)}
                sort={sort}
                onSortChange={setSort}
                onNewSubject={() => setSheet("new-subject")}
              />
              <ul
                className={cn(
                  "tw:m-0 tw:grid tw:list-none tw:p-0",
                  compact
                    ? "tw:grid-cols-2 tw:gap-3"
                    : "tw:grid-cols-[repeat(auto-fill,minmax(min(16.875rem,100%),1fr))] tw:gap-4",
                )}
              >
                {shownTiles.map((tile) => (
                  <li key={tile.key} className="tw:min-w-0">
                    <SubjectTileCard tile={tile} compact={compact} onOpen={() => openTile(tile)} />
                  </li>
                ))}
                {compact ? null : (
                  <li className="tw:min-w-0">
                    <NewSubjectTile onSelect={() => setSheet("new-subject")} />
                  </li>
                )}
              </ul>
            </section>
          </div>
        )
      ) : null}
      {sheet === "new-subject" ? (
        <NewSubjectSheet
          onClose={() => setSheet(null)}
          onCreated={(created, name) => {
            acceptLibrary(created);
            setSheet(null);
            setAnnouncement(`${name} created.`);
          }}
        />
      ) : null}
      {sheet === "choose-subjects" ? (
        <ChooseSubjectsSheet
          subjects={shownTiles.filter((tile) => tile.dueCount > 0)}
          onClose={() => setSheet(null)}
          onStart={(deckIds) => {
            setSheet(null);
            onStartReview(deckIds);
          }}
        />
      ) : null}
      {sheet === "check-these" ? (
        <CheckTheseSheet onClose={() => setSheet(null)} onChanged={refresh} />
      ) : null}
    </PageScaffold>
  );
}

interface SubjectsHeaderProps {
  headingId: string;
  compact: boolean;
  subjectCount: number;
  sourceCount: number;
  cardCount: number;
  sort: SubjectSort;
  onSortChange: (sort: SubjectSort) => void;
  onNewSubject: () => void;
}

function SubjectsHeader({
  headingId,
  compact,
  subjectCount,
  sourceCount,
  cardCount,
  sort,
  onSortChange,
  onNewSubject,
}: SubjectsHeaderProps) {
  const sortControl = (
    <div className={compact ? "tw:w-[11.25rem]" : "tw:w-[12.5rem]"}>
      <NativeSelect
        aria-label="Sort subjects"
        value={sort}
        onChange={(event) => onSortChange(event.currentTarget.value as SubjectSort)}
        className={cn(
          "tw:rounded-full tw:border-border tw:bg-card tw:ps-4 tw:font-medium",
          compact ? "tw:h-9 tw:text-label" : "tw:h-11 tw:text-body",
        )}
      >
        {SUBJECT_SORT_OPTIONS.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </NativeSelect>
    </div>
  );

  if (compact) {
    return (
      <div className="tw:flex tw:flex-col tw:gap-2.5">
        <div className="tw:flex tw:items-center tw:justify-between tw:gap-3">
          <h2 id={headingId} className="tw:m-0 tw:text-section tw:leading-tight tw:font-semibold tw:tracking-tight">
            Your subjects
          </h2>
          <Button className="tw:rounded-full" onClick={onNewSubject}>
            <Icon name="plus" size={16} /> New subject
          </Button>
        </div>
        {subjectCount > 1 ? sortControl : null}
      </div>
    );
  }

  return (
    <div className="tw:flex tw:flex-wrap tw:items-end tw:gap-3">
      <div className="tw:min-w-0 tw:flex-[1_1_16.25rem]">
        <h2 id={headingId} className="tw:m-0 tw:text-title tw:leading-tight tw:font-semibold tw:tracking-tight">
          Your subjects
        </h2>
        <p className="tw:mt-1 tw:mb-0 tw:text-body tw:text-muted-foreground">
          {countLabel(subjectCount, "subject")} · {countLabel(sourceCount, "source")} ·{" "}
          {countLabel(cardCount, "card")}
        </p>
      </div>
      {subjectCount > 1 ? sortControl : null}
    </div>
  );
}

function NewSubjectTile({ onSelect }: { onSelect: () => void }) {
  return (
    <ItemTrigger
      data-testid="home-new-subject"
      onClick={onSelect}
      className="tw:h-full tw:min-h-50 tw:w-full tw:flex-col tw:justify-center tw:gap-2.5 tw:rounded-lg tw:border-2 tw:border-dashed tw:border-border tw:p-5 tw:text-center tw:text-muted-foreground tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:hover:border-primary tw:hover:text-foreground tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:reduce-motion:transition-none"
    >
      <span
        aria-hidden="true"
        className="tw:grid tw:size-13 tw:place-items-center tw:rounded-full tw:bg-accent tw:text-accent-foreground"
      >
        <Icon name="plus" size={24} />
      </span>
      <span className="tw:font-heading tw:text-title-small tw:font-semibold tw:text-foreground">
        New subject
      </span>
    </ItemTrigger>
  );
}
