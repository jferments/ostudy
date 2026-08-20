import { useState } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type {
  FlashcardsRootRequest,
  FlashcardsRouteState,
  FlashcardsWorkspaceName,
} from "../../navigation/flashcardsRoutes";
import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
} from "../../platform/backNavigation";
import { FlashcardsWorkspace } from "./FlashcardsWorkspace";

vi.mock("../CardsScreen", () => ({ CardsScreen: () => <div>Search workspace</div> }));
vi.mock("../TemplatesScreen", () => ({ TemplatesScreen: () => null }));
vi.mock("./TransferWorkspace", () => ({ TransferWorkspace: () => null }));
vi.mock("./DeckLibraryWorkspace", async () => {
  const React = await import("react");
  return {
    DeckLibraryWorkspace: ({ rootRequestId, onRouteChange }: { rootRequestId: number; onRouteChange: (route: FlashcardsRouteState) => void }) => {
      const [view, setView] = React.useState<"library" | "detail" | "editor">("library");
      React.useEffect(() => {
        if (rootRequestId > 0) setView("library");
      }, [rootRequestId]);
      React.useEffect(() => {
        onRouteChange({ workspace: "decks", deckView: view, deckId: view === "library" ? null : "a".repeat(32) });
      }, [onRouteChange, view]);
      return (
        <div>
          <output data-testid="mock-deck-view">{view}</output>
          {view === "library" ? <button type="button" onClick={() => setView("detail")}>Open detail</button> : null}
          {view === "detail" ? <button type="button" onClick={() => setView("editor")}>Open editor</button> : null}
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
  tutorialsEnabled: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
  aiReasoningEffort: "low" as const,
};

function Harness({ initial = "decks" }: { initial?: FlashcardsWorkspaceName }) {
  const [workspace, setWorkspace] = useState<FlashcardsWorkspaceName>(initial);
  const [rootRequest, setRootRequest] = useState<FlashcardsRootRequest | null>(null);
  const [sequence, setSequence] = useState(1);
  const navigate = (next: FlashcardsWorkspaceName) => {
    if (next === workspace) {
      setRootRequest({ requestId: sequence, workspace: next });
      setSequence((current) => current + 1);
    } else {
      setWorkspace(next);
    }
  };
  return (
    <FlashcardsWorkspace
      workspace={workspace}
      rootRequest={rootRequest}
      onRouteChange={vi.fn()}
      onWorkspaceChange={navigate}
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

describe("FlashcardsWorkspace explicit Back ownership", () => {
  it("leaves nested deck Back ownership to the deck workflow", async () => {
    const view = render(<Harness />);
    fireEvent.click(screen.getByRole("button", { name: "Open detail" }));
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("detail"));
    expect(platformBackHandlerCountForTests()).toBe(0);
    fireEvent.click(screen.getByRole("button", { name: "Open editor" }));
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("editor"));
    expect(platformBackHandlerCountForTests()).toBe(0);
    view.unmount();
  });

  it("returns a secondary workspace to Decks before leaving Flashcards", async () => {
    const view = render(<Harness initial="search" />);
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(1));
    await act(async () => expect(dispatchPlatformBack()).toBe(true));
    await waitFor(() => expect(screen.getByTestId("mock-deck-view")).toBeTruthy());
    view.unmount();
  });
});
