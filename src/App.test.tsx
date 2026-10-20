import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  AnalyticsLaunchRequest,
  PracticeLaunchRequest,
  PracticeReturnTarget,
} from "./App";
import App from "./App";
import * as decksIpc from "./ipc/decks";
import * as homeIpc from "./ipc/home";
import type { SelectedPracticeRequest } from "./ipc/practice";
import * as settingsIpc from "./ipc/settings";
import * as subjectsIpc from "./ipc/subjects";
import * as checkpointIpc from "./ipc/uiCheckpoint";
import { dispatchPlatformBack } from "./platform/backNavigation";
import type { WorkbenchLaunch } from "./screens/library/workbench/LibraryWorkbench";
import {
  makeDeck,
  makeHomeSummary,
  makeSettings,
  makeSubjectLibrary,
  resetTestEntityIds,
} from "./test/factories";
import { OverlayProvider } from "./ui/overlays/OverlayProvider";

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => ({
    draftId: "d".repeat(32),
    assetDraftId: "7".repeat(32),
    revision: 1,
    status: "saved",
    errorMessage: null,
    recovered: false,
    hasUnsavedLocalChanges: false,
    flush: vi.fn().mockResolvedValue(undefined),
    discard: vi.fn().mockResolvedValue(undefined),
    commit: vi.fn().mockResolvedValue(undefined),
    restart: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("./ipc/common", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected startup error",
}));
vi.mock("./ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("./ipc/settings", () => ({ getSettings: vi.fn() }));
vi.mock("./ipc/home", () => ({ getHomeSummary: vi.fn() }));
vi.mock("./ipc/subjects", () => ({ getSubjectLibrary: vi.fn() }));
vi.mock("./ipc/platform", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./ipc/platform")>();
  return {
    ...actual,
    listenToLifecycle: vi.fn().mockResolvedValue(() => undefined),
  };
});
vi.mock("./ipc/uiCheckpoint", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./ipc/uiCheckpoint")>();
  return {
    ...actual,
    getUiCheckpoint: vi.fn().mockResolvedValue(null),
    saveUiCheckpoint: vi.fn().mockImplementation(
      async (input: import("./ipc/uiCheckpoint").SaveUiCheckpointInput) => ({
        schemaVersion: 2 as const,
        updatedAt: 0,
        section: input.section,
        flashcardsWorkspace: input.flashcardsWorkspace ?? null,
        deckId: input.deckId ?? null,
        editorDraftId: input.editorDraftId ?? null,
        practice: input.practice ?? null,
        analytics: input.analytics ?? null,
      }),
    ),
  };
});

vi.mock("./screens/library/workbench/LibraryWorkbench", () => ({
  LibraryWorkbench: ({
    decks,
    launch,
    onPracticeDeck,
    onPracticeSelected,
    onPracticeCards,
    onCardDirtyChange,
    onDeckScopeChange,
    onOpenDocuments,
    onOpenStudyGuides,
  }: {
    decks: decksIpc.DeckSummary[];
    launch?: WorkbenchLaunch;
    onPracticeDeck: (deckId: string) => void;
    onPracticeSelected: (request: SelectedPracticeRequest) => void;
    onPracticeCards: (cardIds: string[]) => void;
    onCardDirtyChange: (dirty: boolean) => void;
    onDeckScopeChange?: (deckId: string | null) => void;
    onOpenDocuments: () => void;
    onOpenStudyGuides: () => void;
  }) => {
    const scope = launch?.scope;
    const place = !scope
      ? "none"
      : scope.kind === "view"
        ? `view:${scope.view}`
        : scope.kind === "subject"
          ? `subject:${scope.subjectId ?? "none"}`
          : scope.kind === "deck"
            ? `deck:${scope.deckId}`
            : `tag:${scope.tagId}`;
    return (
      <section>
        <h1>Decks mock</h1>
        <nav aria-label="Library pages">
          <button type="button" onClick={onOpenDocuments}>Documents</button>
          <button type="button" onClick={onOpenStudyGuides}>Study guides</button>
        </nav>
        <span>{decks.map((deck) => deck.name).join(",")}</span>
        <output aria-label="Workbench launch">{`${place}${launch?.focusSearch ? "|search" : ""}`}</output>
        <output aria-label="Workbench request">{launch?.requestId ?? "none"}</output>
        <button type="button" onClick={() => onCardDirtyChange(true)}>
          Mark card dirty
        </button>
        <button type="button" onClick={() => onDeckScopeChange?.(decks[0]?.id ?? null)}>
          Open deck mock
        </button>
        <button type="button" onClick={() => onDeckScopeChange?.(null)}>
          Leave deck mock
        </button>
        <button type="button" onClick={() => onPracticeDeck(decks[0]?.id ?? "")}>
          Practice deck mock
        </button>
        <button
          type="button"
          onClick={() => {
            const deck = decks[0];
            if (!deck) return;
            onPracticeSelected({
              deckIds: [deck.id],
              tagIds: ["selected-tag"],
              shuffle: false,
              mode: "extra",
            });
          }}
        >
          Practice selected decks mock
        </button>
        <button type="button" onClick={() => onPracticeCards(["search-card"])}>
          Practice search result mock
        </button>
        <button type="button" onClick={() => onPracticeCards(["editor-card"])}>
          Practice editor card mock
        </button>
      </section>
    );
  },
}));

