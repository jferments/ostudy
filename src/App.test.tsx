import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  AnalyticsLaunchRequest,
  DeckOpenRequest,
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
import type {
  AnalyticsSearchLaunch,
  FlashcardsWorkspaceName,
  SubjectFocusLaunch,
  TransientDeckSelectionEntry,
} from "./navigation/flashcardsRoutes";
import { dispatchPlatformBack } from "./platform/backNavigation";
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

vi.mock("./screens/flashcards/FlashcardsWorkspace", () => ({
  FlashcardsWorkspace: ({
    workspace,
    decks,
    onWorkspaceChange,
    onPracticeDeck,
    onPracticeSelected,
    onPracticeCards,
    restoredSelection,
    analyticsSearchLaunch,
    subjectFocus,
    openDeckRequest,
    onOpenDeck,
    onReturnToLibrary,
    onCardDirtyChange,
  }: {
    workspace: FlashcardsWorkspaceName;
    decks: decksIpc.DeckSummary[];
    onWorkspaceChange: (workspace: FlashcardsWorkspaceName) => void;
    onPracticeDeck: (deckId: string) => void;
    onPracticeSelected: (
      request: SelectedPracticeRequest,
      selection: readonly TransientDeckSelectionEntry[],
    ) => void;
    onPracticeCards: (cardIds: string[]) => void;
    restoredSelection?: readonly TransientDeckSelectionEntry[];
    analyticsSearchLaunch?: AnalyticsSearchLaunch;
    subjectFocus?: SubjectFocusLaunch;
    openDeckRequest: DeckOpenRequest | null;
    onOpenDeck: (
      deckId: string,
      returnDeckSelection?: readonly TransientDeckSelectionEntry[],
    ) => void;
    onReturnToLibrary: () => void | Promise<void>;
    onCardDirtyChange: (dirty: boolean) => void;
  }) => (
    <section>
      <h1>Decks mock</h1>
      <span>{decks.map((deck) => deck.name).join(",")}</span>
      <output aria-label="Flashcards workspace">{workspace}</output>
      <output aria-label="Deck open request">{openDeckRequest?.deckId ?? "none"}</output>
      <output aria-label="Restored deck selection">
        {restoredSelection?.map((entry) => entry.id).join(",") ?? "none"}
      </output>
      <output aria-label="Analytics Search launch">
        {analyticsSearchLaunch
          ? `${analyticsSearchLaunch.requestId}|${analyticsSearchLaunch.source}|${analyticsSearchLaunch.tagIds.join(",")}`
          : "none"}
      </output>
      <output aria-label="Subject focus">
        {subjectFocus ? `${subjectFocus.requestId}|${subjectFocus.subjectId ?? "none"}` : "none"}
      </output>
      <button type="button" onClick={() => onCardDirtyChange(true)}>
        Mark card dirty
      </button>
      <button type="button" onClick={() => onWorkspaceChange("search")}>
        Open Search workspace
      </button>
      <button
        type="button"
        onClick={() => {
          const deck = decks[0];
          if (deck) onOpenDeck(deck.id, [{ id: deck.id, revision: deck.revision }]);
        }}
      >
        Open deck detail mock
      </button>
      <button type="button" onClick={() => void onReturnToLibrary()}>
        Return to library mock
      </button>
      <button type="button" onClick={() => onPracticeDeck(decks[0]?.id ?? "")}>
        Practice deck mock
      </button>
      <button
        type="button"
        onClick={() => {
          const deck = decks[0];
          if (!deck) return;
          onPracticeSelected(
            {
              deckIds: [deck.id],
              tagIds: ["selected-tag"],
              shuffle: false,
              mode: "extra",
            },
            [{ id: deck.id, revision: deck.revision }],
          );
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
  ),
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

vi.mock("./screens/AnalyticsScreen", () => ({
  AnalyticsScreen: ({
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
  return within(screen.getByRole("navigation", { name: "Library" })).getByRole("button", { name });
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
    expect(screen.getByLabelText("Analytics launch").textContent).toBe("7d||");
  });

  it("changes Flashcards workspaces without creating a top-level section", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open Search workspace" }));

    await waitFor(() =>
      expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("search"),
    );
    expect(currentDestination()).toBe("Library");
    expect(libraryView("Decks").getAttribute("aria-current")).toBe("page");
  });

  it("routes Progress tags into Search with fresh transient request IDs", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(navButton("Progress"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Search current analytics tag mock" }),
    );

    await waitFor(() =>
      expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("search"),
    );
    const firstLaunch = screen.getByLabelText("Analytics Search launch").textContent;
    expect(firstLaunch).toMatch(/^\d+\|analytics\|stable-analytics-tag-id$/u);

    fireEvent.click(navButton("Progress"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Search current analytics tag mock" }),
    );
    const secondLaunch = await screen.findByLabelText("Analytics Search launch");
    expect(secondLaunch.textContent).toMatch(
      /^\d+\|analytics\|stable-analytics-tag-id$/u,
    );
    expect(secondLaunch.textContent).not.toBe(firstLaunch);
  });

  it("clears a Progress Search launch on ordinary workspace navigation", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(navButton("Progress"));
    fireEvent.click(
      await screen.findByRole("button", { name: "Search current analytics tag mock" }),
    );
    await waitFor(() =>
      expect(screen.getByLabelText("Analytics Search launch").textContent).not.toBe("none"),
    );

    fireEvent.click(screen.getByRole("button", { name: "Open Search workspace" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Analytics Search launch").textContent).toBe("none"),
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
    expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("search");
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
    await waitFor(() =>
      expect(screen.getByLabelText("Deck open request").textContent).toBe("none"),
    );
    expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("decks");
  });

  it("carries selected setup exactly and restores its transient deck selection on return", async () => {
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
    await waitFor(() =>
      expect(screen.getByLabelText("Restored deck selection").textContent).toBe(deck.id),
    );
  });

  it("preserves selected decks through the real detail return route without checkpointing them", async () => {
    const deck = makeDeck({
      name: "Pharmacology",
      revision: 7,
      generatedCardCount: 0,
    });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    await openDecks();
    vi.mocked(checkpointIpc.saveUiCheckpoint).mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Open deck detail mock" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Deck open request").textContent).toBe(deck.id),
    );
    await waitFor(() => expect(checkpointIpc.saveUiCheckpoint).toHaveBeenCalled());
    expect(
      vi.mocked(checkpointIpc.saveUiCheckpoint).mock.calls.every(
        ([saved]) => !JSON.stringify(saved).includes("returnDeckSelection"),
      ),
    ).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Return to library mock" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Restored deck selection").textContent).toBe(deck.id),
    );
  });

  it.each([
    [
      "revision changes",
      (candidate: decksIpc.DeckSummary) => ({ ...candidate, revision: 8 }),
    ],
    [
      "the deck is archived",
      (candidate: decksIpc.DeckSummary) => ({
        ...candidate,
        archivedAt: Date.now(),
      }),
    ],
  ] as const)("drops selection when %s before detail returns", async (_label, updateDeck) => {
    const original = makeDeck({ name: "Pharmacology", revision: 7 });
    const changed = updateDeck(original);
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks)
      .mockResolvedValueOnce([original])
      .mockResolvedValueOnce([original])
      .mockResolvedValueOnce([changed]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open deck detail mock" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Deck open request").textContent).toBe(original.id),
    );
    fireEvent.click(screen.getByRole("button", { name: "Return to library mock" }));

    await waitFor(() =>
      expect(screen.getByLabelText("Restored deck selection").textContent).toBe("none"),
    );
    // Startup, opening Library, and the return.
    expect(decksIpc.listDecks).toHaveBeenCalledTimes(3);
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

  it("returns an early-ended deck-detail practice session to that exact deck", async () => {
    const deck = makeDeck({ name: "Respiratory physiology" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open deck detail mock" }));
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));
    await screen.findByRole("heading", { name: "Practice mock" });
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));

    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Deck open request").textContent).toBe(deck.id);
  });

  it("returns exact-card Search practice to the Search workspace", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open Search workspace" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("search"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Practice search result mock" }));

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      "|search-card|custom|decks||search",
    );
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect((await screen.findByLabelText("Flashcards workspace")).textContent).toBe("search");
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

  it("returns an editor exact-card launch to its exact deck detail", async () => {
    const deck = makeDeck({ name: "Editor origin" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);
    await openDecks();
    fireEvent.click(screen.getByRole("button", { name: "Open deck detail mock" }));
    fireEvent.click(screen.getByRole("button", { name: "Practice editor card mock" }));
    expect((await screen.findByLabelText("Practice launch")).textContent).toBe(
      `|editor-card|custom|decks|${deck.id}|decks`,
    );
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect((await screen.findByLabelText("Deck open request")).textContent).toBe(deck.id);
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

  it("opens Search/Filter inside Library from the header", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(within(screen.getByTestId("app-header")).getByRole("button", { name: "Search" }));

    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("search");
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
    expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("decks");
  });

  it("opens a subject from Home in the Library view that shows it", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Home mock" });
    fireEvent.click(screen.getByRole("button", { name: "Open subject mock" }));
    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Subject focus").textContent).toMatch(/^\d+\|subject-a$/u);

    fireEvent.click(navButton("Home"));
    fireEvent.click(await screen.findByRole("button", { name: "Open unfiled documents mock" }));
    expect(await screen.findByRole("heading", { name: "Documents mock" })).toBeTruthy();
    expect(libraryView("Documents").getAttribute("aria-current")).toBe("page");

    fireEvent.click(navButton("Home"));
    fireEvent.click(await screen.findByRole("button", { name: "Open subject mock" }));
    // An ordinary visit to Decks carries no focus request.
    fireEvent.click(await screen.findByRole("button", { name: "Open Search workspace" }));
    await waitFor(() => expect(screen.getByLabelText("Subject focus").textContent).toBe("none"));
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
