import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PracticeCard, PracticeSession, ReviewPreviewSet } from "../../ipc/practice";
import type { QuestionAttemptView, SavePracticeAnswerInput } from "../../ipc/questionAttempts";
import { makeCardPresentation, paragraphDocument, requiredTestValue as required, resetTestEntityIds } from "../../test/factories";
import { layoutWidths, matchesLayoutQuery } from "../../test/viewport";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { ActivePractice } from "./ActivePractice";
vi.mock("../../editor/RichTextView", () => ({ RichTextView: ({ className = "" }: { className?: string }) => <div className={className}>Rich card content</div> }));
const previews: ReviewPreviewSet = {
  scheduleRevision: 2, previewedAt: 1, previews: [
    { outcome: "wrong", dueAt: 61_000, intervalMs: 60_000 },
    { outcome: "not_confident", dueAt: 3_601_000, intervalMs: 3_600_000 },
    { outcome: "somewhat_confident", dueAt: 86_401_000, intervalMs: 86_400_000 },
    { outcome: "very_confident", dueAt: 2_592_001_000, intervalMs: 2_592_000_000 },
  ],
};
function makeCurrent(): PracticeCard { return { presentation: makeCardPresentation(), deckName: "Cardiology", dueAt: 1, scheduleState: 2, needsMorePractice: true }; }
const choiceAttempt: QuestionAttemptView = {
  instanceId: "c".repeat(32), revision: 1, assisted: false, progress: { phase: "answering", savedAnswer: null },
  question: { kind: "single_choice", prompt: paragraphDocument("Pick one"), options: [
    { id: "a".repeat(32), content: paragraphDocument("Alpha") }, { id: "b".repeat(32), content: paragraphDocument("Beta") },
  ] },
};
function makeQuestionCard(): PracticeCard {
  const { cardId, noteId, deckId } = makeCardPresentation();
  return { deckName: "Cardiology", dueAt: 1, scheduleState: 2, needsMorePractice: false, presentation: {
    cardId, noteId, deckId, direction: "forward", frontText: "Pick one", createdAt: 1, updatedAt: 1, noteRevision: 1,
    question: { attempt: choiceAttempt, assets: [] },
  } };
}
function makeSession(): PracticeSession {
  return { id: "a".repeat(32), cardCount: 1, firstWindow: { startPosition: 0, cards: [makeCurrent()], hasMore: false }, shuffle: false, mode: "scheduled", startedAt: 1 };
}
function renderActive(overrides: Partial<React.ComponentProps<typeof ActivePractice>> = {}) {
  const session = makeSession();
  const props: React.ComponentProps<typeof ActivePractice> = {
    session, current: session.firstWindow.cards[0] as PracticeCard, position: 0, revealed: false, ratedCount: 0, previewSet: null,
    isRevealing: false, isRating: false, isEnding: false, isManaging: false, errorMessage: null,
    onReveal: vi.fn(), onScore: vi.fn(), onEdit: vi.fn(), onScheduleAction: vi.fn(), onEnd: vi.fn(), onLinkError: vi.fn(), ...overrides,
  };
  return { props, ...render(<LayoutModeProvider><ActivePractice {...props} /></LayoutModeProvider>) };
}
describe("ActivePractice", () => {
  beforeEach(() => { resetTestEntityIds(); vi.clearAllMocks(); });
  it("reveals with Space or Enter when focus is not in an interactive control", () => {
    const { props } = renderActive(); fireEvent.keyDown(window, { key: " " }); fireEvent.keyDown(window, { key: "Enter" });
    expect(props.onReveal).toHaveBeenCalledTimes(2);
  });
  it("does not steal reveal shortcuts from interactive controls", () => {
    const { props } = renderActive(); const input = document.createElement("input"); document.body.append(input);
    fireEvent.keyDown(input, { key: "Enter" }); fireEvent.keyDown(screen.getByRole("button", { name: "Show answer" }), { key: " " });
    expect(props.onReveal).not.toHaveBeenCalled(); input.remove();
  });
  it("keeps the animated card readable content and reveals through a real button", () => {
    const { props } = renderActive({ flipAnimationEnabled: true });
    expect(screen.queryByRole("button", { name: "Reveal answer" })).toBeNull();
    const question = screen.getByText("Question");
    expect(question.closest("button, [role='button']")).toBeNull();
    expect(question.closest("[aria-hidden='true']")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show answer" }));
    expect(props.onReveal).toHaveBeenCalledOnce();
  });
  it("still reveals when the animated card face is clicked, as a pointer convenience", () => {
    const { props } = renderActive({ flipAnimationEnabled: true }); fireEvent.click(screen.getByText("Question"));
    expect(props.onReveal).toHaveBeenCalledOnce();
  });
  it("flips an already revealed animated card back and forth without changing review state", () => {
    const { props } = renderActive({ flipAnimationEnabled: true, revealed: true, previewSet: previews });
    const front = screen.getByText("Question").closest("[aria-hidden]"); const back = screen.getByText("Answer").closest("[aria-hidden]");
    expect(front?.getAttribute("aria-hidden")).toBe("true"); expect(back?.getAttribute("aria-hidden")).toBe("false");
    expect(front?.hasAttribute("inert")).toBe(true); expect(back?.hasAttribute("inert")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Show question" }));
    expect(screen.getByRole("button", { name: "Show answer" })).toBeTruthy();
    expect(front?.getAttribute("aria-hidden")).toBe("false"); expect(back?.getAttribute("aria-hidden")).toBe("true");
    expect(front?.hasAttribute("inert")).toBe(false); expect(back?.hasAttribute("inert")).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Show answer" }));
    expect(screen.getByRole("button", { name: "Show question" })).toBeTruthy();
    expect(front?.getAttribute("aria-hidden")).toBe("true"); expect(back?.getAttribute("aria-hidden")).toBe("false");
    fireEvent.click(screen.getByText("Answer"));
    expect(screen.getByRole("button", { name: "Show answer" })).toBeTruthy();
    expect(front?.getAttribute("aria-hidden")).toBe("false");
    expect(props.onReveal).not.toHaveBeenCalled();
  });
  it("offers no turn-over button on a static card", () => {
    renderActive({ flipAnimationEnabled: false, revealed: true, previewSet: previews });
    expect(screen.queryByRole("button", { name: "Show question" })).toBeNull();
    expect(screen.getByText("Answer").closest("[aria-hidden='true']")).toBeNull();
  });
  it("shows only the answer after a Basic-style reveal by default", () => {
    renderActive({ flipAnimationEnabled: false, revealed: true, previewSet: previews, displayQuestionOnAnswer: false });
    expect(screen.queryByText("Question")).toBeNull(); expect(screen.getByText("Answer")).toBeTruthy();
  });
  it("can keep the question visible beside the answer after reveal", () => {
    renderActive({ flipAnimationEnabled: false, revealed: true, previewSet: previews, displayQuestionOnAnswer: true });
    expect(screen.getByText("Question")).toBeTruthy(); expect(screen.getByText("Answer")).toBeTruthy();
  });
  it("shows concise ratings and submits 0-3 keyboard scores after reveal", () => {
    const { props } = renderActive({ revealed: true, previewSet: previews });
    for (const name of ["0 Wrong", "1 Hard", "2 Medium", "3 Easy"]) expect(screen.getByRole("button", { name })).toBeTruthy();
    expect(screen.queryByText(/About 1/u)).toBeNull(); expect(screen.queryByText("I could not recall it.")).toBeNull();
    for (const key of ["0", "1", "2", "3"]) fireEvent.keyDown(window, { key });
    expect(props.onScore).toHaveBeenNthCalledWith(1, 0); expect(props.onScore).toHaveBeenNthCalledWith(2, 1);
    expect(props.onScore).toHaveBeenNthCalledWith(3, 2); expect(props.onScore).toHaveBeenNthCalledWith(4, 3);
  });
  it("rates from one labeled group of four, by pointer, and takes focus to it when the answer shows", () => {
    const { props, rerender } = renderActive({ previewSet: previews });
    rerender(<LayoutModeProvider><ActivePractice {...props} revealed /></LayoutModeProvider>);
    const group = screen.getByRole("region", { name: "How difficult was that answer?" });
    expect(document.activeElement).toBe(group);
    const ratings = within(group).getAllByRole("button");
    expect(ratings.map((button) => button.getAttribute("aria-label"))).toEqual(["0 Wrong", "1 Hard", "2 Medium", "3 Easy"]);
    expect(ratings.map((button) => button.getAttribute("aria-keyshortcuts"))).toEqual(["0", "1", "2", "3"]);
    fireEvent.click(within(group).getByRole("button", { name: "2 Medium" }));
    expect(props.onScore).toHaveBeenCalledOnce(); expect(props.onScore).toHaveBeenCalledWith(2);
  });
  it("offers to retry the interval preview and keeps the ratings disabled until it arrives", () => {
    const { props } = renderActive({ revealed: true, previewSet: null });
    for (const name of ["0 Wrong", "1 Hard", "2 Medium", "3 Easy"]) expect((screen.getByRole("button", { name }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Retry interval preview" }));
    expect(props.onReveal).toHaveBeenCalledOnce();
  });
  it("puts a question's Check answer where a flashcard's Show answer sits", async () => {
    const onSaveQuestionAnswer = vi.fn(async (input: SavePracticeAnswerInput): Promise<QuestionAttemptView> => ({
      ...choiceAttempt, revision: input.request.expectedRevision + 1, progress: { phase: "answering", savedAnswer: input.request.answer },
    }));
    renderActive({ current: makeQuestionCard(), onSaveQuestionAnswer });
    expect(screen.queryByRole("button", { name: "Show answer" })).toBeNull();
    const check = screen.getByRole("button", { name: "Check answer" }) as HTMLButtonElement;
    const card = required(document.querySelector<HTMLElement>("[data-tour-anchor='practice.card']"), "card");
    expect(card.contains(check)).toBe(false); expect(check.closest("[data-practice-actions]")).not.toBeNull();
    expect(check.disabled).toBe(true);
    fireEvent.click(required(screen.getAllByRole("radio")[0], "first choice"));
    await waitFor(() => expect(onSaveQuestionAnswer).toHaveBeenCalledOnce());
    expect((screen.getByRole("button", { name: "Check answer" }) as HTMLButtonElement).disabled).toBe(false);
  });
  it("blocks shortcuts and buttons while any review mutation is pending", () => {
    const { props } = renderActive({ revealed: true, previewSet: previews, isRating: true });
    fireEvent.keyDown(window, { key: "2" }); expect(props.onScore).not.toHaveBeenCalled();
    expect((screen.getByRole("button", { name: /2 Medium/u }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "End session" }) as HTMLButtonElement).disabled).toBe(true);
  });
  it("also blocks rating while schedule or end operations are active", () => {
    const { props, rerender } = renderActive({ revealed: true, previewSet: previews, isManaging: true });
    const medium = screen.getByRole("button", { name: "2 Medium" }) as HTMLButtonElement;
    expect(medium.disabled).toBe(true); fireEvent.click(medium); expect(props.onScore).not.toHaveBeenCalled();
    rerender(<LayoutModeProvider><ActivePractice {...props} isManaging={false} isEnding /></LayoutModeProvider>);
    expect(medium.disabled).toBe(true); fireEvent.keyDown(window, { key: "2" }); expect(props.onScore).not.toHaveBeenCalled();
  });
  it("places the timer-style Schedule control before Edit card", () => {
    renderActive(); const schedule = screen.getByRole("button", { name: "Schedule" }), edit = screen.getByRole("button", { name: "Edit card" });
    expect(Boolean(schedule.compareDocumentPosition(edit) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(true);
    expect(schedule.querySelector("svg")).not.toBeNull();
  });
  it("opens the card's cited page from beside Edit card once the answer shows", () => {
    const onOpenSource = vi.fn();
    const { rerender, props } = renderActive({ sourcePage: 12, onOpenSource });
    expect(screen.queryByRole("button", { name: "Source, page 12" })).toBeNull();
    rerender(<LayoutModeProvider><ActivePractice {...props} revealed previewSet={previews} /></LayoutModeProvider>);
    fireEvent.click(screen.getByRole("button", { name: "Source, page 12" }));
    expect(onOpenSource).toHaveBeenCalledOnce();
  });
  it("opens the card editor from an active session", () => {
    const { props } = renderActive(); fireEvent.click(screen.getByRole("button", { name: "Edit card" })); expect(props.onEdit).toHaveBeenCalledOnce();
  });
  it("routes shared menu schedule controls to the exact-card action callback", () => {
    const { props } = renderActive(); const onScheduleAction = vi.mocked(props.onScheduleAction);
    for (const label of ["Bury until tomorrow", "Suspend card", "Reschedule for tomorrow", "Reset schedule"]) {
      fireEvent.click(screen.getByRole("button", { name: "Schedule" })); fireEvent.click(screen.getByRole("menuitem", { name: label }));
    }
    expect(onScheduleAction).toHaveBeenNthCalledWith(1, "bury"); expect(onScheduleAction).toHaveBeenNthCalledWith(2, "suspend");
    expect(onScheduleAction).toHaveBeenNthCalledWith(3, "reschedule", { kind: "tomorrow" }); expect(onScheduleAction).toHaveBeenNthCalledWith(4, "reset");
  });
  it("reports studied-card progress before the current card is reviewed", () => {
    renderActive(); const progress = screen.getByRole("progressbar", { name: "Practice session progress" });
    expect(progress.getAttribute("aria-valuenow")).toBe("0"); expect(progress.getAttribute("aria-valuemax")).toBe("1");
    expect(progress.getAttribute("aria-valuetext")).toBe("Card 1 of 1, 0 studied, 1 remaining, 0% complete");
    expect(screen.getByText("Practice · 0 studied · 1 remaining")).toBeTruthy();
  });
  it("renders authoritative session context without a weak-card header badge", () => {
    renderActive({ session: { ...makeSession(), mode: "extra" } });
    expect(screen.getByText("Extra practice · 0 studied · 1 remaining")).toBeTruthy(); expect(screen.getByRole("heading", { name: "Cardiology" })).toBeTruthy();
    expect(screen.queryByText("Needs more practice")).toBeNull(); expect(screen.queryByLabelText("1 of 1 cards, 0 studied, 1 remaining")).toBeNull();
    expect(screen.getByRole("button", { name: "Schedule" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "End session" }).getAttribute("data-practice-action")).toBe("end");
  });
  it("handles an empty session count without division or count drift", () => {
    renderActive({ session: { ...makeSession(), cardCount: 0 }, ratedCount: 3 });
    const progress = screen.getByRole("progressbar", { name: "Practice session progress" });
    expect(progress.getAttribute("aria-valuenow")).toBe("0"); expect(progress.getAttribute("aria-valuemax")).toBe("0");
    expect(progress.getAttribute("aria-valuetext")).toBe("Card 0 of 0, 0 studied, 0 remaining, 0% complete");
    expect(screen.getByText("Practice · 0 studied · 0 remaining")).toBeTruthy();
  });
});
describe("ActivePractice on a phone", () => {
  const originalMatchMedia = window.matchMedia;
  beforeEach(() => {
    resetTestEntityIds(); vi.clearAllMocks();
    window.matchMedia = vi.fn((query: string) => ({
      matches: matchesLayoutQuery(query, layoutWidths.compact), media: query, addEventListener: vi.fn(), removeEventListener: vi.fn(),
    }) as unknown as MediaQueryList);
  });
  afterEach(() => { window.matchMedia = originalMatchMedia; });
  it("keeps End session in the bar and gathers Edit card and the schedule actions in one menu", () => {
    const { props } = renderActive({ session: { ...makeSession(), mode: "extra" } });
    const end = screen.getByRole("button", { name: "End session" });
    expect(end.textContent).toBe("End session"); expect(end.getAttribute("data-practice-action")).toBe("end");
    expect(screen.queryByRole("button", { name: "Edit card" })).toBeNull(); expect(screen.queryByRole("button", { name: "Schedule" })).toBeNull();
    expect(screen.getByText("0 studied · 1 remaining")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Card actions" }));
    const sheet = screen.getByRole("dialog", { name: "Card actions" });
    // The sheet's own close control is an icon; the actions are the labeled rows.
    expect(within(sheet).getAllByRole("button").map((button) => button.textContent).filter(Boolean))
      .toEqual(["Edit card", "Bury until tomorrow", "Suspend card", "Reschedule for tomorrow", "Reset schedule"]);
    fireEvent.click(within(sheet).getByRole("button", { name: "Edit card" }));
    expect(props.onEdit).toHaveBeenCalledOnce(); expect(screen.queryByRole("dialog", { name: "Card actions" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Card actions" }));
    fireEvent.click(within(screen.getByRole("dialog", { name: "Card actions" })).getByRole("button", { name: "Suspend card" }));
    expect(props.onScheduleAction).toHaveBeenCalledWith("suspend");
  });
  it("keeps Show answer, then all four ratings, together in the action area after the card", () => {
    const { props, rerender } = renderActive({ previewSet: previews });
    const card = required(document.querySelector<HTMLElement>("[data-tour-anchor='practice.card']"), "card");
    const reveal = screen.getByRole("button", { name: "Show answer" });
    const actions = required(reveal.closest<HTMLElement>("[data-practice-actions]"), "action area");
    expect(card.contains(actions)).toBe(false);
    expect(Boolean(card.compareDocumentPosition(actions) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(true);
    fireEvent.click(reveal); expect(props.onReveal).toHaveBeenCalledOnce();
    rerender(<LayoutModeProvider><ActivePractice {...props} revealed /></LayoutModeProvider>);
    const area = required(document.querySelector<HTMLElement>("[data-practice-actions]"), "action area");
    expect(within(area).getAllByRole("button").map((button) => button.textContent)).toEqual(["0Wrong", "1Hard", "2Medium", "3Easy"]);
    fireEvent.keyDown(window, { key: "0" }); expect(props.onScore).toHaveBeenCalledWith(0);
  });
  it("disables the card actions menu with the other controls while a change is pending", () => {
    renderActive({ isManaging: true });
    expect((screen.getByRole("button", { name: "Card actions" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "End session" }) as HTMLButtonElement).disabled).toBe(true);
  });
  it("offers the card's source in its menu once the answer shows", () => {
    const onOpenSource = vi.fn();
    renderActive({ sourcePage: 12, onOpenSource, revealed: true, previewSet: previews });
    fireEvent.click(screen.getByRole("button", { name: "Card actions" }));
    fireEvent.click(within(screen.getByRole("dialog", { name: "Card actions" })).getByRole("button", { name: "Source, page 12" }));
    expect(onOpenSource).toHaveBeenCalledOnce();
  });
});
