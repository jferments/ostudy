import { useId, useMemo, useState } from "react";

import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { useHomeSummary } from "../../controllers/HomeSummaryProvider";
import { Icon } from "../../ui/icons/Icon";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { Panel } from "../../ui/primitives/Panel";
import { VisuallyHidden } from "../../ui/primitives/VisuallyHidden";
import { processingPhaseLabel } from "../documents/DocumentProgress";
import { ChooseSubjectsSheet } from "./ChooseSubjectsSheet";
import { CheckTheseSheet } from "./CheckTheseSheet";
import { HomeFirstRun } from "./HomeFirstRun";
import { ContinuePanel, SuggestionsPanel, useHomeLessons } from "./HomeStudyPanels";
import { HomeWeekPanel } from "./HomeWeekPanel";
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
  /** Opens a subject's page in Library; `null` opens No subject's. */
  onOpenSubject: (subjectId: string | null) => void;
  /** Plays a lesson on its subject's Lessons page. */
  onOpenLesson: (subjectId: string, lessonId: string) => void;
  onOpenLibrary: () => void;
  /** Practices a subject's cards beyond today's schedule. */
  onPracticeSubject: (subjectId: string) => void;
  onOpenProgress: () => void;
}

/**
 * Home: the Today strip with Start review, then lessons to pick up, suggestions,
 * the week at a glance and the learner's subjects with what is due in each. A
 * library with nothing in it shows a welcome instead.
 */
export function HomeScreen({
  onStartReview,
  onOpenSubject,
  onOpenLesson,
  onOpenLibrary,
  onPracticeSubject,
  onOpenProgress,
}: HomeScreenProps) {
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
    onOpenSubject(tile.subjectId);
  }

  const subjectIds = useMemo(
    () => (tiles ? tiles.subjects.flatMap((tile) => (tile.subjectId === null ? [] : [tile.subjectId])) : []),
    [tiles],
  );
  // Home rereads lessons and the week whenever today's summary changes.
  const refreshKey = summary ? summary.today.reviewsRemaining + summary.today.newRemaining * 100_000 : 0;
  const lessons = useHomeLessons(subjectIds, refreshKey);
  const openSubjectPage = (subjectId: string) => onOpenSubject(subjectId);

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
            <div className="tw:flex tw:flex-wrap tw:items-start tw:gap-5 tw:compact:gap-4">
              <div className="tw:grid tw:min-w-0 tw:flex-[2_1_28rem] tw:gap-5 tw:empty:hidden tw:compact:gap-4">
                <ContinuePanel lessons={lessons} subjects={tiles.subjects} onOpenLesson={onOpenLesson} />
                <SuggestionsPanel
                  lessons={lessons}
                  subjects={tiles.subjects}
                  refreshKey={refreshKey}
                  onStartReview={onStartReview}
                  onPracticeSubject={onPracticeSubject}
                  onOpenSubject={openSubjectPage}
                />
              </div>
              <div className="tw:grid tw:min-w-0 tw:flex-[1_1_20rem] tw:gap-5 tw:compact:gap-4">
                <HomeWeekPanel refreshKey={refreshKey} onOpenProgress={onOpenProgress} />
                <Panel
                  title="Your subjects"
                  headingId={subjectsHeadingId}
                  data-tour-anchor="home.subjects"
                  description={`${countLabel(tiles.subjects.length, "subject")} · ${countLabel(
                    shownTiles.reduce((sum, tile) => sum + tile.summary.sourceCount, 0),
                    "source",
                  )} · ${countLabel(
                    shownTiles.reduce((sum, tile) => sum + tile.summary.cardCount, 0),
                    "card",
                  )}`}
                  actions={
                    <Button
                      variant="secondary"
                      data-testid="home-new-subject"
                      onClick={() => setSheet("new-subject")}
                    >
                      <Icon name="plus" size={16} /> New subject
                    </Button>
                  }
                >
                  {tiles.subjects.length > 1 ? (
                    <NativeSelect
                      aria-label="Sort subjects"
                      value={sort}
                      onChange={(event) => setSort(event.currentTarget.value as SubjectSort)}
                    >
                      {SUBJECT_SORT_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </NativeSelect>
                  ) : null}
                  <ul className="tw:m-0 tw:grid tw:list-none tw:divide-y tw:divide-border tw:p-0">
                    {shownTiles.map((tile) => (
                      <li key={tile.key} className="tw:min-w-0">
                        <SubjectTileCard tile={tile} compact={compact} onOpen={() => openTile(tile)} />
                      </li>
                    ))}
                  </ul>
                </Panel>
              </div>
            </div>
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