vi.mock("./screens/PracticeScreen", () => ({
  PracticeScreen: ({
    launchRequest,
    returnTarget,
    onLaunchConsumed,
    onViewAnalytics,
    onEndEarly,
  }: {
    launchRequest: PracticeLaunchRequest | null;
    returnTarget: PracticeReturnTarget | null;
    onLaunchConsumed: () => void;
    onViewAnalytics: (sessionId: string, deckIds: string[]) => void;
    onEndEarly: (target: PracticeReturnTarget) => void;
  }) => (
    <section>
      <h1>Practice mock</h1>
      <output aria-label="Practice launch">
        {launchRequest
          ? launchRequest.kind === "selected"
            ? `${launchRequest.deckIds.join(",")}|${launchRequest.tagIds.join(",")}|${launchRequest.mode}|${returnTarget?.section ?? "none"}|${returnTarget?.deckId ?? ""}|${
                returnTarget?.flashcardsWorkspace ?? ""
              }`
            : `|${launchRequest.cardIds.join(",")}|custom|${returnTarget?.section ?? "none"}|${returnTarget?.deckId ?? ""}|${
                returnTarget?.flashcardsWorkspace ?? ""
              }`
          : "none"}
      </output>
      <button type="button" onClick={onLaunchConsumed}>
        Consume Practice launch mock
      </button>
      <button
        type="button"
        onClick={() => onViewAnalytics("session-1", ["deck-from-practice"])}
      >
        Open session analytics
      </button>
      <button
        type="button"
        onClick={() => {
          if (returnTarget) onEndEarly(returnTarget);
        }}
      >
        End early mock
      </button>
    </section>
  ),
}));

vi.mock("./screens/progress/ProgressScreen", () => ({
  ProgressScreen: ({
    launchRequest,
    onPracticeScope,
    onSearchTag,
  }: {
    launchRequest: AnalyticsLaunchRequest | null;
    onPracticeScope: (deckIds: string[], tagIds: string[]) => void;
    onSearchTag: (tagId: string) => void;
  }) => (
    <section>
      <h1>Analytics mock</h1>
      <output aria-label="Analytics launch">
        {launchRequest
          ? `${launchRequest.range}|${launchRequest.deckIds.join(",")}|${
              launchRequest.highlightedSessionId ?? ""
            }`
          : "none"}
      </output>
      <button type="button" onClick={() => onPracticeScope([], ["analytics-tag"])}>
        Practice analytics scope mock
      </button>
      <button type="button" onClick={() => onSearchTag("stable-analytics-tag-id")}>
        Search current analytics tag mock
      </button>
    </section>
  ),
}));

