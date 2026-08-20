import { useState } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  flashcardsRootRoute,
  type FlashcardsDeckView,
  type FlashcardsRoute,
  type FlashcardsWorkspaceName,
} from "../../navigation/flashcardsRoutes";
import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
  subscribePlatformBackStack,
} from "../../platform/backNavigation";
import { FlashcardsWorkspace } from "./FlashcardsWorkspace";

vi.mock("../CardsScreen", () => ({ CardsScreen: () => <div>Search workspace</div> }));
vi.mock("../TemplatesScreen", () => ({ TemplatesScreen: () => null }));
vi.mock("./TransferWorkspace", () => ({ TransferWorkspace: () => null }));
vi.mock("./DeckLibraryWorkspace", () => ({
  DeckLibraryWorkspace: ({
    route,
    onViewChange,
  }: {
    route: Extract<FlashcardsRoute, { workspace: "decks" }>;
    onViewChange: (view: FlashcardsDeckView, deckId: string | null) => void;
  }) => (
    <div>
      <output data-testid="mock-deck-view">{route.view}</output>
      {route.view === "library" ? (
        <button type="button" onClick={() => onViewChange("detail", "deck-1")}>
          Open detail
        </button>
      ) : null}
      {route.view === "detail" ? (
        <button type="button" onClick={() => onViewChange("editor", "deck-1")}>
          Open editor
        </button>
      ) : null}
    </div>
  ),
}));

const settings = {
  theme: "dark" as const,
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system" as const,
  materialPreference: "automatic" as const,
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

function Harness({ initial = "decks" }: { initial?: FlashcardsWorkspaceName }) {
  const [route, setRoute] = useState<FlashcardsRoute>(() => flashcardsRootRoute(initial, 1));

  function changeWorkspace(next: FlashcardsWorkspaceName): void {
    setRoute((current) => flashcardsRootRoute(next, current.requestId + 1));
  }

  function changeDeckView(view: FlashcardsDeckView, deckId: string | null): void {
    setRoute((current) => ({
      workspace: "decks",
      view,
      deckId: view === "library" ? null : deckId,
      requestId: current.requestId,
    }));
  }

  return (
    <FlashcardsWorkspace
      route={route}
      onWorkspaceChange={changeWorkspace}
      onDeckViewChange={changeDeckView}
      decks={[]}
      settings={settings}
      refreshDecks={async () => []}
      onPracticeDeck={vi.fn()}
      onPracticeCards={vi.fn()}
      onCardDirtyChange={vi.fn()}
      onTemplateDirtyChange={vi.fn()}
      showToast={vi.fn()}
    />
  );
}

afterEach(() => {
  expect(platformBackHandlerCountForTests()).toBe(0);
});

describe("FlashcardsWorkspace Android Back ownership", () => {
  it("returns a nested deck route to the Decks root without DOM back-button discovery", async () => {
    const counts: number[] = [];
    const unsubscribe = subscribePlatformBackStack((count) => counts.push(count));
    const view = render(<Harness />);

    fireEvent.click(screen.getByRole("button", { name: "Open detail" }));
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("detail"));
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(1));
    expect(counts).toEqual([0, 1]);

    fireEvent.click(screen.getByRole("button", { name: "Open editor" }));
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("editor"));
    expect(platformBackHandlerCountForTests()).toBe(1);

    await act(async () => {
      expect(dispatchPlatformBack()).toBe(true);
    });
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("library"));
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(0));

    unsubscribe();
    view.unmount();
  });

  it("returns a secondary workspace to Decks before leaving Flashcards", async () => {
    const view = render(<Harness initial="search" />);
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(1));
    expect(screen.getByText("Search workspace")).toBeTruthy();

    await act(async () => {
      expect(dispatchPlatformBack()).toBe(true);
    });
    await waitFor(() => expect(screen.getByTestId("mock-deck-view")).toBeTruthy());
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(0));
    view.unmount();
  });
});
