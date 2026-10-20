import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as annotationCardsIpc from "../../../ipc/annotationCards";
import type { CardNote } from "../../../ipc/cards";
import * as decksIpc from "../../../ipc/decks";
import type { DeckSummary } from "../../../ipc/decks";
import * as knowledgeIpc from "../../../ipc/knowledge";
import { MarkCardEditor, newCardFromQuote } from "./MarkCardEditor";

vi.mock("../../../ipc/annotationCards", () => ({ linkCardNoteToAnnotation: vi.fn(), draftMarkQuestion: vi.fn() }));
vi.mock("../../../ipc/knowledge", () => ({ cancelLearningTask: vi.fn() }));
vi.mock("../../../ipc/decks", () => ({ listDecks: vi.fn() }));
// The editor itself has its own tests; this one shows the draft and saves it.
vi.mock("../../CardEditorScreen", () => ({
  CardEditorScreen: ({ deck, card, onSaved, onTemplateGenerated, onCancel, onDirtyChange }: {
    deck: DeckSummary;
    card: CardNote;
    onDirtyChange: (dirty: boolean) => void;
    onSaved: (saved: { noteId: string }, continueAdding: boolean) => void;
    onTemplateGenerated: (result: { createdNoteIds: string[] }, continueAdding: boolean) => void;
    onCancel: () => void;
  }) => (
    <div>
      <p>New card in {deck.name}</p>
      <p>Question: {card.cardType === "basic" ? card.frontText : ""}</p>
      <p>Answer: {card.cardType === "basic" ? card.backText : ""}</p>
      <button type="button" onClick={() => onDirtyChange(true)}>Edit</button>
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
  vi.mocked(annotationCardsIpc.draftMarkQuestion).mockReset();
  vi.mocked(knowledgeIpc.cancelLearningTask).mockReset();
  vi.mocked(knowledgeIpc.cancelLearningTask).mockResolvedValue(undefined);
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

  it("fills in a question the AI writes for the marked words", async () => {
    vi.mocked(annotationCardsIpc.draftMarkQuestion).mockResolvedValue({ question: "What sets the heart rate?" });
    open();
    fireEvent.click(await screen.findByRole("button", { name: "Suggest a question" }));
    expect(await screen.findByText("Question: What sets the heart rate?")).toBeTruthy();
    expect(screen.getByText("Answer: The SA node sets the rate")).toBeTruthy();
    const [operationId, annotationId] = vi.mocked(annotationCardsIpc.draftMarkQuestion).mock.calls.at(0)!;
    expect(operationId).toMatch(/^[0-9a-f]{32}$/u);
    expect(annotationId).toBe(markId);
  });

  it("offers the question instead once the card has been edited", async () => {
    let finish: (draft: { question: string }) => void = () => undefined;
    vi.mocked(annotationCardsIpc.draftMarkQuestion).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    open();
    fireEvent.click(await screen.findByRole("button", { name: "Suggest a question" }));
    expect(screen.getByText("Writing a question…")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    finish({ question: "What sets the heart rate?" });
    expect(await screen.findByText("What sets the heart rate?")).toBeTruthy();
    expect(screen.getByText("Question:")).toBeTruthy();
  });

  it("stops writing on request and says when a question could not be written", async () => {
    vi.mocked(annotationCardsIpc.draftMarkQuestion).mockReturnValueOnce(new Promise(() => undefined));
    open();
    fireEvent.click(await screen.findByRole("button", { name: "Suggest a question" }));
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    const [operationId] = vi.mocked(annotationCardsIpc.draftMarkQuestion).mock.calls.at(-1)!;
    expect(knowledgeIpc.cancelLearningTask).toHaveBeenCalledWith(operationId);

    vi.mocked(annotationCardsIpc.draftMarkQuestion).mockRejectedValueOnce(new Error("Choose a default Flashcard generation model in Settings → AI."));
    fireEvent.click(screen.getByRole("button", { name: "Suggest a question" }));
    expect(await screen.findByText(/A question could not be written: Choose a default Flashcard generation model/u)).toBeTruthy();
  });

  it("offers no question for a mark without words", async () => {
    open("2".repeat(32), "");
    expect(await screen.findByText("New card in Heart chapter")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Suggest a question" })).toBeNull();
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