vi.mock("./screens/DocumentsSection", () => ({
  DocumentsSection: ({ onPracticeCards }: { onPracticeCards: (cardIds: string[]) => Promise<void> }) => (
    <section>
      <h1>Documents mock</h1>
      <button type="button" onClick={() => void onPracticeCards(["document-card"])}>
        Practice document card mock
      </button>
    </section>
  ),
}));
vi.mock("./screens/StudyGuidesRoute", () => ({
  StudyGuidesRoute: ({
    onPracticeCards,
  }: {
    onPracticeCards: (cardIds: string[]) => Promise<void>;
  }) => (
    <section>
      <h1>Study Guides mock</h1>
      <button type="button" onClick={() => void onPracticeCards(["topic-card"])}>
        Practice topic cards mock
      </button>
    </section>
  ),
}));
vi.mock("./screens/SettingsScreen", () => ({
  SettingsScreen: () => <h1>Settings mock</h1>,
}));
vi.mock("./screens/home/HomeScreen", () => ({
  HomeScreen: ({
    onStartReview,
    onOpenSubject,
    onOpenLibrary,
  }: {
    onStartReview: (deckIds: string[]) => void;
    onOpenSubject: (subjectId: string | null, view: "decks" | "documents") => void;
    onOpenLibrary: () => void;
  }) => (
    <section>
      <h1>Home mock</h1>
      <button type="button" onClick={() => onStartReview(["deck-due-a", "deck-due-b"])}>
        Start review mock
      </button>
      <button type="button" onClick={() => onOpenSubject("subject-a", "decks")}>
        Open subject mock
      </button>
      <button type="button" onClick={() => onOpenSubject(null, "documents")}>
        Open unfiled documents mock
      </button>
      <button type="button" onClick={onOpenLibrary}>
        Open Library mock
      </button>
    </section>
  ),
}));
vi.mock("./screens/subject/SubjectScreen", () => ({
  SubjectScreen: ({
    subjectId,
    onBack,
    onOpenLibrary,
  }: {
    subjectId: string;
    onBack: () => void;
    onOpenLibrary: (scope: { subjectId?: string | null; deckId?: string }) => void;
  }) => (
    <section>
      <h1>Subject mock</h1>
      <output aria-label="Subject page">{subjectId}</output>
      <button type="button" onClick={onBack}>
        Back to subjects mock
      </button>
      <button type="button" onClick={() => onOpenLibrary({ subjectId })}>
        Manage subject mock
      </button>
    </section>
  ),
}));
vi.mock("./screens/review/ReviewScreen", () => ({
  ReviewScreen: ({
    onStartReview,
    onChooseDecks,
  }: {
    onStartReview: (deckIds: string[]) => void;
    onChooseDecks: () => void;
  }) => (
    <section>
      <h1>Review mock</h1>
      <button type="button" onClick={() => onStartReview(["deck-due-a"])}>
        Start subject review mock
      </button>
      <button type="button" onClick={onChooseDecks}>
        Choose decks mock
      </button>
    </section>
  ),
}));

function renderApp() {
  return render(
    <OverlayProvider>
      <App />
    </OverlayProvider>,
  );
}

function mainNavigation(): HTMLElement {
  return screen.getByRole("navigation", { name: "Main navigation" });
}

function navButton(name: string): HTMLElement {
  return within(mainNavigation()).getByRole("button", { name });
}

function currentDestination(): string | null {
  return (
    within(mainNavigation())
      .queryByRole("button", { current: "page" })
      ?.querySelector('[data-slot="nav-label"]')?.textContent ?? null
  );
}

function libraryView(name: string): HTMLElement {
  return within(screen.getByRole("navigation", { name: "Library pages" })).getByRole("button", { name });
}

/** Opens the app on a fresh library (Home), then Library's Decks view. */
async function openDecks(): Promise<void> {
  renderApp();
  await screen.findByRole("heading", { name: "Home mock" });
  fireEvent.click(navButton("Library"));
  await screen.findByRole("heading", { name: "Decks mock" });
}

