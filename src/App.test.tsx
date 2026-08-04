import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AnalyticsLaunchRequest, PracticeLaunchRequest } from "./App";
import App from "./App";
import * as ipc from "./ipc";
import { makeDeck, makeSettings, resetTestEntityIds } from "./test/factories";

vi.mock("./ipc", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected startup error",
  getSettings: vi.fn(),
  listDecks: vi.fn(),
}));

vi.mock("./screens/DecksScreen", () => ({
  DecksScreen: ({
    decks,
    onPracticeDeck,
    onDirtyChange,
  }: {
    decks: ipc.Deck[];
    onPracticeDeck: (deckId: string) => void;
    onDirtyChange: (dirty: boolean) => void;
  }) => (
    <section>
      <h1>Decks mock</h1>
      <span>{decks.map((deck) => deck.name).join(",")}</span>
      <button type="button" onClick={() => onDirtyChange(true)}>
        Mark card dirty
      </button>
      <button type="button" onClick={() => onPracticeDeck(decks[0]?.id ?? "")}>
        Practice deck mock
      </button>
    </section>
  ),
}));

vi.mock("./screens/CardsScreen", () => ({
  CardsScreen: () => <h1>Search mock</h1>,
}));

vi.mock("./screens/TemplatesScreen", () => ({
  TemplatesScreen: () => <h1>Templates mock</h1>,
}));

vi.mock("./screens/PracticeScreen", () => ({
  PracticeScreen: ({
    launchRequest,
    onViewAnalytics,
  }: {
    launchRequest: PracticeLaunchRequest | null;
    onViewAnalytics: (sessionId: string, deckIds: string[]) => void;
  }) => (
    <section>
      <h1>Practice mock</h1>
      <output aria-label="Practice launch">
        {launchRequest
          ? `${launchRequest.deckIds.join(",")}|${launchRequest.cardIds.join(",")}|${String(
              launchRequest.autoStart,
            )}`
          : "none"}
      </output>
      <button
        type="button"
        onClick={() => onViewAnalytics("session-1", ["deck-from-practice"])}
      >
        Open session analytics
      </button>
    </section>
  ),
}));

vi.mock("./screens/AnalyticsScreen", () => ({
  AnalyticsScreen: ({
    launchRequest,
  }: {
    launchRequest: AnalyticsLaunchRequest | null;
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
    </section>
  ),
}));

vi.mock("./screens/SettingsScreen", () => ({
  SettingsScreen: () => <h1>Settings mock</h1>,
}));

describe("App shell", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTestEntityIds();
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.removeAttribute("data-reduce-motion");
  });

  it("loads the local settings and deck library before rendering navigation", async () => {
    const deck = makeDeck({ name: "Cardiology" });
    vi.mocked(ipc.getSettings).mockResolvedValue(
      makeSettings({ theme: "console", reduceMotion: true, uiScale: 110 }),
    );
    vi.mocked(ipc.listDecks).mockResolvedValue([deck]);

    render(<App />);

    expect(screen.getByRole("heading", { name: "Opening ostudy" })).toBeTruthy();
    expect(await screen.findByRole("heading", { name: "Decks mock" })).toBeTruthy();
    expect(screen.getByText("Cardiology")).toBeTruthy();
    expect(document.documentElement.dataset.theme).toBe("console");
    expect(document.documentElement.dataset.reduceMotion).toBe("true");
    expect(document.documentElement.style.getPropertyValue("--ui-scale")).toBe(
      "1.1",
    );
  });

  it("guards navigation while a card has unsaved changes", async () => {
    vi.mocked(ipc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(ipc.listDecks).mockResolvedValue([makeDeck()]);
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);

    render(<App />);
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Mark card dirty" }));
    fireEvent.click(screen.getByRole("button", { name: /Analytics/u }));

    expect(confirm).toHaveBeenCalledWith(
      "Discard the unsaved changes to this flashcard?",
    );
    expect(screen.queryByRole("heading", { name: "Analytics mock" })).toBeNull();

    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: /Analytics/u }));
    expect(
      await screen.findByRole("heading", { name: "Analytics mock" }),
    ).toBeTruthy();
    expect(screen.getByLabelText("Analytics launch").textContent).toBe("7d||");
  });

  it("launches exact deck practice from the deck screen", async () => {
    const deck = makeDeck({ name: "Pharmacology" });
    vi.mocked(ipc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(ipc.listDecks).mockResolvedValue([deck]);

    render(<App />);
    await screen.findByRole("heading", { name: "Decks mock" });
    fireEvent.click(screen.getByRole("button", { name: "Practice deck mock" }));

    expect(
      await screen.findByRole("heading", { name: "Practice mock" }),
    ).toBeTruthy();
    expect(screen.getByLabelText("Practice launch").textContent).toBe(
      `${deck.id}||true`,
    );
  });

  it("carries a completed practice session into week-scoped analytics", async () => {
    vi.mocked(ipc.getSettings).mockResolvedValue(makeSettings());
    vi.mocked(ipc.listDecks).mockResolvedValue([makeDeck()]);

    render(<App />);
    await screen.findByRole("heading", { name: "Decks mock" });
    const navigation = screen.getByRole("navigation", { name: "Main navigation" });
    fireEvent.click(within(navigation).getByRole("button", { name: "Practice" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Open session analytics" }),
    );

    expect(
      await screen.findByRole("heading", { name: "Analytics mock" }),
    ).toBeTruthy();
    expect(screen.getByLabelText("Analytics launch").textContent).toBe(
      "7d|deck-from-practice|session-1",
    );
  });

  it("shows a recoverable fatal screen when local bootstrap fails", async () => {
    vi.mocked(ipc.getSettings).mockRejectedValue(new Error("Database unavailable"));
    vi.mocked(ipc.listDecks).mockResolvedValue([]);

    render(<App />);

    expect(
      await screen.findByRole("heading", {
        name: "ostudy could not open the library",
      }),
    ).toBeTruthy();
    expect(screen.getByText("Database unavailable")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
    await waitFor(() => {
      expect(ipc.getSettings).toHaveBeenCalledOnce();
      expect(ipc.listDecks).toHaveBeenCalledOnce();
    });
  });
});
