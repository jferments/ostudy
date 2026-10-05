import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PublicQuestion, QuestionAttemptView, QuestionEvaluation, QuestionResponse } from "../../ipc/questionAttempts";
import { InteractiveQuestionActivity } from "./InteractiveQuestionActivity";
import { QuestionBody } from "./QuestionBody";
import { connectMatching, matchingRows } from "./questionPresentation";

import { requiredTestValue as required } from "../../test/factories";
vi.mock("../../editor/RichTextView", () => ({
  RichTextView: ({ document }: { document: { content?: Array<{ content?: Array<{ text?: string }> }> } }) => (
    <span>{document.content?.flatMap((node) => node.content ?? []).map((node) => node.text ?? "").join(" ")}</span>
  ),
}));
const id = (value: string) => value.repeat(32).slice(0, 32);
const rich = (text: string) => ({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] });
const item = (value: string, text: string) => ({ id: id(value), content: rich(text) });
const matching: Extract<PublicQuestion, { kind: "matching" }> = {
  kind: "matching", prompt: rich("Match the pairs"), items: [item("a", "Left A"), item("b", "Left B"), item("c", "Left C")],
  targets: [item("x", "Right X"), item("y", "Right Y"), item("z", "Right Z")],
};
const grouping: Extract<PublicQuestion, { kind: "grouping" }> = {
  kind: "grouping", prompt: rich("Group"), items: [item("a", "Dachshund"), item("b", "Maine Coon")],
  categories: [item("x", "Dogs"), item("y", "Cats")],
};
function baseAttempt(question: PublicQuestion): QuestionAttemptView {
  return { instanceId: id("i"), revision: 1, assisted: false, question, progress: { phase: "answering", savedAnswer: null } };
}
function mount(question: PublicQuestion) {
  const attempt = baseAttempt(question);
  const onSave = vi.fn().mockImplementation(async (input) => ({ ...attempt, revision: input.request.expectedRevision + 1,
    progress: { phase: "answering", savedAnswer: input.request.answer } }));
  const onCheck = vi.fn(); const onChecked = vi.fn();
  const view = render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
    disabled={false} onSave={onSave} onCheck={onCheck} onAttempt={vi.fn()} onChecked={onChecked} onError={vi.fn()} />);
  return { ...view, attempt, onSave, onCheck, onChecked };
}
/** Presses a tile's button and drags. The tile captures the pointer, so the browser delivers
 * the rest of the gesture to the tile rather than to its button. */
function drag(handle: HTMLElement, target: HTMLElement, pointerType = "touch", cancel = false) {
  const tile = required(handle.closest<HTMLElement>("[data-question-item-id]"), "dragged tile");
  for (const name of ["setPointerCapture", "releasePointerCapture", "hasPointerCapture"]) {
    Object.defineProperty(tile, name, { configurable: true, value: vi.fn(() => true) });
  }
  Object.defineProperty(document, "elementFromPoint", { configurable: true, value: vi.fn(() => target) });
  fireEvent.pointerDown(handle, { pointerId: 7, button: 0, clientX: 1, clientY: 1, pointerType });
  fireEvent.pointerMove(tile, { pointerId: 7, clientX: 50, clientY: 50, pointerType });
  if (cancel) fireEvent.pointerCancel(tile, { pointerId: 7, pointerType });
  else fireEvent.pointerUp(tile, { pointerId: 7, clientX: 50, clientY: 50, pointerType });
}
function evaluation(components: QuestionEvaluation["components"], verdict: QuestionEvaluation["verdict"] = "correct"): QuestionEvaluation {
  return { gradingPolicyVersion: 1, verdict, matchedCount: 1, requiredCount: 1, components, explanation: rich("Overall reason") };
}
function showFeedback(question: PublicQuestion, response: QuestionResponse, result: QuestionEvaluation) {
  return render(<QuestionBody question={question} response={response} evaluation={result} assets={[]}
    disabled={false} onChange={vi.fn()} onError={vi.fn()} />);
}
beforeEach(() => { vi.restoreAllMocks(); });

