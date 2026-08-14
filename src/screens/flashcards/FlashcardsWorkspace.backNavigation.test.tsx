import { useState } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { FlashcardsWorkspaceName } from "../../navigation/flashcardsRoutes";
import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
  subscribePlatformBackStack,
} from "../../platform/backNavigation";
import { FlashcardsWorkspace } from "./FlashcardsWorkspace";

vi.mock("../CardsScreen", () => ({ CardsScreen: () => <div>Search workspace</div> }));
vi.mock("../TemplatesScreen", () => ({ TemplatesScreen: () => null }));
vi.mock("./CreateCardsWorkspace", () => ({ CreateCardsWorkspace: () => null }));
vi.mock("./TransferWorkspace", () => ({ TransferWorkspace: () => null }));
vi.mock("../DecksScreen", async () => {
  const React = await import("react");
  return {
    DecksScreen: ({
      onViewChange,
    }: {
      onViewChange?: (view: "library" | "detail" | "editor") => void;
    }) => {
      const [view, setView] = React.useState<"library" | "detail" | "editor">("library");
      React.useEffect(() => onViewChange?.(view), [onViewChange, view]);
      return (
        <div>
          <output data-testid="mock-deck-view">{view}</output>
          {view === "library" ? (
            <button type="button" onClick={() => setView("detail")}>Open detail</button>
          ) : null}
          {view === "detail" ? (
            <>
              <button type="button" className="back-button" onClick={() => setView("library")}>
                Back to library
              </button>
              <button type="button" onClick={() => setView("editor")}>Open editor</button>
            </>
          ) : null}
          {view === "editor" ? (
            <button type="button" className="back-button" onClick={() => setView("detail")}>
              Back to detail
            </button>
          ) : null}
        </div>
      );
    },
  };
});

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
  deckViewDensity: "comfortable" as const,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

function Harness({ initial = "decks" }: { initial?: FlashcardsWorkspaceName }) {
  const [workspace, setWorkspace] = useState<FlashcardsWorkspaceName>(initial);
  return (
    <FlashcardsWorkspace
      workspace={workspace}
      onWorkspaceChange={setWorkspace}
      decks={[]}
      settings={settings}
      refreshDecks={async () => []}
      onPracticeDeck={vi.fn()}
      onPracticeCards={vi.fn()}
      openDeckRequest={null}
      onOpenDeckRequestConsumed={vi.fn()}
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
  it("keeps one handler registered while moving between detail and editor", async () => {
    const counts: number[] = [];
    const unsubscribe = subscribePlatformBackStack((count) => counts.push(count));
    const view = render(<Harness />);

    fireEvent.click(screen.getByRole("button", { name: "Open detail" }));
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(1));
    expect(counts).toEqual([0, 1]);

    fireEvent.click(screen.getByRole("button", { name: "Open editor" }));
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("editor"));
    expect(platformBackHandlerCountForTests()).toBe(1);

    await act(async () => {
      expect(dispatchPlatformBack()).toBe(true);
    });
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("detail"));

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
    expect(screen.getByText("Search workspace")).toBeInTheDocument();

    await act(async () => {
      expect(dispatchPlatformBack()).toBe(true);
    });
    await waitFor(() => expect(screen.getByTestId("mock-deck-view")).toBeInTheDocument());
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(0));
    view.unmount();
  });
});
