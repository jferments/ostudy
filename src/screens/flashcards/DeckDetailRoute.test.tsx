import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../ipc/decks";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../ui/overlays/overlayContext";
import { CardPracticeProvider } from "../cards/CardPracticeContext";
import { DeckDetailRoute } from "./DeckDetailRoute";

const deck: DeckSummary = {
  id: "a".repeat(32),
  name: "Cardiovascular pharmacology",
  description: "NCLEX medication safety",
  sourceNoteCount: 2,
  generatedCardCount: 15,
  studyableDueCount: 15,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 2,
  revision: 3,
};

vi.mock("../../controllers/decksController", () => ({
  DecksController: class {
    deck = vi.fn().mockResolvedValue(deck);
    setArchived = vi.fn();
    deleteDeck = vi.fn();
    undo = vi.fn();
    prepareExport = vi.fn();
    invalidate = vi.fn();
  },
}));

vi.mock("../../components/SourceCitationPanel", () => ({
  SourceCitationPanel: () => <aside>Source citation</aside>,
}));

vi.mock("../cards/CardBrowserWorkspace", () => ({
  CardBrowserWorkspace: ({ fixedDeckId }: { fixedDeckId?: string }) => (
    <section aria-label="Deck cards" data-deck-id={fixedDeckId}>
      What is the first-line intervention?
    </section>
  ),
}));

function renderRoute() {
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <CardPracticeProvider onPracticeCards={vi.fn()}>
          <DeckDetailRoute
            decks={[deck]}
            refreshDecks={async () => [deck]}
            onPracticeDeck={vi.fn()}
            openRequest={{ requestId: 9, deckId: deck.id }}
            onReturnToLibrary={vi.fn()}
            onDirtyChange={vi.fn()}
            showToast={vi.fn()}
          />
        </CardPracticeProvider>
      </OverlayProvider>
    </LayoutModeProvider>,
  );
}

describe("DeckDetailRoute", () => {
  it("renders the selected deck and its card browser on the first route render", () => {
    renderRoute();

    expect(screen.getByRole("heading", { name: deck.name })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Deck cards" })).toBeTruthy();
    expect(screen.getByText("What is the first-line intervention?")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Decks" })).toBeNull();
    expect(screen.queryByRole("region", { name: "Deck library" })).toBeNull();
  });
});
