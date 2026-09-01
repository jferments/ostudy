import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AppSettings } from "../../ipc/settings";
import type {
  AnalyticsSearchLaunch,
  FlashcardsWorkspaceName,
} from "../../navigation/flashcardsRoutes";
import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
} from "../../platform/backNavigation";
import { FlashcardsWorkspace } from "./FlashcardsWorkspace";

vi.mock("../CardsScreen", () => ({
  CardsScreen: ({
    analyticsSearchLaunch,
  }: {
    analyticsSearchLaunch?: AnalyticsSearchLaunch;
  }) => (
    <div>
      Search workspace
      <output aria-label="Forwarded Analytics Search launch">
        {analyticsSearchLaunch
          ? `${analyticsSearchLaunch.requestId}|${analyticsSearchLaunch.source}|${analyticsSearchLaunch.tagIds.join(",")}`
          : "none"}
      </output>
    </div>
  ),
}));
vi.mock("../TemplatesScreen", () => ({ TemplatesScreen: () => <div>Templates workspace</div> }));
vi.mock("./TransferWorkspace", () => ({ TransferWorkspace: () => <div>Transfer workspace</div> }));
vi.mock("./DeckLibraryWorkspace", () => ({
  DeckLibraryWorkspace: () => (
    <div>
      <button type="button" className="back-button">
        Rendered child back button
      </button>
      Deck library workspace
    </div>
  ),
}));

const settings: AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

interface RenderWorkspaceOptions {
  workspace?: FlashcardsWorkspaceName;
  navigationKey?: number;
  onWorkspaceChange?: (workspace: FlashcardsWorkspaceName) => void | Promise<void>;
  analyticsSearchLaunch?: AnalyticsSearchLaunch;
}

function renderWorkspace({
  workspace = "decks",
  navigationKey = 1,
  onWorkspaceChange = vi.fn(),
  analyticsSearchLaunch,
}: RenderWorkspaceOptions = {}) {
  return render(
    <FlashcardsWorkspace
      workspace={workspace}
      navigationKey={navigationKey}
      onWorkspaceChange={onWorkspaceChange}
      decks={[]}
      settings={settings}
      refreshDecks={async () => []}
      onPracticeDeck={vi.fn()}
      onPracticeSelected={vi.fn()}
      onPracticeCards={vi.fn()}
      analyticsSearchLaunch={analyticsSearchLaunch}
      openDeckRequest={null}
      onOpenDeck={vi.fn()}
      onReturnToLibrary={vi.fn()}
      onCardDirtyChange={vi.fn()}
      onTemplateDirtyChange={vi.fn()}
      showToast={vi.fn()}
    />,
  );
}

afterEach(() => {
  expect(platformBackHandlerCountForTests()).toBe(0);
});

describe("FlashcardsWorkspace root navigation", () => {
  it("does not infer nested Decks state from rendered back-button classes", () => {
    const view = renderWorkspace();

    expect(screen.getByRole("button", { name: "Rendered child back button" })).toBeTruthy();
    expect(platformBackHandlerCountForTests()).toBe(0);
    view.unmount();
  });

  it("returns a secondary workspace to the Decks root through platform Back", async () => {
    const onWorkspaceChange = vi.fn();
    const view = renderWorkspace({ workspace: "search", onWorkspaceChange });
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(1));
    expect(screen.getByText("Search workspace")).toBeTruthy();

    await act(async () => {
      expect(dispatchPlatformBack()).toBe(true);
    });
    expect(onWorkspaceChange).toHaveBeenCalledWith("decks");
    view.unmount();
  });

  it("forwards the typed launch through the keyed Search workspace boundary", () => {
    const view = renderWorkspace({
      workspace: "search",
      navigationKey: 19,
      analyticsSearchLaunch: {
        requestId: 19,
        source: "analytics",
        tagIds: ["stable-tag-id"],
      },
    });

    expect(screen.getByLabelText("Forwarded Analytics Search launch").textContent).toBe(
      "19|analytics|stable-tag-id",
    );
    view.unmount();
  });

  it("reactivates the selected Decks tab so the parent can return to its root", () => {
    const onWorkspaceChange = vi.fn();
    const view = renderWorkspace({ workspace: "decks", onWorkspaceChange });

    fireEvent.click(screen.getByRole("tab", { name: "Decks" }));
    expect(onWorkspaceChange).toHaveBeenCalledWith("decks");
    view.unmount();
  });

  it("reactivates a selected secondary tab instead of treating it as a no-op", () => {
    const onWorkspaceChange = vi.fn();
    const view = renderWorkspace({ workspace: "transfer", onWorkspaceChange });

    fireEvent.click(screen.getByRole("tab", { name: "Import/Export" }));
    expect(onWorkspaceChange).toHaveBeenCalledWith("transfer");
    view.unmount();
  });

  it("contains exactly the four maintained Flashcards workspaces", () => {
    const view = renderWorkspace();
    const labels = screen.getAllByRole("tab").map((tab) => tab.getAttribute("aria-label"));
    expect(labels).toEqual(["Decks", "Templates", "Search", "Import/Export"]);
    expect(screen.queryByRole("tab", { name: "Create Cards" })).toBeNull();
    view.unmount();
  });
});
