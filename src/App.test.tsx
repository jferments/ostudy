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
import * as settingsIpc from "./ipc/settings";
import * as checkpointIpc from "./ipc/uiCheckpoint";
import type { FlashcardsWorkspaceName } from "./navigation/flashcardsRoutes";
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
        schemaVersion: 1 as const,
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
    openDeckRequest,
    onCardDirtyChange,
  }: {
    workspace: FlashcardsWorkspaceName;
    decks: decksIpc.DeckSummary[];
    onWorkspaceChange: (workspace: FlashcardsWorkspaceName) => void;
    onPracticeDeck: (deckId: string, origin: "library" | "detail") => void;
    openDeckRequest: DeckOpenRequest | null;
    onCardDirtyChange: (dirty: boolean) => void;
  }) => (
    <section>
      <h1>Decks mock</h1>
      <span>{decks.map((deck) => deck.name).join(",")}</span>
      <output aria-label="Flashcards workspace">{workspace}</output>
      <output aria-label="Deck open request">{openDeckRequest?.deckId ?? "none"}</output>
      <button type="button" onClick={() => onCardDirtyChange(true)}>
        Mark card dirty
      </button>
      <button type="button" onClick={() => onWorkspaceChange("search")}>
        Open Search workspace
      </button>
      <button type="button" onClick={() => onPracticeDeck(decks[0]?.id ?? "", "detail")}>
        Practice deck mock
      </button>
    </section>
  ),
}));

vi.mock("./screens/PracticeScreen", () => ({
  PracticeScreen: ({
    launchRequest,
    onViewAnalytics,
    onEndEarly,
  }: {
    launchRequest: PracticeLaunchRequest | null;
    onViewAnalytics: (sessionId: string, deckIds: string[]) => void;
    onEndEarly: (target: PracticeReturnTarget) => void;
  }) => (
    <section>
      <h1>Practice mock</h1>
      <output aria-label="Practice launch">
        {launchRequest
          ? `${launchRequest.deckIds.join(",")}|${launchRequest.cardIds.join(",")}|${String(
              launchRequest.autoStart,
            )}|${launchRequest.mode}|${launchRequest.returnTarget?.section ?? "none"}|${
              launchRequest.returnTarget?.deckId ?? ""
            }`
          : "none"}
      </output>
      <button
        type="button"
        onClick={() => onViewAnalytics("session-1", ["deck-from-practice"])}
      >
        Open session analytics
      </button>
      <button
        type="button"
        onClick={() => {
          if (launchRequest?.returnTarget) onEndEarly(launchRequest.returnTarget);
        }}
      >
        End early mock
      </button>
    </section>
  ),
}));

vi.mock("./screens/AnalyticsScreen", () => ({
  AnalyticsScreen: ({ launchRequest }: { launchRequest: AnalyticsLaunchRequest | null }) => (
    <section>
      <h1>Analytics mock</h1>
      <output aria-label="Analytics launch">
        {launchRequest
          ? `${launchRequest.range}|${launchRequest.deckIds.join(",")}|${
              launchRequest.highlightedSessionId ?? ""
            }`
          : "none"}
      </output>
    </section>
  ),
}));

vi.mock("./screens/DocumentsSection", () => ({
  DocumentsSection: () => <h1>Documents mock</h1>,
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

  it("normalizes a legacy Cards checkpoint into the Search workspace", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);
    vi.mocked(checkpointIpc.getUiCheckpoint).mockResolvedValue({
      schemaVersion: 1,
      updatedAt: 1,
      section: "cards",
      flashcardsWorkspace: null,
      deckId: null,
      editorDraftId: null,
      practice: null,
      analytics: null,
    });

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    expect(screen.getByLabelText("Flashcards workspace").textContent).toBe("search");
  });

  it("launches exact deck practice from the deck screen", async () => {
    const deck = makeDeck({ name: "Pharmacology" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));

    expect(await screen.findByRole("heading", { name: "Practice mock" })).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      `${deck.id}||true|extra|decks|${deck.id}`,
    );
  });

  it("returns an early-ended deck-detail practice session to that exact deck", async () => {
    const deck = makeDeck({ name: "Respiratory physiology" });
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));
    await screen.findByRole("heading", { name: "Practice mock" });
    fireEvent.click(screen.getByRole("button", { name: "End early mock" }));

    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByLabelText("Deck open request").textContent).toBe(deck.id);
  });

  it("carries a completed practice session into week-scoped analytics", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(decksIpc.listDecks).mockResolvedValue([makeDeck()]);

    renderApp();
    await screen.findByRole("heading", { name: "Decks mock" });
    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    fireEvent.click(within(navigation).getByRole("button", { name: "Practice" }));
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