describe("App shell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTestEntityIds();
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue(null);
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(
      makeHomeSummary({ today: { scheduledReviews: 10, scheduledNew: 2, deckIds: ["deck-due-a"] } }),
    );
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary());
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.removeAttribute("data-motion");
    document.documentElement.removeAttribute("data-animations-enabled");
    document.documentElement.removeAttribute("data-reduce-motion");
  });

  it("loads local settings and the deck library before rendering navigation", async () => {
    const deck = makeDeck({ name: "Cardiology" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(
      makeSettings({ theme: "console", motionPreference: "none", uiScale: 110 }),
    );
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    renderApp();

    expect(screen.getByRole("heading", { name: "Opening ostudy" })).toBeTruthy();
    expect(screen.getByText("Loading your library…")).toBeTruthy();
    expect(await screen.findByRole("heading", { name: "Home mock" })).toBeTruthy();
    expect(currentDestination()).toBe("Home");
    fireEvent.click(navButton("Library"));
    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByText("Cardiology")).toBeTruthy();
    expect(document.documentElement.dataset.theme).toBe("console");
    expect(document.documentElement.dataset.motion).toBe("none");
    expect(document.documentElement.dataset.animationsEnabled).toBeUndefined();
    expect(document.documentElement.dataset.reduceMotion).toBeUndefined();
    expect(document.documentElement.style.getPropertyValue("--ui-scale")).toBe("1.1");
    expect(screen.queryByRole("group", { name: "Window controls" })).toBeNull();

    const main = screen.getByRole("main");
    expect(main.getAttribute("data-slot")).toBe("sidebar-inset");
    expect(main.parentElement?.getAttribute("data-ui-root")).toBe("true");
    expect(main.firstElementChild?.className).toContain("routeLayer");
  });

  it("guards navigation with the shared unsaved-change dialog", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Mark card dirty" }));
    fireEvent.click(navButton("Progress"));

    expect(
      await screen.findByRole("alertdialog", { name: "Discard flashcard changes?" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("heading", { name: "Analytics mock" })).toBeNull();

    fireEvent.click(navButton("Progress"));
    await screen.findByRole("alertdialog", { name: "Discard flashcard changes?" });
    fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));

    expect(await screen.findByRole("heading", { name: "Analytics mock" })).toBeTruthy();
    expect(screen.getByLabelText("Analytics launch").textContent).toBe("30d||");
  });

  it("records the open deck without asking the Workbench to move again", async () => {
    const deck = makeDeck({ name: "Pharmacology" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    await openDecks();
    const request = screen.getByLabelText("Workbench request").textContent;
    vi.mocked(checkpointIpc.saveUiCheckpoint).mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Open deck mock" }));

    await waitFor(() =>
      expect(screen.getByLabelText("Workbench launch").textContent).toBe(`deck:${deck.id}`),
    );
    expect(screen.getByLabelText("Workbench request").textContent).toBe(request);
    expect(currentDestination()).toBe("Library");
    await waitFor(() =>
      expect(
        vi.mocked(checkpointIpc.saveUiCheckpoint).mock.calls.some(([saved]) => saved.deckId === deck.id),
      ).toBe(true),
    );

    fireEvent.click(screen.getByRole("button", { name: "Leave deck mock" }));
    await waitFor(() => expect(screen.getByLabelText("Workbench launch").textContent).toBe("none"));
    expect(screen.getByLabelText("Workbench request").textContent).toBe(request);
  });

  it("reopens the Workbench's deck when coming back to Library", async () => {
    const deck = makeDeck({ name: "Pharmacology" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open deck mock" }));
    fireEvent.click(navButton("Home"));
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(navButton("Library"));
    await waitFor(() =>
      expect(screen.getByLabelText("Workbench launch").textContent).toBe(`deck:${deck.id}`),
    );

    // Choosing Library again from inside it returns to All cards.
    fireEvent.click(navButton("Library"));
    await waitFor(() => expect(screen.getByLabelText("Workbench launch").textContent).toBe("none"));
  });

  it("routes Progress tags into the Workbench with fresh request IDs", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(navButton("Progress"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Search current analytics tag mock" }),
    );

    await waitFor(() =>
      expect(screen.getByLabelText("Workbench launch").textContent).toBe("tag:stable-analytics-tag-id"),
    );
    const firstRequest = screen.getByLabelText("Workbench request").textContent;

    fireEvent.click(navButton("Progress"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Search current analytics tag mock" }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("Workbench launch").textContent).toBe("tag:stable-analytics-tag-id"),
    );
    expect(screen.getByLabelText("Workbench request").textContent).not.toBe(firstRequest);
  });

  it("clears a Progress tag launch on ordinary navigation", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(navButton("Progress"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Search current analytics tag mock" }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("Workbench launch").textContent).not.toBe("none"),
    );

    fireEvent.click(navButton("Home"));
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(navButton("Library"));
    await waitFor(() =>
      expect(screen.getByLabelText("Workbench launch").textContent).toBe("none"),
    );
  });

  it("restores a canonical checkpoint normalized from legacy Cards into Search", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 2,
      updatedAt: 1,
      section: "decks",
      flashcardsWorkspace: "search",
      deckId: null,
      editorDraftId: null,
      practice: null,
      analytics: null,
    });

    // A checkpoint from before Home existed still opens where it was saved.
    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    expect(screen.getByLabelText("Workbench launch").textContent).toBe("view:all|search");
    expect(currentDestination()).toBe("Library");
  });

  it("renders a resumable Practice checkpoint as an internal workflow", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 2,
      updatedAt: 1,
      section: "decks",
      flashcardsWorkspace: "decks",
      deckId: null,
      editorDraftId: null,
      practice: {
        sessionId: "session-restore",
        position: 2,
        revealed: true,
        returnTarget: {
          section: "decks",
          flashcardsWorkspace: "decks",
          deckId: null,
        },
      },
      analytics: null,
    });

    renderApp();

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe("none");
    // The session runs inside Review, whichever view started it.
    expect(currentDestination()).toBe("Review");
  });

  it("keeps a setup-only legacy-normalized Practice checkpoint in Flashcards", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 2,
      updatedAt: 1,
      section: "decks",
      flashcardsWorkspace: "decks",
      deckId: null,
      editorDraftId: null,
      practice: null,
      analytics: null,
    });

    renderApp();

    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Practice mock" })).toBeNull();
  });

  it("launches one-step library-row practice and returns to the library", async () => {
    const deck = makeDeck({ name: "Pharmacology" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      `${deck.id}||extra|decks||decks`,
    );
    expect(currentDestination()).toBe("Review");
    // Choosing Review keeps the running session.
    fireEvent.click(navButton("Review"));
    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Consume Practice launch mock" }));
    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe("none");
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Workbench launch").textContent).toBe("none");
  });

  it("carries practice options exactly and returns to the Workbench", async () => {
    const deck = makeDeck({ name: "Pharmacology", revision: 7 });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Practice selected decks mock" }));
    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      `${deck.id}|selected-tag|extra|decks||decks`,
    );
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
  });

  it("opens Study guides as a Library view without exposing a Practice destination", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    expect(within(mainNavigation()).queryByRole("button", { name: "Practice" })).toBeNull();
    fireEvent.click(libraryView("Study guides"));

    expect(await screen.findByRole("heading", { name: "Study Guides mock" })).toBeTruthy();
    expect(libraryView("Study guides").getAttribute("aria-current")).toBe("page");
    expect(currentDestination()).toBe("Library");
    await waitFor(() => expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalled());
    expect(
      vi.mocked(checkpointIpc.saveUiCheckpoint).mock.calls.some(
        ([input]) => input.section === "study-guides",
      ),
    ).toBe(true);
  });

  it("returns topic-linked practice to Study Guides", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(libraryView("Study guides"));
    fireEvent.click(await screen.findByRole("button", { name: "Practice topic cards mock" }));

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      "|topic-card|custom|study-guides||",
    );
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect(await screen.findByRole("heading", { name: "Study Guides mock" })).toBeTruthy();
  });

  it("returns an early-ended practice session to the deck the Workbench had open", async () => {
    const deck = makeDeck({ name: "Respiratory physiology" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open deck mock" }));
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));
    await screen.findByRole("heading", { name: "Practice mock" });
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));

    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Workbench launch").textContent).toBe(`deck:${deck.id}`);
  });

  it("returns exact-card Search practice to Search", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(within(screen.getByTestId("app-header")).getByRole("button", { name: "Search" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Workbench launch").textContent).toBe("view:all|search"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Practice search result mock" }));

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      "|search-card|custom|decks||search",
    );
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect((await screen.findByLabelText("Workbench launch")).textContent).toBe("view:all|search");
  });

  it("returns exact-card Documents practice to Documents", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);
    await openDecks();
    fireEvent.click(libraryView("Documents"));
    fireEvent.click(await screen.findByRole("button", { name: "Practice document card mock" }));
    expect((await screen.findByLabelText("Practice launch")).textContent).toBe(
      "|document-card|custom|documents||",
    );
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect(await screen.findByRole("heading", { name: "Documents mock" })).toBeTruthy();
  });

  it("returns an editor exact-card launch to its deck", async () => {
    const deck = makeDeck({ name: "Editor origin" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);
    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open deck mock" }));
    fireEvent.click(screen.getByRole("button", { name: "Practice editor card mock" }));
    expect((await screen.findByLabelText("Practice launch")).textContent).toBe(
      `|editor-card|custom|decks|${deck.id}|decks`,
    );
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect((await screen.findByLabelText("Workbench launch")).textContent).toBe(`deck:${deck.id}`);
  });

  it("returns scheduled Progress practice to Progress", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(navButton("Progress"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Practice analytics scope mock" }),
    );

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      "|analytics-tag|scheduled|analytics||",
    );
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect(await screen.findByRole("heading", { name: "Analytics mock" })).toBeTruthy();
  });

  it("carries a completed practice session into week-scoped Progress", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));
    fireEvent.click(await screen.findByRole("button", { name: "Open session analytics" }));

    expect(await screen.findByRole("heading", { name: "Analytics mock" })).toBeTruthy();
    expect(screen.getByLabelText("Analytics launch").textContent).toBe(
      "7d|deck-from-practice|session-1",
    );
  });

  it("lands a fresh library on Home, saves it and shows today's count on Review", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    expect(await screen.findByRole("heading", { name: "Home mock" })).toBeTruthy();
    expect(currentDestination()).toBe("Home");
    await waitFor(() =>
      expect(
        vi.mocked(checkpointIpc.saveUiCheckpoint).mock.calls.some(([input]) => input.section === "home"),
      ).toBe(true),
    );
    const review = navButton("Review");
    await waitFor(() => expect(within(review).queryByText("12")).not.toBeNull());
    expect(document.getElementById(review.getAttribute("aria-describedby") ?? "")?.textContent).toBe(
      "12 cards due today",
    );
  });

  it("opens Library with its search focused from the header", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(within(screen.getByTestId("app-header")).getByRole("button", { name: "Search" }));

    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Workbench launch").textContent).toBe("view:all|search");
    expect(currentDestination()).toBe("Library");
  });

  it("opens Settings from the header gear, which no destination marks", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(screen.getByTestId("app-settings"));

    expect(await screen.findByRole("heading", { name: "Settings mock" })).toBeTruthy();
    expect(currentDestination()).toBeNull();
    expect(screen.getByTestId("app-settings").getAttribute("aria-current")).toBe("page");
  });

  it("starts today's review from Home as scheduled study and returns there", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(screen.getByRole("button", { name: "Start review mock" }));

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      "deck-due-a,deck-due-b||scheduled|home||",
    );
    expect(currentDestination()).toBe("Review");
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect(await screen.findByRole("heading", { name: "Home mock" })).toBeTruthy();
  });

  it("runs a session started on Review inside Review and returns there", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(navButton("Review"));
    expect(await screen.findByRole("heading", { name: "Review mock" })).toBeTruthy();
    expect(currentDestination()).toBe("Review");
    fireEvent.click(screen.getByRole("button", { name: "Start subject review mock" }));

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe("deck-due-a||scheduled|review||");
    fireEvent.click(navButton("Review"));
    expect(screen.getByRole("heading", { name: "Practice mock" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect(await screen.findByRole("heading", { name: "Review mock" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Choose decks mock" }));
    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Workbench launch").textContent).toBe("none");
  });

  it("opens a subject's page from Home and manages it in Library", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(screen.getByRole("button", { name: "Open subject mock" }));
    expect(await screen.findByRole("heading", { name: "Subject mock" })).toBeTruthy();
    expect(screen.getByLabelText("Subject page").textContent).toBe("subject-a");
    expect(currentDestination()).toBe("Home");

    fireEvent.click(screen.getByRole("button", { name: "Back to subjects mock" }));
    expect(await screen.findByRole("heading", { name: "Home mock" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Open subject mock" }));
    await screen.findByRole("heading", { name: "Subject mock" });
    // Choosing Home again, or Android Back, returns to the subject list.
    fireEvent.click(navButton("Home"));
    expect(await screen.findByRole("heading", { name: "Home mock" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Open subject mock" }));
    await screen.findByRole("heading", { name: "Subject mock" });
    await waitFor(() => expect(dispatchPlatformBack()).toBe(true));
    expect(await screen.findByRole("heading", { name: "Home mock" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Open subject mock" }));
    fireEvent.click(await screen.findByRole("button", { name: "Manage subject mock" }));
    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Workbench launch").textContent).toBe("subject:subject-a");
    // An ordinary visit to Library carries no subject.
    fireEvent.click(navButton("Home"));
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(navButton("Library"));
    await waitFor(() => expect(screen.getByLabelText("Workbench launch").textContent).toBe("none"));

    fireEvent.click(navButton("Home"));
    fireEvent.click(await screen.findByRole("button", { name: "Open unfiled documents mock" }));
    expect(await screen.findByRole("heading", { name: "Documents mock" })).toBeTruthy();
    expect(libraryView("Documents").getAttribute("aria-current")).toBe("page");
  });

  it("reopens the Library view used last", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(libraryView("Documents"));
    expect(await screen.findByRole("heading", { name: "Documents mock" })).toBeTruthy();
    fireEvent.click(navButton("Tutor"));
    await waitFor(() => expect(currentDestination()).toBe("Tutor"));
    fireEvent.click(navButton("Library"));
    expect(await screen.findByRole("heading", { name: "Documents mock" })).toBeTruthy();

    fireEvent.click(navButton("Home"));
    fireEvent.click(await screen.findByRole("button", { name: "Open Library mock" }));
    expect(await screen.findByRole("heading", { name: "Documents mock" })).toBeTruthy();
  });

  it("returns to Home on Android Back from another destination", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(navButton("Progress"));
    await screen.findByRole("heading", { name: "Analytics mock" });
    await waitFor(() => expect(dispatchPlatformBack()).toBe(true));
    expect(await screen.findByRole("heading", { name: "Home mock" })).toBeTruthy();
    expect(currentDestination()).toBe("Home");
  });

  it("shows a recoverable fatal screen when local bootstrap fails", async () => {
    vi.mocked(settingsIpc.getSettings).mockRejectedValue(new Error("Database unavailable"));
    vi.mocked(decksIpc.listDecks).mockResolvedValue([]);

    renderApp();

    expect(
      await screen.findByRole("heading", { name: "ostudy could not open the library" }),
    ).toBeTruthy();
    expect(screen.getByText("Database unavailable")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
    await waitFor(() => {
      expect(settingsIpc.getSettings).toHaveBeenCalledOnce();
      expect(decksIpc.listDecks).toHaveBeenCalledOnce();
    });
  });
});
