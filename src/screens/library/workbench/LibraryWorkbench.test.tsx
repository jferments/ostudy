import type { ReactNode } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as cardsIpc from "../../../ipc/cards";
import type { DeckSummary } from "../../../ipc/decks";
import * as documentsIpc from "../../../ipc/documents";
import * as schedulingIpc from "../../../ipc/scheduling";
import * as subjectsIpc from "../../../ipc/subjects";
import { dispatchPlatformBack } from "../../../platform/backNavigation";
import { makeDeck, makeSubject, makeSubjectLibrary } from "../../../test/factories";
import { matchesLayoutQuery } from "../../../test/viewport";
import { LayoutModeProvider } from "../../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../../ui/overlays/OverlayProvider";
import type { LibrarySubjectScope } from "../../../navigation/appDestinations";
import { LibraryFrame } from "../LibraryFrame";
import { LibraryWorkbench, type WorkbenchLaunch } from "./LibraryWorkbench";

vi.mock("../../../ipc/cards", () => ({
  analyzeBulkCards: vi.fn(), bulkCreateCards: vi.fn(), bulkTagCards: vi.fn(), deleteCards: vi.fn(),
  getCardNote: vi.fn(), listMatchingCards: vi.fn(), listTags: vi.fn(), searchCards: vi.fn(), transferCards: vi.fn(),
}));
vi.mock("../../../ipc/common", () => ({
  createMutationId: vi.fn(() => "f".repeat(32)),
  createOperationId: vi.fn(() => "e".repeat(32)),
  getErrorCode: () => null,
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../../ipc/documents", () => ({ listDocuments: vi.fn() }));
vi.mock("../../../ipc/scheduling", () => ({ applyScheduleAction: vi.fn() }));
vi.mock("../../../ipc/subjects", () => ({
  getSubjectLibrary: vi.fn(),
  createSubject: vi.fn(),
  renameSubject: vi.fn(),
  moveSubject: vi.fn(),
  deleteSubject: vi.fn(),
  moveDeckToSubject: vi.fn(),
  moveDocumentToSubject: vi.fn(),
  getSubjectFilingReviews: vi.fn(),
  dismissSubjectFilingReview: vi.fn(),
}));
vi.mock("../../CardEditorScreen", () => ({
  CardEditorScreen: ({ deck, card, onCancel, details, moreActions = [] }: {
    deck: DeckSummary;
    card: { noteId: string } | null;
    onCancel: () => void;
    details?: ReactNode;
    moreActions?: Array<{ id: string; label: string; onSelect: () => void }>;
  }) => (
    <div>
      <span>Editing {card ? card.noteId : "a new card"} in {deck.name}</span>
      <button type="button" onClick={onCancel}>Close editor</button>
      <div role="group" aria-label="Card actions">
        {moreActions.map((action) => <button key={action.id} type="button" onClick={action.onSelect}>{action.label}</button>)}
      </div>
      {details}
    </div>
  ),
}));

const originalMatchMedia = window.matchMedia;

function setViewport(width: number): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesLayoutQuery(query, width),
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

const cardiac = makeSubject({ id: "cardiac", name: "Cardiac physiology", sortOrder: 0 });
const renal = makeSubject({ id: "renal", name: "Renal physiology", sortOrder: 1 });
const lecture = makeDeck({ id: "d".repeat(32), name: "Lecture 7: The cardiac cycle", generatedCardCount: 41, studyableDueCount: 3 });
const lab = makeDeck({ id: "a".repeat(32), name: "Lab handout: PV loops", generatedCardCount: 18, studyableDueCount: 0 });
const loose = makeDeck({ id: "b".repeat(32), name: "Loose deck", generatedCardCount: 5, studyableDueCount: 1 });
const decks = [lecture, lab, loose];

function row(overrides: Partial<cardsIpc.CardBrowserRow> = {}): cardsIpc.CardBrowserRow {
  return {
    cardId: "c".repeat(32), noteId: "9".repeat(32), deckId: lecture.id, deckName: lecture.name,
    cardType: "basic", cardLabel: "Basic", frontText: "During which two phases are all four heart valves closed?",
    backText: "Isovolumic contraction and relaxation", tags: ["valves"], dueAt: 0, lastReviewedAt: 1, reviewCount: 6,
    suspendedAt: null, buriedUntil: null, needsMorePractice: false, createdAt: 1, updatedAt: 1,
    intervalDays: 5, lapses: 1, stability: 5.2, difficulty: 5.1, sourceUnitKind: "slide", sourceUnit: 14,
    ...overrides,
  };
}

function renderWorkbench(width = 1400, launch?: WorkbenchLaunch, subject?: LibrarySubjectScope) {
  setViewport(width);
  const props = {
    subject,
    onSubjectChange: vi.fn(),
    decks,
    refreshDecks: vi.fn(async () => decks),
    launch,
    onPracticeDeck: vi.fn(),
    onPracticeCards: vi.fn(),
    onPracticeSelected: vi.fn(),
    practiceShuffle: true,
    onCardDirtyChange: vi.fn(),
    showToast: vi.fn(),
  };
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <LibraryFrame
          view="decks"
          fill
          onSelectView={vi.fn()}
          subject={subject ?? { kind: "all" }}
          onChooseSubject={vi.fn()}
          onStartReview={vi.fn()}
          onPracticeSubject={vi.fn()}
          showToast={vi.fn()}
        >
          <LibraryWorkbench {...props} />
        </LibraryFrame>
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  return props;
}

function lastSearch(): cardsIpc.CardSearchInput {
  // Count requests ask for one row; the list asks for a page.
  const calls = vi.mocked(cardsIpc.searchCards).mock.calls.filter(([input]) => input.limit > 1);
  return calls.at(-1)![0];
}

describe("LibraryWorkbench", () => {
  beforeEach(() => {
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary({
      subjects: [cardiac, renal],
      decks: [
        { deckId: lecture.id, subjectId: cardiac.id, sortOrder: 0 },
        { deckId: lab.id, subjectId: cardiac.id, sortOrder: 1 },
        { deckId: loose.id, subjectId: null, sortOrder: 0 },
      ],
      checkTheseCount: 2,
    }));
    vi.mocked(documentsIpc.listDocuments).mockResolvedValue([]);
    vi.mocked(cardsIpc.listTags).mockResolvedValue([
      { id: "t".repeat(32), name: "high-yield", sourceNoteCount: 12, generatedCardCount: 12, studyableDueCount: 2 },
    ]);
    vi.mocked(cardsIpc.searchCards).mockResolvedValue({ cards: [row()], totalCount: 1, todayStartAt: 0 });
    vi.mocked(cardsIpc.getCardNote).mockResolvedValue({ noteId: "9".repeat(32), deckId: lecture.id } as never);
    vi.mocked(schedulingIpc.applyScheduleAction).mockResolvedValue({
      affectedCount: 1, requestedCount: 1, changedCount: 1, unchangedCount: 0, rejectedCount: 0, action: "suspend", resolvedDueAt: null,
    });
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
    vi.clearAllMocks();
  });

  it("lists every deck under Everything, without a subject level, and opens on all cards", async () => {
    renderWorkbench();
    const tree = screen.getByRole("navigation", { name: "Library" });
    await waitFor(() => expect(within(tree).getByRole("button", { name: /^Lecture 7: The cardiac cycle\s*\d/u })).toBeTruthy());
    expect(within(tree).getByRole("button", { name: /^Lab handout: PV loops\s*\d/u })).toBeTruthy();
    expect(within(tree).getByRole("button", { name: /^Loose deck\s*\d/u })).toBeTruthy();
    expect(within(tree).queryByRole("button", { name: /^Cardiac physiology/u })).toBeNull();
    expect(within(tree).queryByRole("button", { name: /^No subject/u })).toBeNull();
    expect(within(tree).getByRole("button", { name: /Check these \(2\)/u })).toBeTruthy();
    expect(within(tree).getByRole("button", { name: /All cards/u }).getAttribute("aria-current")).toBe("true");
    await waitFor(() => expect(screen.getByText("During which two phases are all four heart valves closed?")).toBeTruthy());
    expect(lastSearch()).toMatchObject({ deckIds: [], statusFilter: "all", sort: "updated_desc" });
  });

  it("shows a deck's cards in source order with its due and interval", async () => {
    renderWorkbench();
    const tree = screen.getByRole("navigation", { name: "Library" });
    fireEvent.click(await within(tree).findByRole("button", { name: /^Lecture 7: The cardiac cycle\s*\d/u }));
    await waitFor(() => expect(lastSearch()).toMatchObject({ deckIds: [lecture.id], sort: "source_order" }));
    expect(screen.getByTestId("workbench-title").textContent).toBe(lecture.name);
    expect(screen.getByText("Cardiac physiology", { selector: "p" })).toBeTruthy();
    expect(await screen.findByText("Slide")).toBeTruthy();
    expect(screen.getByText("14")).toBeTruthy();
    expect(screen.getByText("today")).toBeTruthy();
    expect(screen.getByText("5 d")).toBeTruthy();
    expect(within(screen.getByRole("combobox", { name: "Show cards" })).getByRole("option", { name: /^Due today/u })).toBeTruthy();
  });

  it("filters any scope with Show, including Often missed", async () => {
    renderWorkbench();
    const tree = screen.getByRole("navigation", { name: "Library" });
    fireEvent.click(await within(tree).findByRole("button", { name: /^Lecture 7: The cardiac cycle\s*\d/u }));
    await waitFor(() => expect(lastSearch()).toMatchObject({ deckIds: [lecture.id] }));
    fireEvent.change(screen.getByRole("combobox", { name: "Show cards" }), { target: { value: "often_missed" } });
    await waitFor(() => expect(lastSearch()).toMatchObject({ deckIds: [lecture.id], statusFilter: "needs_more_practice" }));
  });

  it("puts Library's pages in tabs above the tree, with Import deck beside them", async () => {
    renderWorkbench();
    const tabs = screen.getByRole("navigation", { name: "Library pages" });
    expect(within(tabs).getByRole("button", { name: "Flashcards" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("button", { name: "Import deck" })).toBeTruthy();
    const tree = screen.getByRole("navigation", { name: "Library" });
    expect(within(tree).queryByRole("button", { name: "Documents" })).toBeNull();
    expect(within(tree).queryByRole("button", { name: /^Due today/u })).toBeNull();
  });

  it("keeps a phone's tab row to the tabs and offers Import deck beside New deck", async () => {
    renderWorkbench(390);
    const tabs = await screen.findByRole("navigation", { name: "Library pages" });
    expect(within(tabs.parentElement!).queryByRole("button", { name: "Import deck" })).toBeNull();
    const tree = screen.getByRole("navigation", { name: "Library" });
    fireEvent.click(within(tree).getByRole("button", { name: "Import deck" }));
    expect(await screen.findByRole("dialog", { name: "Import deck" })).toBeTruthy();
  });

  it("lists only Library's chosen subject's decks and tags, and keeps a tag to that subject", async () => {
    const props = renderWorkbench(1400, { requestId: 1, scope: { kind: "subject", subjectId: cardiac.id } }, {
      kind: "subject",
      subjectId: cardiac.id,
    });
    const tree = screen.getByRole("navigation", { name: "Library" });
    const root = await within(tree).findByRole("button", { name: /^All of Cardiac physiology\s*\d/u });
    expect(root.getAttribute("aria-current")).toBe("true");
    expect(within(tree).getByRole("button", { name: /^Lab handout: PV loops\s*\d/u })).toBeTruthy();
    expect(within(tree).queryByRole("button", { name: /^Loose deck/u })).toBeNull();
    await waitFor(() => expect(cardsIpc.listTags).toHaveBeenLastCalledWith([lecture.id, lab.id]));
    await waitFor(() => expect(props.onSubjectChange).toHaveBeenLastCalledWith({ kind: "subject", subjectId: cardiac.id }));
    fireEvent.click(await within(tree).findByRole("button", { name: /^high-yield\s*\d/u }));
    await waitFor(() => expect(lastSearch()).toMatchObject({ deckIds: [lecture.id, lab.id], tagIds: ["t".repeat(32)] }));
  });

  it("returns a deck to all of the subject's cards on Back", async () => {
    renderWorkbench(1400, { requestId: 1, scope: { kind: "subject", subjectId: cardiac.id } }, {
      kind: "subject",
      subjectId: cardiac.id,
    });
    const tree = screen.getByRole("navigation", { name: "Library" });
    fireEvent.click(await within(tree).findByRole("button", { name: /^Lab handout: PV loops\s*\d/u }));
    await waitFor(() => expect(screen.getByTestId("workbench-title").textContent).toBe(lab.name));
    await waitFor(() => expect(dispatchPlatformBack()).toBe(true));
    await waitFor(() => expect(screen.getByTestId("workbench-title").textContent).toBe(cardiac.name));
  });

  it("scopes a subject to its own decks", async () => {
    renderWorkbench(1400, { requestId: 1, scope: { kind: "subject", subjectId: cardiac.id } });
    await waitFor(() => expect(lastSearch()).toMatchObject({ deckIds: [lecture.id, lab.id], sort: "source_order" }));
  });

  it("opens a card in a panel over the list with its schedule and Suspend in its menu", async () => {
    renderWorkbench();
    fireEvent.click(await screen.findByRole("button", { name: /During which two phases/u }));
    expect(await screen.findByText(`Editing ${"9".repeat(32)} in ${lecture.name}`)).toBeTruthy();
    const schedule = screen.getByRole("region", { name: "Schedule" });
    expect(within(schedule).getByText("5.1 / 10")).toBeTruthy();
    // The list stays mounted under the panel, hidden and inert, so closing returns to it as it was.
    expect(screen.queryByRole("list", { name: "Cards" })).toBeNull();
    const list = screen.getByRole("list", { name: "Cards", hidden: true });
    expect(list.closest("[inert]")).toBeTruthy();
    fireEvent.click(within(screen.getByRole("group", { name: "Card actions" })).getByRole("button", { name: "Suspend" }));
    await waitFor(() => expect(schedulingIpc.applyScheduleAction).toHaveBeenCalledWith(
      expect.objectContaining({ action: "suspend", cardIds: ["c".repeat(32)] }),
    ));
    fireEvent.click(screen.getByRole("button", { name: "Close editor" }));
    expect(screen.getByRole("list", { name: "Cards" })).toBe(list);
  });

  it("suspends the selected cards from the selection bar", async () => {
    renderWorkbench();
    fireEvent.click(await screen.findByRole("checkbox", { name: /Select During which/u }));
    const bar = await screen.findByRole("region", { name: "Selection actions" });
    fireEvent.click(within(bar).getByRole("button", { name: "Suspend" }));
    await waitFor(() => expect(schedulingIpc.applyScheduleAction).toHaveBeenCalledWith(
      expect.objectContaining({ action: "suspend", cardIds: ["c".repeat(32)] }),
    ));
  });

  it("shows one page at a time on a phone", async () => {
    renderWorkbench(390);
    const tree = screen.getByRole("navigation", { name: "Library" });
    fireEvent.click(await within(tree).findByRole("button", { name: /^Loose deck\s*\d/u }));
    expect(await screen.findByTestId("workbench-title")).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Library" })).toBeNull();
    fireEvent.click(await screen.findByRole("button", { name: /During which two phases/u }));
    expect(await screen.findByText(/Editing/u)).toBeTruthy();
    expect(screen.queryByRole("list", { name: "Cards" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Close editor" }));
    fireEvent.click(await screen.findByRole("button", { name: "Library" }));
    expect(screen.getByRole("navigation", { name: "Library" })).toBeTruthy();
  });

  it("steps back from the card list to the tree on Android Back", async () => {
    renderWorkbench(390);
    const tree = screen.getByRole("navigation", { name: "Library" });
    expect(dispatchPlatformBack()).toBe(false);
    fireEvent.click(await within(tree).findByRole("button", { name: /^Loose deck\s*\d/u }));
    expect(await screen.findByTestId("workbench-title")).toBeTruthy();
    // The list registers its Back handler in an effect after it first paints;
    // an unhandled Back changes nothing, so retrying until it is handled is safe.
    await waitFor(() => expect(dispatchPlatformBack()).toBe(true));
    expect(await screen.findByRole("navigation", { name: "Library" })).toBeTruthy();
    expect(dispatchPlatformBack()).toBe(false);
  });

  it("returns a deck to All cards on Back on wider screens", async () => {
    renderWorkbench();
    const tree = screen.getByRole("navigation", { name: "Library" });
    fireEvent.click(await within(tree).findByRole("button", { name: /^Loose deck\s*\d/u }));
    await waitFor(() => expect(screen.getByTestId("workbench-title").textContent).toBe(loose.name));
    // The Back handler registers in an effect after the deck's title paints;
    // an unhandled Back changes nothing, so retrying until it is handled is safe.
    await waitFor(() => expect(dispatchPlatformBack()).toBe(true));
    await waitFor(() => expect(screen.getByTestId("workbench-title").textContent).toBe("All cards"));
  });

  it("focuses search when the header's Search opens Library", async () => {
    renderWorkbench(1400, { requestId: 1, scope: { kind: "view", view: "all" }, focusSearch: true });
    const search = screen.getByRole("searchbox");
    await waitFor(() => expect(document.activeElement).toBe(search));
    fireEvent.change(search, { target: { value: "valves" } });
    await waitFor(() => expect(lastSearch()).toMatchObject({ query: "valves", sort: "relevance" }));
  });
});
