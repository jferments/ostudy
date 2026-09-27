import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PublicQuestion } from "../../ipc/questionAttempts";
import { requiredTestValue as required } from "../../test/factories";
import { MatchingBoard } from "./MatchingBoard";
import type { Placement } from "./questionPresentation";

vi.mock("../../editor/RichTextView", () => ({
  RichTextView: ({ document }: { document: { content: Array<{ content: Array<{ text: string }> }> } }) =>
    <span>{document.content.flatMap((node) => node.content).map((node) => node.text).join(" ")}</span>,
}));
const rich = (text: string) => ({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] });
const item = (id: string) => ({ id, content: rich(id) });
const question: Extract<PublicQuestion, { kind: "matching" }> = {
  kind: "matching", prompt: rich("Match"),
  items: ["A", "B", "C", "D"].map(item), targets: ["X", "Y", "Z", "W"].map(item),
};
const originalHitStack = Object.getOwnPropertyDescriptor(document, "elementsFromPoint");
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  if (originalHitStack) Object.defineProperty(document, "elementsFromPoint", originalHitStack);
  else Reflect.deleteProperty(document, "elementsFromPoint");
});

function capture(tile: HTMLElement): void {
  for (const name of ["setPointerCapture", "releasePointerCapture", "hasPointerCapture"]) {
    Object.defineProperty(tile, name, { configurable: true, value: vi.fn(() => true) });
  }
}
function hitStack(elements: Element[]): void {
  Object.defineProperty(document, "elementsFromPoint", { configurable: true, value: vi.fn(() => elements) });
}
function begin(tile: HTMLElement, pointerType = "touch"): void {
  capture(tile);
  fireEvent.pointerDown(tile, { pointerId: 7, pointerType, button: 0, clientX: 1, clientY: 1 });
  fireEvent.pointerMove(tile, { pointerId: 7, pointerType, clientX: 80, clientY: 80 });
}
function finish(tile: HTMLElement): void {
  fireEvent.pointerUp(tile, { pointerId: 7, pointerType: "touch", clientX: 80, clientY: 80 });
}
function Harness({ disabled = false, currentQuestion = question, onChange = () => undefined }: {
  disabled?: boolean; currentQuestion?: typeof question; onChange?: (value: Placement[]) => void;
}) {
  const [assignments, setAssignments] = useState<Placement[]>([]);
  return <MatchingBoard question={currentQuestion} assignments={assignments} assets={[]}
    disabled={disabled} evaluation={null} onError={vi.fn()}
    onChange={(value) => { setAssignments(value); onChange(value); }} />;
}
function rows(container: HTMLElement): string[] {
  return [...container.querySelectorAll("[data-match-target]")].map((target) =>
    required(target.parentElement?.querySelector<HTMLElement>("[data-question-item-id]"), "left row tile").dataset.questionItemId ?? "");
}