describe("interactive input and inline feedback", () => {
  it("submits an explicit empty select-all answer without inventing an unanswered state", async () => {
    const { attempt, onCheck, onChecked } = mount({ kind: "select_all", prompt: rich("Choose"), options: [item("a", "Alpha"), item("b", "Beta")] });
    const checked: QuestionAttemptView = { ...attempt, revision: 2, progress: {
      phase: "checked", checkId: id("e"), firstAnswer: { schemaVersion: 1, instanceId: attempt.instanceId, response: { kind: "select_all", selectedOptionIds: [] } },
      responseTimeMs: 25, checkedAtMs: 100, evaluation: evaluation([]),
    } };
    onCheck.mockResolvedValue(checked);
    fireEvent.click(screen.getByRole("button", { name: "Check answer" }));
    await waitFor(() => expect(onChecked).toHaveBeenCalledWith(checked));
    expect(required(onCheck.mock.calls[0]?.[0], "check input").request.answer.response).toEqual({ kind: "select_all", selectedOptionIds: [] });
  });

  it("uses keycaps for T/F and persists false, ignoring IME input", async () => {
    const { onSave } = mount({ kind: "true_false", statement: rich("A statement") });
    expect(screen.getByText("T").tagName).toBe("KBD");
    expect(screen.getByText("F").tagName).toBe("KBD");
    expect(screen.queryByText(/Keyboard: T/)).toBeNull();
    fireEvent.keyDown(window, { key: "t", isComposing: true });
    expect(onSave).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: "f" });
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(required(onSave.mock.calls[0]?.[0], "save input").request.answer.response).toEqual({ kind: "true_false", selected: false });
  });

  it("matches by selecting whole tiles without a separate drag button", async () => {
    const { onSave } = mount(matching);
    fireEvent.click(screen.getByRole("button", { name: "Select Left A" }));
    fireEvent.click(screen.getByRole("button", { name: "Match with Right Y" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(required(onSave.mock.calls[0]?.[0], "save input").request.answer.response).toEqual({ kind: "matching", assignments: [{ itemId: id("a"), targetId: id("y") }] });
    expect(screen.queryByText(/Drag \/ select/)).toBeNull();
    expect(screen.getByRole("button", { name: "Unlink Left A" })).toBeTruthy();
  });

  it("drags the matching tile body onto an opposite-column match", async () => {
    const { onSave } = mount(matching);
    drag(screen.getByRole("button", { name: "Select Left A" }), screen.getByRole("button", { name: "Match with Right X" }));
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    expect(required(onSave.mock.calls.at(-1)?.[0], "latest save input").request.answer.response.assignments).toEqual([{ itemId: id("a"), targetId: id("x") }]);
  });

  it("sorts onto a same-column row and keeps an existing connection in place", async () => {
    const { onSave } = mount(matching);
    drag(screen.getByRole("button", { name: "Select Left A" }), screen.getByRole("button", { name: "Match with Right Y" }), "mouse");
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByRole("button", { name: "Select Left B" })).toHaveProperty("disabled", false));
    drag(screen.getByRole("button", { name: "Select Left B" }), screen.getByRole("button", { name: "Select Left C" }), "pen");
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(2));
    expect(required(onSave.mock.calls.at(-1)?.[0], "latest save input").request.answer.response.assignments).toEqual([
      { itemId: id("a"), targetId: id("y") }, { itemId: id("b"), targetId: id("z") },
    ]);
  });

  it("names each tile and target by what it says and reads a tile's status as its description", async () => {
    const { onSave } = mount(matching);
    const tile = screen.getByRole("button", { name: "Select Left A" });
    expect(tile.tagName).toBe("BUTTON");
    expect(screen.getByRole("button", { name: "Match with Right Y" }).tagName).toBe("BUTTON");
    fireEvent.click(tile);
    expect(tile.getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByRole("button", { name: "Match with Right Y" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(screen.getByRole("button", { name: "Select Left A", description: "Connected" })).toBeTruthy();
  });

  it("names a tile without words by its place", () => {
    mount({ ...matching, items: [{ id: id("a"), content: { type: "doc", content: [{ type: "paragraph" }] } }, ...matching.items.slice(1)] });
    expect(screen.getByRole("button", { name: "Select item 1" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Select Left B" })).toBeTruthy();
  });

  it("will not silently displace a connected pair", () => {
    const connected = [{ itemId: "a", targetId: "y" }];
    expect(matchingRows(["a", "b", "c"], ["x", "y", "z"], connected)).toEqual(["b", "a", "c"]);
    expect(connectMatching(connected, "b", "y")).toBeNull();
    expect(connectMatching(connected, "a", "z")).toBeNull();
  });

  it("grouping tiles disappear into their group, with a way to inspect and return them", async () => {
    const { onSave } = mount(grouping);
    fireEvent.click(screen.getByRole("button", { name: "Select Dachshund" }));
    fireEvent.click(screen.getByRole("button", { name: "Dogs (0 items)" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("button", { name: "Select Dachshund" })).toBeNull();
    expect(screen.getByRole("button", { name: "Select Maine Coon" })).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("button", { name: "Dogs (1 items)" })).toHaveProperty("disabled", false));
    fireEvent.click(screen.getByRole("button", { name: "Dogs (1 items)" }));
    expect(screen.getByRole("button", { name: "Select Dachshund" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Return to items" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(2));
    expect(required(onSave.mock.calls.at(-1)?.[0], "latest save input").request.answer.response.assignments).toEqual([]);
  });

  it("uses whole-tile pen dragging for grouping and cancels without saving", async () => {
    const { onSave } = mount(grouping);
    const target = screen.getByRole("button", { name: "Dogs (0 items)" }).closest("[data-practice-drop-target-id]") as HTMLElement;
    drag(screen.getByRole("button", { name: "Select Dachshund" }), target, "pen", true);
    expect(onSave).not.toHaveBeenCalled();
    drag(screen.getByRole("button", { name: "Select Dachshund" }), target, "pen");
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(required(onSave.mock.calls[0]?.[0], "save input").request.answer.response.assignments).toEqual([{ itemId: id("a"), targetId: id("x") }]);
  });

  it("orders through arrow controls without losing any item", async () => {
    const { onSave } = mount({ kind: "ordering", prompt: rich("Order"), items: [item("a", "First"), item("b", "Second"), item("c", "Third")] });
    fireEvent.click(required(screen.getAllByRole("button", { name: "Move down" })[0], "indexed role result"));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(required(onSave.mock.calls[0]?.[0], "save input").request.answer.response.orderedItemIds).toEqual([id("b"), id("a"), id("c")]);
  });

  it("orders by dragging the body of a tile", async () => {
    const { onSave } = mount({ kind: "ordering", prompt: rich("Order"), items: [item("a", "First"), item("b", "Second"), item("c", "Third")] });
    drag(screen.getByRole("button", { name: "Move Third" }), screen.getByRole("button", { name: "Move First" }));
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(required(onSave.mock.calls[0]?.[0], "save input").request.answer.response.orderedItemIds).toEqual([id("c"), id("a"), id("b")]);
  });

  for (const kind of ["single_choice", "select_all"] as const) it(`${kind} colors options by their keys and attaches rationales to the same option`, () => {
    const question = { kind, prompt: rich("Choose"), options: [item("b", "Beta"), item("a", "Alpha")] };
    const response: QuestionResponse = kind === "single_choice" ? { kind, selectedOptionId: id("a") } : { kind, selectedOptionIds: [id("a")] };
    const { container } = showFeedback(question, response, evaluation([
      { kind: "choice", itemId: id("a"), selected: true, expected: true, rationale: rich("Alpha reason") },
      { kind: "choice", itemId: id("b"), selected: false, expected: false, rationale: rich("Beta reason") },
    ]));
    const alpha = container.querySelector(`[data-choice-id="${id("a")}"]`)!;
    const beta = container.querySelector(`[data-choice-id="${id("b")}"]`)!;
    expect(alpha.closest("label")?.className).toContain("tw:bg-success-soft");
    expect(beta.closest("label")?.className).toContain("tw:bg-destructive-soft");
    expect(within(alpha as HTMLElement).getByText("Alpha reason")).toBeTruthy();
    expect(within(beta as HTMLElement).getByText("Beta reason")).toBeTruthy();
    expect(container.querySelector("[data-question-result]")?.getAttribute("data-question-result")).toBe("correct");
    expect(screen.queryByText("Correctly left unselected")).toBeNull();
  });

  it("true/false shows red/green options and a red question for an incorrect response", () => {
    const { container } = showFeedback({ kind: "true_false", statement: rich("Statement") }, { kind: "true_false", selected: false },
      evaluation([{ kind: "boolean", selected: false, expected: true, correctedStatement: null }], "incorrect"));
    expect(container.querySelector('[data-choice-id="true"]')?.closest("label")?.className).toContain("tw:bg-success-soft");
    expect(container.querySelector('[data-choice-id="false"]')?.closest("label")?.className).toContain("tw:bg-destructive-soft");
    expect(container.querySelector("[data-question-result]")?.className).toContain("tw:bg-destructive-soft");
    expect(screen.queryByText("Correct true/false choice")).toBeNull();
  });

  it("matching marks the original arrangement and shows inline correction arrows rather than a placement table", () => {
    const { container } = showFeedback(matching, { kind: "matching", assignments: [{ itemId: id("a"), targetId: id("x") }] },
      evaluation([{ kind: "placement", itemId: id("a"), selectedTargetId: id("x"), expectedTargetId: id("y"), rationale: rich("Pair reason") }], "incorrect"));
    const tile = container.querySelector(`[data-question-item-id="${id("a")}"]`)!;
    expect(tile.className).toContain("tw:bg-destructive-soft");
    expect(within(tile as HTMLElement).getByText("Right Y")).toBeTruthy();
    expect(within(tile as HTMLElement).getByText("→ Correct match:")).toBeTruthy();
    expect(screen.queryByText("Correct placement")).toBeNull();
  });

  it("leaves a checked board as readable content without tile buttons", () => {
    showFeedback(matching, { kind: "matching", assignments: [{ itemId: id("a"), targetId: id("x") }] },
      evaluation([{ kind: "placement", itemId: id("a"), selectedTargetId: id("x"), expectedTargetId: id("y"), rationale: null }], "incorrect"));
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(screen.getByText("Left A")).toBeTruthy();
    expect(screen.getByText("Incorrect match")).toBeTruthy();
  });

  it("grouping reveals all placed items with per-item results in their chosen groups", () => {
    const { container } = showFeedback(grouping, { kind: "grouping", assignments: [{ itemId: id("a"), targetId: id("x") }, { itemId: id("b"), targetId: id("x") }] },
      evaluation([
        { kind: "placement", itemId: id("a"), selectedTargetId: id("x"), expectedTargetId: id("x"), rationale: null },
        { kind: "placement", itemId: id("b"), selectedTargetId: id("x"), expectedTargetId: id("y"), rationale: null },
      ], "incorrect"));
    const first = container.querySelector(`[data-question-item-id="${id("a")}"]`)!;
    const second = container.querySelector(`[data-question-item-id="${id("b")}"]`)!;
    expect(first.className).toContain("tw:bg-success-soft");
    expect(second.className).toContain("tw:bg-destructive-soft");
    expect(second.closest("[data-practice-drop-target-id]")?.getAttribute("data-practice-drop-target-id")).toBe(id("x"));
    expect(within(second as HTMLElement).getByText("Cats")).toBeTruthy();
  });
});
