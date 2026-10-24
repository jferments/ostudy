import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { analyzeBulkCards, bulkCreateCards } from "../../ipc/cards";
import { listDecks, type DeckSummary } from "../../ipc/decks";
import type { LessonStep } from "../../ipc/lessons";
import { getSubjectLibrary, type SubjectLibrary } from "../../ipc/subjects";
import { SlideCards } from "./SlideCards";

vi.mock("../../ipc/cards", () => ({ analyzeBulkCards: vi.fn(), bulkCreateCards: vi.fn() }));
vi.mock("../../ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("../../ipc/subjects", () => ({ getSubjectLibrary: vi.fn() }));

function deck(id: string, name: string, archivedAt: number | null = null): DeckSummary {
  return {
    id, name, description: "", sourceNoteCount: 0, generatedCardCount: 0, studyableDueCount: 0,
    archivedAt, createdAt: 0, updatedAt: 0, revision: 1,
  };
}

function library(placements: SubjectLibrary["decks"]): SubjectLibrary {
  return {
    libraryRevision: 1, documentFilingRevision: 1, subjects: [], decks: placements,
    documents: [], studyRanges: [], leftOutDocumentIds: [], checkTheseCount: 0,
  };
}

const step: LessonStep = {
  key: "raas",
  title: "Renin and ACE",
  blocks: [
    { kind: "term", term: "Renin", explanation: "An enzyme the kidney releases." },
    { kind: "explanation", heading: null, body: "Teaching only." },
    { kind: "term", term: "ACE", explanation: "Turns angiotensin I into II." },
    { kind: "misconception", misconception: "ACE only acts in the lungs.", correction: "It is on vessel walls everywhere." },
  ],
  narration: "",
  slidePoints: [],
  checkKind: null,
  check: null,
  knowledgeUnitIds: [],
} as unknown as LessonStep;

const rows = [
  { question: "Renin", answer: "An enzyme the kidney releases.", tags: [] },
  { question: "ACE", answer: "Turns angiotensin I into II.", tags: [] },
  { question: "True or false: ACE only acts in the lungs.", answer: "False. It is on vessel walls everywhere.", tags: [] },
];

function show(showToast = vi.fn()) {
  render(<SlideCards subjectId="cardio" step={step} showToast={showToast} />);
  return showToast;
}

describe("SlideCards", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(listDecks).mockResolvedValue([deck("other", "Other"), deck("heart", "Heart"), deck("old", "Old", 5), deck("drugs", "Drugs")]);
    vi.mocked(getSubjectLibrary).mockResolvedValue(library([
      { deckId: "other", subjectId: null, sortOrder: 0 },
      { deckId: "drugs", subjectId: "cardio", sortOrder: 1 },
      { deckId: "heart", subjectId: "cardio", sortOrder: 0 },
      { deckId: "old", subjectId: "cardio", sortOrder: 2 },
    ]));
    vi.mocked(analyzeBulkCards).mockResolvedValue({ uniqueCount: 2, duplicateCount: 1, duplicateRowIndices: [1] });
    vi.mocked(bulkCreateCards).mockResolvedValue({ createdCount: 1, skippedDuplicates: 0 });
  });

  it("adds the picked cards the subject's first deck lacks", async () => {
    const showToast = show();
    fireEvent.click(await screen.findByRole("button", { name: "3 cards from this slide" }));
    await waitFor(() => expect(analyzeBulkCards).toHaveBeenCalledWith({ deckId: "heart", rows }));
    expect(await screen.findByText("In Heart")).toBeTruthy();
    expect((screen.getByRole("combobox", { name: "Deck" }) as HTMLSelectElement).value).toBe("heart");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["Heart", "Drugs"]);

    fireEvent.click(screen.getAllByRole("checkbox")[2]!);
    fireEvent.click(screen.getByRole("button", { name: "Add to deck" }));
    await waitFor(() => expect(bulkCreateCards).toHaveBeenCalledWith({ deckId: "heart", rows: [rows[0]] }));
    await waitFor(() => expect(showToast).toHaveBeenCalledWith("success", "1 card added to Heart."));
    // The deck is read again, so what was just added shows as in it.
    await waitFor(() => expect(analyzeBulkCards).toHaveBeenCalledTimes(2));
  });

  it("checks another deck when one is chosen, and says why adding failed", async () => {
    vi.mocked(bulkCreateCards).mockRejectedValue(new Error("The deck was deleted."));
    show();
    fireEvent.click(await screen.findByRole("button", { name: "3 cards from this slide" }));
    fireEvent.change(await screen.findByRole("combobox", { name: "Deck" }), { target: { value: "drugs" } });
    await waitFor(() => expect(analyzeBulkCards).toHaveBeenLastCalledWith({ deckId: "drugs", rows }));
    await screen.findByText("In Drugs");
    fireEvent.click(screen.getByRole("button", { name: "Add to deck" }));
    expect(await screen.findByText(/The cards could not be added/)).toBeTruthy();
  });

  it("uses every live deck when the subject has none, and shows nothing without cards or decks", async () => {
    vi.mocked(getSubjectLibrary).mockResolvedValue(library([]));
    const { unmount } = render(<SlideCards subjectId="cardio" step={step} showToast={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: "3 cards from this slide" }));
    await waitFor(() => expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["Other", "Heart", "Drugs"]));
    unmount();

    vi.mocked(listDecks).mockResolvedValue([]);
    const { container } = render(<SlideCards subjectId="cardio" step={step} showToast={vi.fn()} />);
    await waitFor(() => expect(listDecks).toHaveBeenCalledTimes(2));
    expect(container.textContent).toBe("");

    const quiet = { ...step, blocks: [{ kind: "explanation", heading: null, body: "Teaching only." }] } as LessonStep;
    const { container: empty } = render(<SlideCards subjectId="cardio" step={quiet} showToast={vi.fn()} />);
    expect(empty.textContent).toBe("");
  });
});
