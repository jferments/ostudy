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
import type { SelectedPracticeRequest } from "./ipc/practice";
import * as settingsIpc from "./ipc/settings";
import * as checkpointIpc from "./ipc/uiCheckpoint";
import type {
  AnalyticsSearchLaunch,
  FlashcardsWorkspaceName,
  TransientDeckSelectionEntry,
} from "./navigation/flashcardsRoutes";
import { makeDeck, makeSettings, resetTestEntityIds } from "./test/factories";
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
    openDeckRequest,
    onOpenDeck,
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
    openDeckRequest: DeckOpenRequest | null;
    onOpenDeck: (deckId: string) => void;
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
      <button type="button" onClick={() => onCardDirtyChange(true)}>
        Mark card dirty
      </button>
      <button type="button" onClick={() => onWorkspaceChange("search")}>
        Open Search workspace
      </button>
      <button type="button" onClick={() => onOpenDeck(decks[0]?.id ?? "")}>
        Open deck detail mock
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

function renderApp() {
  return render(
    <OverlayProvider>
      <App />
    </OverlayProvider>,
  );
}

describe("App shell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTestEntityIds();
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue(null);
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
    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByText("Cardiology")).toBeTruthy();
    expect(document.documentElement.dataset.theme).toBe("console");
    expect(document.documentElement.dataset.motion).toBe("none");
    expect(document.documentElement.dataset.animationsEnabled).toBeUndefined();
    expect(document.documentElement.dataset.reduceMotion).toBeUndefined();
    expect(document.documentElement.style.getPropertyValue("--ui-scale")).toBe("1.1");
    expect(screen.queryByRole("group", { name: "Window controls" })).toBeNull();

    const main = screen.getByRole("main");
    expect(main.className).toContain("main");
    expect(main.parentElement?.className).toContain("shell");
    expect(main.firstElementChild?.className).toContain("routeLayer");
  });

  it("guards navigation with the shared unsaved-change dialog", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Mark card dirty" }));
    fireEvent.click(screen.getByRole("button", { name: /Analytics/u }));

    expect(
      await screen.findByRole("alertdialog", { name: "Discard flashcard changes?" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("heading", { name: "Analytics mock" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /Analytics/u }));
    await screen.findByRole("alertdialog", { name: "Discard flashcard changes?" });
    fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));

    expect(await screen.findByRole("heading", { name: "Analytics mock" })).toBeTruthy();
    expect(screen.getByLabelText("Analytics launch").textContent).toBe("7d||");
  });

  it("changes Flashcards workspaces without creating a top-level section", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Open Search workspace" }));

    await waitFor(() =>
      expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("search"),
    );
    expect(screen.getByRole("button", { name: "Flashcards" }).getAttribute("aria-current")).toBe(
      "page",
    );
  });

  it("routes Analytics tags into Search with fresh transient request IDs", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Analytics" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Search current analytics tag mock" }),
    );

    await waitFor(() =>
      expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("search"),
    );
    const firstLaunch = screen.getByLabelText("Analytics Search launch").textContent;
    expect(firstLaunch).toMatch(/^\d+\|analytics\|stable-analytics-tag-id$/u);

    fireEvent.click(screen.getByRole("button", { name: "Analytics" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Search current analytics tag mock" }),
    );
    const secondLaunch = await screen.findByLabelText("Analytics Search launch");
    expect(secondLaunch.textContent).toMatch(
      /^\d+\|analytics\|stable-analytics-tag-id$/u,
    );
    expect(secondLaunch.textContent).not.toBe(firstLaunch);
  });

  it("clears an Analytics Search launch on ordinary workspace navigation", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Analytics" }));
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

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("search");
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
    expect(
      within(screen.getByRole("navigation", { name: "Main navigation" })).queryByRole(
        "button",
        { current: "page" },
      ),
    ).toBeNull();
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

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      `${deck.id}||extra|decks||decks`,
    );
    expect(within(screen.getByTestId("mobile-app-bar")).getByText("Practice")).toBeTruthy();
    expect(
      within(screen.getByRole("navigation", { name: "Main navigation" })).queryByRole(
        "button",
        { current: "page" },
      ),
    ).toBeNull();

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

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
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

  it("opens Study Guides directly without exposing a Practice destination", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    expect(within(navigation).queryByRole("button", { name: "Practice" })).toBeNull();
    fireEvent.click(within(navigation).getByRole("button", { name: "Study Guides" }));

    expect(await screen.findByRole("heading", { name: "Study Guides mock" })).toBeTruthy();
    expect(
      within(navigation).getByRole("button", { name: "Study Guides" }).getAttribute(
        "aria-current",
      ),
    ).toBe("page");
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

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Study Guides" }));
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

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
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

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
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
    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Documents" }));
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
    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Open deck detail mock" }));
    fireEvent.click(screen.getByRole("button", { name: "Practice editor card mock" }));
    expect((await screen.findByLabelText("Practice launch")).textContent).toBe(
      `|editor-card|custom|decks|${deck.id}|decks`,
    );
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));
    expect((await screen.findByLabelText("Deck open request")).textContent).toBe(deck.id);
  });

  it("returns scheduled Analytics practice to Analytics", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Analytics" }));
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

  it("carries a completed practice session into week-scoped analytics", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));
    fireEvent.click(await screen.findByRole("button", { name: "Open session analytics" }));

    expect(await screen.findByRole("heading", { name: "Analytics mock" })).toBeTruthy();
    expect(screen.getByLabelText("Analytics launch").textContent).toBe(
      "7d|deck-from-practice|session-1",
    );
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
