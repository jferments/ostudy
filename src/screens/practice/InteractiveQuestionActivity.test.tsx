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
    const onSave = vi.fn().mockImplementation(async (input) => ({
      ...attempt,
      revision: 2,
      progress: { phase: "answering", savedAnswer: input.request.answer },
    }));
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

  it("uses Pointer Events for drag placement so touch and pen do not depend on HTML drag events", async () => {
    const attempt = baseAttempt({
      kind: "matching",
      prompt: rich("Match"),
      items: [{ id: id("a"), content: rich("Left") }, { id: id("b"), content: rich("Other") }],
      targets: [{ id: id("x"), content: rich("Right") }, { id: id("y"), content: rich("Other right") }],
    });
    const onSave = vi.fn().mockImplementation(async (input) => ({
      ...attempt,
      revision: 2,
      progress: { phase: "answering", savedAnswer: input.request.answer },
    }));
    render(<InteractiveQuestionActivity sessionId={id("s")} cardId={id("d")} attempt={attempt} assets={[]}
      disabled={false} onSave={onSave} onCheck={vi.fn()} onAttempt={vi.fn()} onChecked={vi.fn()} onError={vi.fn()} />);
    const handle = screen.getByRole("button", { name: "Drag / select item 1" });
    const target = screen.getAllByText("Right")[0].closest("[data-match-target-id]") as HTMLElement;
    vi.spyOn(document, "elementFromPoint").mockReturnValue(target);
    if (!("setPointerCapture" in handle)) Object.defineProperty(handle, "setPointerCapture", { value: vi.fn() });
    if (!("releasePointerCapture" in handle)) Object.defineProperty(handle, "releasePointerCapture", { value: vi.fn() });
    if (!("hasPointerCapture" in handle)) Object.defineProperty(handle, "hasPointerCapture", { value: vi.fn().mockReturnValue(true) });
    fireEvent.pointerDown(handle, { pointerId: 7, button: 0, clientX: 1, clientY: 1, pointerType: "touch" });
    fireEvent.pointerMove(handle, { pointerId: 7, clientX: 20, clientY: 20, pointerType: "touch" });
    fireEvent.pointerUp(handle, { pointerId: 7, clientX: 20, clientY: 20, pointerType: "touch" });
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    expect(onSave.mock.calls.at(-1)?.[0].request.answer.response).toEqual({
      kind: "matching",
      assignments: [{ itemId: id("a"), targetId: id("x") }],
    });
  });
});
