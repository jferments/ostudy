import { act, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { bulkCreateCards } from "../../ipc/cards";
import { listDecks, type DeckSummary } from "../../ipc/decks";
import { cancelLearningTask } from "../../ipc/knowledge";
import { draftTutorAnswerCards } from "../../ipc/tutor";
import { TutorAnswerCards } from "./TutorAnswerCards";

vi.mock("../../ipc/cards", () => ({ bulkCreateCards: vi.fn() }));
vi.mock("../../ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("../../ipc/knowledge", () => ({ cancelLearningTask: vi.fn() }));
vi.mock("../../ipc/tutor", () => ({ draftTutorAnswerCards: vi.fn() }));
vi.mock("../../ui/overlays/Dialog", () => ({
  Sheet: ({ title, children, actions }: { title: string; children: ReactNode; actions?: ReactNode }) => (
    <section role="dialog" aria-label={title}>{children}<div>{actions}</div></section>
  ),
}));

const turnId = "f".repeat(32);
function deck(id: string, name: string, archivedAt: number | null = null): DeckSummary {
  return {
    id: id.repeat(32),
    name,
    description: "",
    sourceNoteCount: 0,
    generatedCardCount: 0,
    studyableDueCount: 0,
    archivedAt,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(cancelLearningTask).mockResolvedValue(undefined as never);
  vi.mocked(listDecks).mockResolvedValue([deck("a", "Geometry"), deck("b", "Old", 5)]);
  vi.mocked(draftTutorAnswerCards).mockResolvedValue([
    { question: "How many sides has a triangle?", answer: "Three" },
    { question: "What do a triangle's angles add up to?", answer: "180 degrees" },
  ]);
  vi.mocked(bulkCreateCards).mockResolvedValue({ createdCount: 1, skippedDuplicates: 0 });
});

describe("Make flashcards from a Tutor answer", () => {
  it("adds only the kept, edited cards to the chosen deck", async () => {
    render(<TutorAnswerCards turnId={turnId} onClose={() => undefined} />);
    expect(screen.getByText("Writing cards from this answer…")).toBeTruthy();
    expect(await screen.findByRole("textbox", { name: "Card 1 question" })).toBeTruthy();
    expect(draftTutorAnswerCards).toHaveBeenCalledWith(expect.any(String), turnId);
    // One live deck needs no deck choice; the archived one is not offered.
    expect(screen.queryByRole("combobox", { name: "Deck" })).toBeNull();
    fireEvent.change(screen.getByRole("textbox", { name: "Card 1 answer" }), { target: { value: "Three sides" } });
    fireEvent.click(screen.getByRole("checkbox", { name: "Keep card 2" }));
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Add 1 card" })); });
    expect(bulkCreateCards).toHaveBeenCalledWith({
      deckId: "a".repeat(32),
      rows: [{ question: "How many sides has a triangle?", answer: "Three sides", tags: [] }],
    });
    expect(screen.getByRole("status").textContent).toBe("1 card added to Geometry.");
    expect(screen.getByRole("button", { name: "Done" })).toBeTruthy();
  });

  it("says how many cards the deck already held", async () => {
    vi.mocked(bulkCreateCards).mockResolvedValue({ createdCount: 1, skippedDuplicates: 1 });
    render(<TutorAnswerCards turnId={turnId} onClose={() => undefined} />);
    const add = await screen.findByRole("button", { name: "Add 2 cards" });
    await act(async () => { fireEvent.click(add); });
    expect(screen.getByRole("status").textContent).toBe("1 card added to Geometry. 1 was already there.");
  });

  it("asks for a deck first when there is none, and offers to try a failed draft again", async () => {
    vi.mocked(listDecks).mockResolvedValue([]);
    vi.mocked(draftTutorAnswerCards).mockRejectedValueOnce(new Error("The model is offline."));
    render(<TutorAnswerCards turnId={turnId} onClose={() => undefined} />);
    expect(await screen.findByText(/The model is offline\./u)).toBeTruthy();
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Try again" })); });
    expect(draftTutorAnswerCards).toHaveBeenCalledTimes(2);
    expect(await screen.findByText(/Make a deck in the Library/u)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Add / })).toBeNull();
  });

  it("stops cards still being written when the sheet closes", async () => {
    vi.mocked(draftTutorAnswerCards).mockReturnValue(new Promise(() => undefined));
    const view = render(<TutorAnswerCards turnId={turnId} onClose={() => undefined} />);
    const operationId = vi.mocked(draftTutorAnswerCards).mock.calls[0]?.[0];
    view.unmount();
    expect(cancelLearningTask).toHaveBeenCalledWith(operationId);
  });
});
