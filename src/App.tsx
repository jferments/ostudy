import { lazy, Suspense, useEffect, useState } from "react";

import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { AppHeaderBar } from "./components/AppHeaderBar";
import { IncomingShareCoordinator } from "./components/IncomingShareCoordinator";
import { MainNavigation } from "./components/MainNavigation";
import { Toast } from "./components/Toast";
import { UsbSyncCoordinator } from "./components/UsbSyncCoordinator";
import { TutorProvider } from "./screens/tutor/TutorProvider";
import { DocumentJobsProvider } from "./controllers/DocumentJobsProvider";
import {
  cardsDueToday,
  HomeSummaryProvider,
  useHomeSummary,
} from "./controllers/HomeSummaryProvider";
import {
  useAppController,
  type AppSection,
  type AppWorkflow,
} from "./controllers/useAppController";
import { ALL_SUBJECTS, isLibrarySection, type AppDestination, type SubjectSection } from "./navigation/appDestinations";
import { shouldResetStudyGuidesRoute } from "./navigation/studyGuidesNavigation";
import type { StudyGuideOpenRequest } from "./controllers/useStudyGuidesController";
import type { NotebookOpenRequest } from "./screens/notes/LibraryNotes";
import { HomeScreen } from "./screens/home/HomeScreen";
import { LibraryLessons } from "./screens/library/LibraryLessons";
import { LibraryHome } from "./screens/library/LibraryHome";
import { LibraryFrame } from "./screens/library/LibraryFrame";
import { workbenchLaunchFor } from "./screens/library/workbench/workbenchLaunch";
import { PerformanceProfiler } from "./testing/PerformanceProfiler";
import { AppearanceProvider } from "./ui/appearance/AppearanceProvider";
import { LayoutModeProvider } from "./ui/layout/LayoutModeProvider";
import { RouteTransition } from "./ui/motion/Motion";
import { Button } from "./ui/primitives/Button";
import { LoadingState } from "./ui/primitives/EmptyState";
import { PageScaffold } from "./ui/patterns/WorkspacePatterns";
import { TourProvider } from "./ui/tutorials/TourProvider";
import shellStyles from "./AppShell.module.css";
import { AppFrame, AppContent } from "./ui/layout/AppFrame";
import { BrandMark } from "./components/Brand";
import { InlineNotice } from "./ui/primitives/InlineNotice";

const loadLibraryWorkbench = () => import("./screens/library/workbench/LibraryWorkbench");
const loadDocumentsSection = () => import("./screens/DocumentsSection");
const loadStudyGuidesRoute = () => import("./screens/StudyGuidesRoute");
const loadLibraryNotes = () => import("./screens/notes/LibraryNotes");
const loadPracticeScreen = () => import("./screens/PracticeScreen");
const loadProgressScreen = () => import("./screens/progress/ProgressScreen");
const loadTutorScreen = () => import("./screens/tutor/TutorWorkspace");
const loadSettingsScreen = () => import("./screens/SettingsScreen");

const LibraryWorkbench = lazy(() =>
  loadLibraryWorkbench().then((module) => ({ default: module.LibraryWorkbench })),
);
const DocumentsSection = lazy(() =>
  loadDocumentsSection().then((module) => ({ default: module.DocumentsSection })),
);
const StudyGuidesRoute = lazy(() =>
  loadStudyGuidesRoute().then((module) => ({ default: module.StudyGuidesRoute })),
);
const LibraryNotes = lazy(() =>
  loadLibraryNotes().then((module) => ({ default: module.LibraryNotes })),
);
const PracticeScreen = lazy(() =>
  loadPracticeScreen().then((module) => ({ default: module.PracticeScreen })),
);
const ProgressScreen = lazy(() =>
  loadProgressScreen().then((module) => ({ default: module.ProgressScreen })),
);
const TutorScreen = lazy(() =>
  loadTutorScreen().then((module) => ({ default: module.TutorScreen })),
);
const SettingsScreen = lazy(() =>
  loadSettingsScreen().then((module) => ({ default: module.SettingsScreen })),
);

const FEATURE_PRELOADERS = [
  loadLibraryWorkbench,
  loadDocumentsSection,
  loadStudyGuidesRoute,
  loadLibraryNotes,
  loadPracticeScreen,
  loadProgressScreen,
  loadTutorScreen,
  loadSettingsScreen,
] as const;

export type {
  AnalyticsLaunchRequest,
  AppSection,
  AppWorkflow,
  ContextualPracticeLaunchRequest,
  DeckOpenRequest,
  PracticeLaunchRequest,
  PracticeReturnTarget,
} from "./controllers/useAppController";

