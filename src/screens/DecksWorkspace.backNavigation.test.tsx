import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
  subscribePlatformBackStack,
} from "../platform/backNavigation";
import { DecksWorkspace } from "./DecksWorkspace";

vi.mock("../components/AndroidDeckSharePanel", () => ({
  AndroidDeckSharePanel: () => null,
}));
vi.mock("./CardsScreen", () => ({ CardsScreen: () => null }));
vi.mock("./cards/BulkEntryModal", () => ({ BulkEntryModal: () => null }));
vi.mock("./DecksScreen", async () => {
  const React = await import("react");
  return {
    DecksScreen: ({ onViewChange }: { onViewChange?: (view: "library" | "detail" | "editor") => void }) => {
      const [view, setView] = React.useState<"library" | "detail" | "editor">("library");
      React.useEffect(() => onViewChange?.(view), [onViewChange, view]);
      return (
        <div>
          <output data-testid="mock-deck-view">{view}</output>
          {view === "library" ? (
            <button type="button" onClick={() => setView("detail")}>
              Open detail
            </button>
          ) : null}
          {view === "detail" ? (
            <>
              <button
                type="button"
                className="back-button"
                onClick={() => setView("library")}
              >
                Back to library
              </button>
              <button type="button" onClick={() => setView("editor")}>
                Open editor
              </button>
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

afterEach(() => {
  expect(platformBackHandlerCountForTests()).toBe(0);
});

describe("DecksWorkspace Android Back ownership", () => {
  it("keeps one handler registered while moving between detail and editor", async () => {
    const counts: number[] = [];
    const unsubscribe = subscribePlatformBackStack((count) => counts.push(count));
    const view = render(
      <DecksWorkspace
        decks={[]}
        settings={{
          theme: "dark",
          uiFont: "system",
          cardFont: "system",
          uiScale: 100,
          cardBaseSize: 18,
          animationsEnabled: true,
          reduceMotion: false,
          confirmDeletions: true,
          practiceShuffle: true,
          desiredRetention: 0.9,
          dailyNewLimit: 20,
          dailyReviewLimit: 200,
        }}
        refreshDecks={async () => []}
        onPracticeDeck={vi.fn()}
        onPracticeCards={vi.fn()}
        openDeckRequest={null}
        onOpenDeckRequestConsumed={vi.fn()}
        onManageTemplates={vi.fn()}
        onDirtyChange={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Open detail" }));
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(1));
    expect(counts).toEqual([0, 1]);

    fireEvent.click(screen.getByRole("button", { name: "Open editor" }));
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("editor"));
    expect(platformBackHandlerCountForTests()).toBe(1);
    expect(counts).toEqual([0, 1]);

    expect(dispatchPlatformBack()).toBe(true);
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("detail"));
    expect(platformBackHandlerCountForTests()).toBe(1);
    expect(counts).toEqual([0, 1]);

    expect(dispatchPlatformBack()).toBe(true);
    await waitFor(() => expect(screen.getByTestId("mock-deck-view").textContent).toBe("library"));
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(0));
    expect(counts).toEqual([0, 1, 0]);

    unsubscribe();
    view.unmount();
  });
});
