import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import type { DeckSummary } from "../../ipc/decks";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { CardPracticeProvider } from "../cards/CardPracticeContext";
import { DeckDetailRoute } from "./DeckDetailRoute";

const deck: DeckSummary = {
  id: "a".repeat(32), name: "Cardiovascular pharmacology", description: "NCLEX medication safety",
  sourceNoteCount: 2, generatedCardCount: 15, studyableDueCount: 15,
  archivedAt: null, createdAt: 1, updatedAt: 2, revision: 3,
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
vi.mock("../../components/SourceCitationPanel", () => ({ SourceCitationPanel: () => <aside>Source citation</aside> }));
vi.mock("../cards/CardBrowserWorkspace", () => ({
  CardBrowserWorkspace: ({ fixedDeckId, onViewChange }: {
    fixedDeckId?: string; onViewChange?: (view: "list" | "editor") => void;
  }) => {
    const [draft, setDraft] = useState("");
    return <section aria-label="Deck cards" data-deck-id={fixedDeckId}>
      What is the first-line intervention?
      <input aria-label="Unsaved card" value={draft} onChange={(event) => setDraft(event.currentTarget.value)} />
      <button onClick={() => onViewChange?.("editor")}>Open editor</button>
      <button onClick={() => onViewChange?.("list")}>Close editor</button>
    </section>;
  },
}));
function renderRoute() {
  render(<LayoutModeProvider><OverlayProvider><CardPracticeProvider onPracticeCards={vi.fn()}>
    <DeckDetailRoute decks={[deck]} refreshDecks={async () => [deck]} onPracticeDeck={vi.fn()}
      openRequest={{ requestId: 9, deckId: deck.id }} onReturnToLibrary={vi.fn()} onDirtyChange={vi.fn()} showToast={vi.fn()} />
  </CardPracticeProvider></OverlayProvider></LayoutModeProvider>);
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
  it("places source provenance after the card listing", () => {
    renderRoute();
    const cards = screen.getByRole("region", { name: "Deck cards" });
    const source = screen.getByText("Source citation");
    expect(cards.compareDocumentPosition(source) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
  it("preserves the browser and its unsaved state when the deck header is hidden for authoring", () => {
    renderRoute();
    const browser = screen.getByRole("region", { name: "Deck cards" });
    fireEvent.change(screen.getByRole("textbox", { name: "Unsaved card" }), { target: { value: "An unsaved card draft" } });
    fireEvent.click(screen.getByRole("button", { name: "Open editor" }));
    expect(screen.queryByRole("heading", { name: deck.name })).toBeNull();
    expect(screen.queryByText("Source citation")).toBeNull();
    expect(screen.getByRole("region", { name: "Deck cards" })).toBe(browser);
    expect(screen.getByDisplayValue("An unsaved card draft")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Close editor" }));
    expect(screen.getByRole("heading", { name: deck.name })).toBeTruthy();
    expect(screen.getByDisplayValue("An unsaved card draft")).toBeTruthy();
    expect(screen.getByRole("region", { name: "Deck cards" })).toBe(browser);
  });
});