function App() {
  const controller = useAppController();
  const [studyGuidesNavigationKey, setStudyGuidesNavigationKey] = useState(0);
  // The lesson Library's Lessons page is playing.
  const [openLessonState, setOpenLesson] = useState<{ subjectId: string; lessonId: string } | null>(null);
  const [guideOpenRequest, setGuideOpenRequest] = useState<StudyGuideOpenRequest | null>(null);
  const [notebookOpenRequest, setNotebookOpenRequest] = useState<NotebookOpenRequest | null>(null);
  const tutorialsEnabled = controller.activeSettings.tutorialsEnabled ?? false;

  useEffect(() => {
    if (controller.isLoading || controller.fatalError) return;
    const timer = window.setTimeout(() => {
      for (const preload of FEATURE_PRELOADERS) {
        void preload().catch(() => undefined);
      }
    }, 750);
    return () => window.clearTimeout(timer);
  }, [controller.fatalError, controller.isLoading]);

  function navigate(section: AppSection): void {
    if (
      shouldResetStudyGuidesRoute(
        section,
        controller.section,
        controller.activeWorkflow,
      )
    ) {
      setStudyGuidesNavigationKey((current) => current + 1);
    }
    void controller.navigate(section);
  }

  function selectDestination(destination: AppDestination): void {
    if (destination === "library") {
      void controller.openLibrary();
      return;
    }
    navigate(DESTINATION_SECTIONS[destination] ?? "home");
  }

  function startReview(deckIds: string[]): void {
    void controller.practiceScope(deckIds, []);
  }

  function openLesson(subjectId: string, lessonId: string): void {
    setOpenLesson({ subjectId, lessonId });
    controller.chooseLibrarySubject({ kind: "subject", subjectId });
    navigate("lessons");
  }

  const librarySubject = controller.librarySubject;
  const libraryFrame = {
    subject: librarySubject,
    onChooseSubject: controller.chooseLibrarySubject,
    onOpenLibrary: () => void controller.openLibrary(),
    onSelectView: (view: SubjectSection) =>
      view === "decks" && controller.section !== "decks" && librarySubject.kind !== "all"
        ? void controller.openSubject(librarySubject.kind === "subject" ? librarySubject.subjectId : null)
        : navigate(view),
    onStartReview: startReview,
    onPracticeSubject: (subjectId: string) => void controller.practiceSubject(subjectId),
    showToast: controller.showToast,
  };
  const subjectFilter =
    librarySubject.kind === "all" ? undefined : librarySubject.kind === "none" ? null : librarySubject.subjectId;

  // Review lives on Home, so Home also stands in for the legacy Review section.
  const homeScreen = (
    <HomeScreen
      onStartReview={startReview}
      onOpenSubject={(subjectId, page) => void controller.openSubject(subjectId, page)}
      onOpenLesson={openLesson}
      onOpenLibrary={() => void controller.openLibrary()}
      onPracticeSubject={(subjectId) => void controller.practiceSubject(subjectId)}
      onOpenProgress={() => navigate("analytics")}
      onOpenDeck={(deckId) => void controller.openLibrary({ deckId })}
      refreshDecks={controller.refreshDecks}
      showToast={controller.showToast}
    />
  );

  return (
    <LayoutModeProvider>
      <AppearanceProvider settings={controller.activeSettings}>
        <TourProvider enabled={!controller.isLoading && !controller.fatalError && tutorialsEnabled}>
          {controller.isLoading ? (
            <div className="tw:grid tw:min-h-dvh tw:place-content-center tw:gap-4 tw:bg-background tw:p-8 tw:text-center tw:text-foreground">
              <div className="tw:mx-auto"><BrandMark className="tw:size-12" /></div>
              <h1>Opening ostudy</h1>
              <p>Loading your library…</p>
            </div>
          ) : controller.fatalError ? (
            <div className="tw:grid tw:min-h-dvh tw:place-content-center tw:gap-4 tw:bg-background tw:p-8 tw:text-center tw:text-foreground">
              <h1>ostudy could not open the library</h1>
              <InlineNotice kind="error">{controller.fatalError}</InlineNotice>
              <Button variant="primary" onClick={() => window.location.reload()}>
                Try again
              </Button>
            </div>
          ) : (
            <DocumentJobsProvider refreshDecks={controller.refreshDecks} showToast={controller.showToast}>
              <HomeSummaryProvider
                section={controller.section}
                activeWorkflow={controller.activeWorkflow}
                decks={controller.decks}
                finishedSessions={controller.finishedPracticeSessions}
                studyRules={controller.savedSettings}
              >
                <TutorProvider>
                  <AppFrame>
                    <IncomingShareCoordinator
                      refreshDecks={controller.refreshDecks}
                      showToast={controller.showToast}
                    />
                    <UsbSyncCoordinator
                      refreshDecks={controller.refreshDecks}
                      showToast={controller.showToast}
                    />
                    <PerformanceProfiler id="navigation">
                      <ShellHeader
                        section={controller.section}
                        activeWorkflow={controller.activeWorkflow}
                        onSelectDestination={selectDestination}
                        onSearch={() => void controller.openSearch()}
                        onSettings={() => navigate("settings")}
                      />
                    </PerformanceProfiler>

                    <AppContent>
                      <RouteTransition
                        routeKey={
                          controller.activeWorkflow ??
                          (isLibrarySection(controller.section) ? "library" : controller.section)
                        }
                        relation="sibling"
                        className={shellStyles.routeLayer}
                      >
                        <AppErrorBoundary>
                          <Suspense fallback={<LoadingState label="Opening section…" />}>
                            {controller.activeWorkflow === "practice" ? (
                              <PracticeScreen
                                decks={controller.decks}
                                settings={controller.activeSettings}
                                launchRequest={controller.practiceWorkflow?.launchRequest ?? null}
                                onLaunchConsumed={controller.consumePracticeLaunch}
                                restoreCheckpoint={
                                  controller.practiceWorkflow?.restoreCheckpoint ?? null
                                }
                                onRestoreConsumed={controller.consumePracticeRestore}
                                onCheckpointChange={controller.reportPracticeCheckpoint}
                                returnTarget={controller.practiceWorkflow?.returnTarget ?? null}
                                onViewAnalytics={controller.viewSessionAnalytics}
                                onEndEarly={controller.returnFromPractice}
                                onChooseDecks={controller.choosePracticeDecks}
                                showToast={controller.showToast}
                              />
                            ) : controller.section === "home" ? (
                              homeScreen
                            ) : controller.section === "overview" ? (
                              <LibraryHome
                                onOpenSubject={(subjectId, page) => void controller.openSubject(subjectId, page)}
                                onChooseSubject={controller.chooseLibrarySubject}
                                onOpenAllCards={() => void controller.openAllCards()}
                                onStartReview={startReview}
                                showToast={controller.showToast}
                              />
                            ) : controller.section === "lessons" ? (
                              <LibraryFrame view="lessons" {...libraryFrame}>
                                <h1 className="tw:sr-only">Lessons</h1>
                                <LibraryLessons
                                  page="lessons"
                                  subject={librarySubject}
                                  openLessonId={
                                    openLessonState !== null &&
                                    librarySubject.kind === "subject" &&
                                    openLessonState.subjectId === librarySubject.subjectId
                                      ? openLessonState.lessonId
                                      : null
                                  }
                                  onOpenLesson={openLesson}
                                  onCloseLesson={() => setOpenLesson(null)}
                                  showToast={controller.showToast}
                                />
                              </LibraryFrame>
                            ) : controller.section === "podcasts" ? (
                              <LibraryFrame view="podcasts" {...libraryFrame}>
                                <h1 className="tw:sr-only">Podcasts</h1>
                                <LibraryLessons
                                  page="podcasts"
                                  subject={librarySubject}
                                  openLessonId={null}
                                  onOpenLesson={openLesson}
                                  onCloseLesson={() => setOpenLesson(null)}
                                  showToast={controller.showToast}
                                />
                              </LibraryFrame>
                            ) : controller.section === "decks" ? (
                              <LibraryFrame view="decks" fill {...libraryFrame}>
                                <LibraryWorkbench
                                  decks={controller.decks}
                                  refreshDecks={controller.refreshDecks}
                                  launch={workbenchLaunchFor(controller.flashcardsRoute)}
                                  onPracticeDeck={controller.practiceDeck}
                                  onPracticeCards={controller.practiceCards}
                                  onPracticeSelected={(request) => controller.practiceSelected(request, [])}
                                  practiceShuffle={controller.activeSettings.practiceShuffle}
                                  onCardDirtyChange={controller.setHasUnsavedCard}
                                  onDeckScopeChange={controller.recordLibraryDeck}
                                  subject={librarySubject}
                                  onSubjectChange={controller.recordLibrarySubject}
                                  showToast={controller.showToast}
                                />
                              </LibraryFrame>
                            ) : controller.section === "study-guides" ? (
                              <LibraryFrame view="study-guides" {...libraryFrame}>
                                <StudyGuidesRoute
                                  key={studyGuidesNavigationKey}
                                  openRequest={guideOpenRequest}
                                  onOpenRequestConsumed={(requestId) =>
                                    setGuideOpenRequest((current) => (current?.requestId === requestId ? null : current))
                                  }
                                  subject={librarySubject}
                                  showToast={controller.showToast}
                                  onPracticeCards={controller.practiceCards}
                                  onSetUpAi={() => void controller.openAiSetup()}
                                  onPracticeDeck={(deckId) => controller.practiceDeck(deckId)}
                                  onOpenNotebook={(notebookId, pageId) => {
                                    setNotebookOpenRequest({ requestId: Date.now(), notebookId, pageId });
                                    navigate("notes");
                                  }}
                                />
                              </LibraryFrame>
                            ) : controller.section === "documents" ? (
                              <LibraryFrame view="documents" {...libraryFrame}>
                                <PerformanceProfiler id="documents">
                                  <DocumentsSection
                                    refreshDecks={controller.refreshDecks}
                                    onPracticeCards={controller.practiceCards}
                                    onStudySubject={controller.practiceSubject}
                                    onOpenSubject={(subjectId) =>
                                      controller.chooseLibrarySubject(subjectId === null ? { kind: "none" } : { kind: "subject", subjectId })
                                    }
                                    subjectId={subjectFilter}
                                    showToast={controller.showToast}
                                    onSetUpAi={() => void controller.openAiSetup()}
                                    onOpenNotebook={(notebookId, pageId) => {
                                      setNotebookOpenRequest({ requestId: Date.now(), notebookId, pageId });
                                      navigate("notes");
                                    }}
                                  />
                                </PerformanceProfiler>
                              </LibraryFrame>
                            ) : controller.section === "notes" ? (
                              <PageScaffold width="full" data-testid="notes">
                                <LibraryNotes
                                  subject={ALL_SUBJECTS}
                                  openRequest={notebookOpenRequest}
                                  onOpenRequestConsumed={(requestId) =>
                                    setNotebookOpenRequest((current) => (current?.requestId === requestId ? null : current))
                                  }
                                  showToast={controller.showToast}
                                  onPracticeDeck={(deckId) => controller.practiceDeck(deckId)}
                                  onOpenGuide={(documentId, documentTitle) => {
                                    setGuideOpenRequest({ requestId: Date.now(), documentId, documentTitle });
                                    navigate("study-guides");
                                  }}
                                />
                              </PageScaffold>
                            ) : controller.section === "review" ? (
                              homeScreen
                            ) : controller.section === "tutor" ? (
                              <TutorScreen />
                            ) : controller.section === "analytics" ? (
                              <ProgressScreen
                                decks={controller.decks}
                                launchRequest={controller.analyticsLaunch}
                                onLaunchConsumed={controller.consumeAnalyticsLaunch}
                                onPracticeScope={controller.practiceScope}
                                onSearchTag={controller.openAnalyticsTagSearch}
                              />
                            ) : controller.section === "settings" ? (
                              <SettingsScreen
                                settings={controller.savedSettings}
                                aiSetupRequest={controller.aiSetupRequest}
                                onAiSetupConsumed={controller.consumeAiSetup}
                                onPreviewSettings={controller.setActiveSettings}
                                onSavedSettings={controller.saveSettingsLocally}
                                onDirtyChange={controller.setHasUnsavedSettings}
                                showToast={controller.showToast}
                              />
                            ) : null}
                          </Suspense>
                        </AppErrorBoundary>
                      </RouteTransition>
                    </AppContent>

                    <Toast toast={controller.toast} onDismiss={() => controller.setToast(null)} />
                  </AppFrame>
                </TutorProvider>
              </HomeSummaryProvider>
            </DocumentJobsProvider>
          )}
        </TourProvider>
      </AppearanceProvider>
    </LayoutModeProvider>
  );
}

/** The section each destination opens; Library opens its own page through the controller. */
const DESTINATION_SECTIONS: Readonly<Record<AppDestination, AppSection | null>> = {
  home: "home",
  library: null,
  notes: "notes",
  tutor: "tutor",
  progress: "analytics",
};

interface ShellHeaderProps {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
  onSelectDestination: (destination: AppDestination) => void;
  onSearch: () => void;
  onSettings: () => void;
}

/** The header and main navigation, with today's count on Home. */
function ShellHeader({
  section,
  activeWorkflow,
  onSelectDestination,
  onSearch,
  onSettings,
}: ShellHeaderProps) {
  const { summary } = useHomeSummary();
  return (
    <AppHeaderBar
      section={section}
      activeWorkflow={activeWorkflow}
      onSearch={onSearch}
      onSettings={onSettings}
      navigation={
        <MainNavigation
          section={section}
          activeWorkflow={activeWorkflow}
          dueToday={cardsDueToday(summary)}
          onSelect={onSelectDestination}
        />
      }
    />
  );
}

export default App;