describe("matching gesture stability", () => {
  it("swaps only the dropped rows, and unlink does not reshuffle unrelated loose tiles", () => {
    const changed = vi.fn();
    const { container } = render(<Harness onChange={changed} />);
    const tile = screen.getByRole("button", { name: "Select A" });
    const target = screen.getByRole("button", { name: "Match with W" });
    hitStack([target]); begin(tile); finish(tile);
    expect(rows(container)).toEqual(["D", "B", "C", "A"]);
    expect(changed).toHaveBeenLastCalledWith([{ itemId: "A", targetId: "W" }]);
    fireEvent.click(screen.getByRole("button", { name: "Unlink A" }));
    expect(rows(container)).toEqual(["D", "B", "C", "A"]);
    expect(changed).toHaveBeenLastCalledWith([]);
  });

  it("looks underneath the captured moving tile and its rich descendants for the actual target", () => {
    const changed = vi.fn();
    render(<Harness onChange={changed} />);
    const tile = screen.getByRole("button", { name: "Select A" });
    const target = screen.getByRole("button", { name: "Match with Z" });
    hitStack([required(tile.firstElementChild, "dragged rich content"), tile,
      required(target.firstElementChild, "target rich content"), target]);
    begin(tile, "mouse"); finish(tile);
    expect(changed).toHaveBeenCalledTimes(1);
    expect(changed).toHaveBeenCalledWith([{ itemId: "A", targetId: "Z" }]);
  });

  it("accepts the row gutter as a target without treating another board as this question", () => {
    const changed = vi.fn();
    const { container } = render(<Harness onChange={changed} />);
    const tile = screen.getByRole("button", { name: "Select A" });
    const target = required(container.querySelector('[data-match-target="Y"]')?.parentElement, "matching row");
    hitStack([target]); begin(tile); finish(tile);
    expect(changed).toHaveBeenLastCalledWith([{ itemId: "A", targetId: "Y" }]);
    const otherBoard = document.createElement("div");
    otherBoard.dataset.practiceDropTargetId = "Z";
    document.body.append(otherBoard);
    try {
      const nextTile = screen.getByRole("button", { name: "Select B" });
      hitStack([otherBoard]); begin(nextTile); finish(nextTile);
      expect(changed).toHaveBeenCalledTimes(1);
    } finally { otherBoard.remove(); }
  });

  it("suppresses delayed touch clicks after a drop, while Enter and Space remain usable", () => {
    vi.useFakeTimers();
    const changed = vi.fn();
    render(<Harness onChange={changed} />);
    const tile = screen.getByRole("button", { name: "Select A" });
    const target = screen.getByRole("button", { name: "Match with W" });
    hitStack([target]); begin(tile); finish(tile);
    act(() => { vi.runOnlyPendingTimers(); });
    const afterDrop = screen.getByRole("button", { name: "Select A" });
    fireEvent.click(afterDrop);
    expect(afterDrop.getAttribute("aria-pressed")).toBe("false");
    fireEvent.keyDown(afterDrop, { key: "Enter" });
    expect(afterDrop.getAttribute("aria-pressed")).toBe("true");
    fireEvent.keyDown(screen.getByRole("button", { name: "Match with X" }), { key: " " });
    expect(changed).toHaveBeenLastCalledWith([{ itemId: "A", targetId: "X" }]);
  });

  for (const interruption of ["cancel", "lost-capture", "escape", "blur"] as const) {
    it(`${interruption} retires the gesture without applying a late pointer-up`, () => {
      const changed = vi.fn();
      render(<Harness onChange={changed} />);
      const tile = screen.getByRole("button", { name: "Select A" });
      hitStack([screen.getByRole("button", { name: "Match with Z" })]);
      begin(tile);
      if (interruption === "cancel") fireEvent.pointerCancel(tile, { pointerId: 7 });
      else if (interruption === "lost-capture") fireEvent.lostPointerCapture(tile, { pointerId: 7 });
      else if (interruption === "escape") fireEvent.keyDown(window, { key: "Escape" });
      else fireEvent.blur(window);
      finish(tile);
      expect(changed).not.toHaveBeenCalled();
    });
  }

  it("disabled or replacement-question state rejects an already-started drag", () => {
    const changed = vi.fn();
    const view = render(<Harness onChange={changed} />);
    const tile = screen.getByRole("button", { name: "Select A" });
    hitStack([screen.getByRole("button", { name: "Match with Z" })]); begin(tile);
    view.rerender(<Harness disabled onChange={changed} />); finish(tile);
    expect(changed).not.toHaveBeenCalled();
    view.rerender(<Harness onChange={changed} />);
    const enabled = screen.getByRole("button", { name: "Select A" }); begin(enabled);
    view.rerender(<Harness currentQuestion={{ ...question, items: ["E", "F", "G", "H"].map(item) }} onChange={changed} />);
    finish(enabled);
    expect(changed).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Select A" })).toBeNull();
  });
});
