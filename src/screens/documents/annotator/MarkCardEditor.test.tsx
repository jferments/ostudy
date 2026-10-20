import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as annotationCardsIpc from "../../../ipc/annotationCards";
import type { CardNote } from "../../../ipc/cards";
import * as decksIpc from "../../../ipc/decks";
import type { DeckSummary } from "../../../ipc/decks";
import { MarkCardEditor, newCardFromQuote } from "./MarkCardEditor";

vi.mock("../../../ipc/annotationCards", () => ({ linkCardNoteToAnnotation: vi.fn() }));
vi.mock("../../../ipc/decks", () => ({ listDecks: vi.fn() }));
// The editor itself has its own tests; this one shows the draft and saves it.
vi.mock("../../CardEditorScreen", () => ({
  CardEditorScreen: ({ deck, card, onSaved, onTemplateGenerated, onCancel }: {
    deck: DeckSummary;
    card: CardNote;
    onSaved: (saved: { noteId: string }, continueAdding: boolean) => void;
    onTemplateGenerated: (result: { createdNoteIds: string[] }, continueAdding: boolean) => void;
    onCancel: () => void;
  }) => (
    <div>
      <p>New card in {deck.name}</p>
      <p>Answer: {card.cardType === "basic" ? card.backText : ""}</p>
      <button type="button" onClick={() => onSaved({ noteId: "n".repeat(32) }, false)}>Save</button>
      <button type="button" onClick={() => onSaved({ noteId: "o".repeat(32) }, true)}>Save + another</button>
      <button type="button" onClick={() => onTemplateGenerated({ createdNoteIds: ["p".repeat(32), "q".repeat(32)] }, false)}>Generate</button>
      <button type="button" onClick={onCancel}>Cancel</button>
    </div>
  ),
}));

const markId = "m".repeat(32);

function deck(id: string, name: string, archivedAt: number | null = null): DeckSummary {
  return { id, name, archivedAt } as DeckSummary;
}

function open(suggestedDeckId: string | null = "2".repeat(32), quote = "The SA node sets the rate") {
  const onSaved = vi.fn();
  const onClose = vi.fn();
  const showToast = vi.fn();
  render(<MarkCardEditor mark={{ id: markId, quote }} suggestedDeckId={suggestedDeckId}
    onSaved={onSaved} onClose={onClose} showToast={showToast} />);
  return { onSaved, onClose, showToast };
}

beforeEach(() => {
  vi.mocked(decksIpc.listDecks).mockResolvedValue([
    deck("1".repeat(32), "Cardiology"),
    deck("2".repeat(32), "Heart chapter"),
    deck("3".repeat(32), "Old deck", 5),
  ]);
  vi.mocked(annotationCardsIpc.linkCardNoteToAnnotation).mockReset();
  vi.mocked(annotationCardsIpc.linkCardNoteToAnnotation).mockResolvedValue({
    noteId: "n".repeat(32), documentId: null, annotationId: markId, pageNumber: 1, quote: "",
  });
});

describe("card from a mark", () => {
  it("opens in the suggested deck with the marked words as the answer, and links what it saves", async () => {
    const { onSaved, showToast } = open();
    expect(await screen.findByText("New card in Heart chapter")).toBeTruthy();
    expect(screen.getByText("Answer: The SA node sets the rate")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Save + another" }));
    await waitFor(() => expect(onSaved).toHaveBeenCalledWith(false));
    expect(annotationCardsIpc.linkCardNoteToAnnotation).toHaveBeenCalledWith("o".repeat(32), markId);

    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onSaved).toHaveBeenLastCalledWith(true));
    expect(annotationCardsIpc.linkCardNoteToAnnotation).toHaveBeenLastCalledWith("n".repeat(32), markId);
    expect(showToast).toHaveBeenLastCalledWith("success", "Card saved.");

    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() => expect(showToast).toHaveBeenLastCalledWith("success", "2 cards saved."));
    expect(annotationCardsIpc.linkCardNoteToAnnotation).toHaveBeenCalledWith("q".repeat(32), markId);
  });

  it("falls back to the first open deck and says when a saved card could not be linked", async () => {
    vi.mocked(annotationCardsIpc.linkCardNoteToAnnotation).mockRejectedValue(new Error("That mark is no longer on the document."));
    const { onSaved, showToast } = open("3".repeat(32));
    expect(await screen.findByText("New card in Cardiology")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(onSaved).toHaveBeenCalledWith(true));
    expect(showToast).toHaveBeenCalledWith("error",
      "The card was saved, but it could not be linked to the mark: That mark is no longer on the document.");
  });

  it("asks for a deck first when the library has none", async () => {
    vi.mocked(decksIpc.listDecks).mockResolvedValue([deck("3".repeat(32), "Old deck", 5)]);
    const { onClose } = open();
    expect(await screen.findByText(/Make a deck in the Library/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("writes each line of the marked words as its own paragraph", () => {
    const card = newCardFromQuote("1".repeat(32), "First line\n\nThird line");
    expect(card).toMatchObject({ noteId: "", cardType: "basic", frontText: "", backText: "First line\n\nThird line" });
    if (card.cardType !== "basic") throw new Error("expected a basic card");
    expect(card.backDoc.content).toEqual([
      { type: "paragraph", content: [{ type: "text", text: "First line" }] },
      { type: "paragraph" },
      { type: "paragraph", content: [{ type: "text", text: "Third line" }] },
    ]);
    const empty = newCardFromQuote("1".repeat(32), "");
    if (empty.cardType !== "basic") throw new Error("expected a basic card");
    expect(empty.backDoc.content).toEqual([{ type: "paragraph" }]);
  });
});
