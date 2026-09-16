import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { QuestionAttemptView } from "../../ipc/questionAttempts";
import { InteractiveQuestionActivity } from "./InteractiveQuestionActivity";

vi.mock("../../editor/RichTextView", () => ({
  RichTextView: ({ document }: { document: { content?: Array<{ content?: Array<{ text?: string }> }> } }) => (
    <span>{document.content?.flatMap((node) => node.content ?? []).map((node) => node.text ?? "").join(" ")}</span>
  ),
}));

const id = (value: string) => value.repeat(32).slice(0, 32);
const rich = (text: string) => ({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] });

function baseAttempt(question: QuestionAttemptView["question"]): QuestionAttemptView {
  return {
    instanceId: id("i"),
    revision: 1,
    assisted: false,
    question,
    progress: { phase: "answering", savedAnswer: null },
  };
}

function savedFrom(attempt: QuestionAttemptView, input: { request: { answer: unknown } }): QuestionAttemptView {
  return {
    ...attempt,
    revision: 2,
    progress: {
      phase: "answering",
      savedAnswer: input.request.answer as Extract<QuestionAttemptView["progress"], { phase: "answering" }>["savedAnswer"],
    },
  };
}

function supportPointerCapture(handle: HTMLElement): void {
  if (!("setPointerCapture" in handle)) Object.defineProperty(handle, "setPointerCapture", { value: vi.fn() });
  if (!("releasePointerCapture" in handle)) Object.defineProperty(handle, "releasePointerCapture", { value: vi.fn() });
  if (!("hasPointerCapture" in handle)) Object.defineProperty(handle, "hasPointerCapture", { value: vi.fn().mockReturnValue(true) });
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("InteractiveQuestionActivity", () => {
  it("submits an explicit empty select-all answer without inventing an unanswered state", async () => {
    const attempt = baseAttempt({
      kind: "select_all",
      prompt: rich("Choose every correct answer"),
      options: [
        { id: id("a"), content: rich("Alpha") },
        { id: id("b"), content: rich("Beta") },
      ],
    });
    const checked: QuestionAttemptView = {
      ...attempt,
      revision: 2,
      progress: {
        phase: "checked",
        checkId: id("c"),
        firstAnswer: { schemaVersion: 1, instanceId: attempt.instanceId, response: { kind: "select_all", selectedOptionIds: [] } },
        responseTimeMs: 25,
        checkedAtMs: 100,
        evaluation: { gradingPolicyVersion: 1, verdict: "correct", matchedCount: 0, requiredCount: 0, components: [], explanation: null },
      },
    };
    const onCheck = vi.fn().mockResolvedValue(checked);
    const onChecked = vi.fn();
    render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
      disabled={false} onSave={vi.fn()} onCheck={onCheck} onAttempt={vi.fn()} onChecked={onChecked} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Check answer" }));
    await waitFor(() => expect(onCheck).toHaveBeenCalledTimes(1));
    expect(onCheck.mock.calls[0][0].request.answer.response).toEqual({ kind: "select_all", selectedOptionIds: [] });
    await waitFor(() => expect(onChecked).toHaveBeenCalledWith(checked));
  });

  it("supports the T/F keyboard shortcut and persists false as a deliberate answer", async () => {
    const attempt = baseAttempt({ kind: "true_false", statement: rich("The statement is false") });
    const saved = {
      ...attempt,
      revision: 2,
      progress: {
        phase: "answering" as const,
        savedAnswer: { schemaVersion: 1 as const, instanceId: attempt.instanceId, response: { kind: "true_false" as const, selected: false } },
      },
    };
    const onSave = vi.fn().mockResolvedValue(saved);
    render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
      disabled={false} onSave={onSave} onCheck={vi.fn()} onAttempt={vi.fn()} onChecked={vi.fn()} onError={vi.fn()} />);
    fireEvent.keyDown(window, { key: "f" });
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0].request.answer.response).toEqual({ kind: "true_false", selected: false });
  });

  it("offers matching without dragging by selecting an item and a target", async () => {
    const attempt = baseAttempt({
      kind: "matching",
      prompt: rich("Match the pairs"),
      items: [
        { id: id("a"), content: rich("Left A") },
        { id: id("b"), content: rich("Left B") },
      ],
      targets: [
        { id: id("x"), content: rich("Right X") },
        { id: id("y"), content: rich("Right Y") },
      ],
    });
    const onSave = vi.fn().mockImplementation(async (input) => savedFrom(attempt, input));
    render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
      disabled={false} onSave={onSave} onCheck={vi.fn()} onAttempt={vi.fn()} onChecked={vi.fn()} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Drag / select item 1" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Match here" })[0]);
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0].request.answer.response).toEqual({
      kind: "matching",
      assignments: [{ itemId: id("a"), targetId: id("x") }],
    });
  });

  it("uses the shared Pointer Events adapter for matching touch and pen placement", async () => {
    const attempt = baseAttempt({
      kind: "matching",
      prompt: rich("Match"),
      items: [{ id: id("a"), content: rich("Left") }, { id: id("b"), content: rich("Other") }],
      targets: [{ id: id("x"), content: rich("Right") }, { id: id("y"), content: rich("Other right") }],
    });
    const onSave = vi.fn().mockImplementation(async (input) => savedFrom(attempt, input));
    render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
      disabled={false} onSave={onSave} onCheck={vi.fn()} onAttempt={vi.fn()} onChecked={vi.fn()} onError={vi.fn()} />);
    const handle = screen.getByRole("button", { name: "Drag / select item 1" });
    const target = screen.getAllByText("Right")[0].closest("[data-practice-drop-target-id]") as HTMLElement;
    Object.defineProperty(document, "elementFromPoint", { configurable: true, value: vi.fn(() => target) });
    supportPointerCapture(handle);
    fireEvent.pointerDown(handle, { pointerId: 7, button: 0, clientX: 1, clientY: 1, pointerType: "touch" });
    fireEvent.pointerMove(handle, { pointerId: 7, clientX: 20, clientY: 20, pointerType: "touch" });
    fireEvent.pointerUp(handle, { pointerId: 7, clientX: 20, clientY: 20, pointerType: "touch" });
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    expect(onSave.mock.calls.at(-1)?.[0].request.answer.response).toEqual({
      kind: "matching",
      assignments: [{ itemId: id("a"), targetId: id("x") }],
    });
  });

  it("offers grouping without dragging by selecting an item and a category", async () => {
    const attempt = baseAttempt({
      kind: "grouping",
      prompt: rich("Sort the findings"),
      items: [{ id: id("a"), content: rich("Shivering") }, { id: id("b"), content: rich("Sweating") }],
      categories: [{ id: id("x"), content: rich("Cold") }, { id: id("y"), content: rich("Hot") }],
    });
    const onSave = vi.fn().mockImplementation(async (input) => savedFrom(attempt, input));
    render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
      disabled={false} onSave={onSave} onCheck={vi.fn()} onAttempt={vi.fn()} onChecked={vi.fn()} onError={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Drag / select item 1" }));
    fireEvent.click(screen.getAllByRole("button", { name: "Place selected item here" })[0]);
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0].request.answer.response).toEqual({
      kind: "grouping",
      assignments: [{ itemId: id("a"), targetId: id("x") }],
    });
  });

  it("uses the same Pointer Events drop adapter for grouping", async () => {
    const attempt = baseAttempt({
      kind: "grouping",
      prompt: rich("Group"),
      items: [{ id: id("a"), content: rich("Item A") }, { id: id("b"), content: rich("Item B") }],
      categories: [{ id: id("x"), content: rich("Group X") }, { id: id("y"), content: rich("Group Y") }],
    });
    const onSave = vi.fn().mockImplementation(async (input) => savedFrom(attempt, input));
    render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
      disabled={false} onSave={onSave} onCheck={vi.fn()} onAttempt={vi.fn()} onChecked={vi.fn()} onError={vi.fn()} />);
    const handle = screen.getByRole("button", { name: "Drag / select item 1" });
    const target = screen.getByText("Group X").closest("[data-practice-drop-target-id]") as HTMLElement;
    Object.defineProperty(document, "elementFromPoint", { configurable: true, value: vi.fn(() => target) });
    supportPointerCapture(handle);
    fireEvent.pointerDown(handle, { pointerId: 8, button: 0, clientX: 1, clientY: 1, pointerType: "pen" });
    fireEvent.pointerMove(handle, { pointerId: 8, clientX: 20, clientY: 20, pointerType: "pen" });
    fireEvent.pointerUp(handle, { pointerId: 8, clientX: 20, clientY: 20, pointerType: "pen" });
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    expect(onSave.mock.calls.at(-1)?.[0].request.answer.response).toEqual({
      kind: "grouping",
      assignments: [{ itemId: id("a"), targetId: id("x") }],
    });
  });

  it("orders without dragging through explicit move controls while preserving a full permutation", async () => {
    const attempt = baseAttempt({
      kind: "ordering",
      prompt: rich("Order the steps"),
      items: [
        { id: id("a"), content: rich("First") },
        { id: id("b"), content: rich("Second") },
        { id: id("c"), content: rich("Third") },
      ],
    });
    const onSave = vi.fn().mockImplementation(async (input) => savedFrom(attempt, input));
    render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
      disabled={false} onSave={onSave} onCheck={vi.fn()} onAttempt={vi.fn()} onChecked={vi.fn()} onError={vi.fn()} />);
    fireEvent.click(screen.getAllByRole("button", { name: "Move down" })[0]);
    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0].request.answer.response).toEqual({
      kind: "ordering",
      orderedItemIds: [id("b"), id("a"), id("c")],
    });
  });

  it("reorders by Pointer Events drag without revealing the canonical answer", async () => {
    const attempt = baseAttempt({
      kind: "ordering",
      prompt: rich("Order"),
      items: [
        { id: id("a"), content: rich("First") },
        { id: id("b"), content: rich("Second") },
        { id: id("c"), content: rich("Third") },
      ],
    });
    const onSave = vi.fn().mockImplementation(async (input) => savedFrom(attempt, input));
    render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
      disabled={false} onSave={onSave} onCheck={vi.fn()} onAttempt={vi.fn()} onChecked={vi.fn()} onError={vi.fn()} />);
    const handle = screen.getByText("Drag item 3");
    const target = screen.getByText("First").closest("[data-practice-drop-target-id]") as HTMLElement;
    Object.defineProperty(document, "elementFromPoint", { configurable: true, value: vi.fn(() => target) });
    supportPointerCapture(handle);
    fireEvent.pointerDown(handle, { pointerId: 9, button: 0, clientX: 1, clientY: 1, pointerType: "touch" });
    fireEvent.pointerMove(handle, { pointerId: 9, clientX: 20, clientY: 20, pointerType: "touch" });
    fireEvent.pointerUp(handle, { pointerId: 9, clientX: 20, clientY: 20, pointerType: "touch" });
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    expect(onSave.mock.calls.at(-1)?.[0].request.answer.response).toEqual({
      kind: "ordering",
      orderedItemIds: [id("c"), id("a"), id("b")],
    });
  });
});
