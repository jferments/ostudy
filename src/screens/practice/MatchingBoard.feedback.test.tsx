import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PublicQuestion } from "../../ipc/questionAttempts";
import { requiredTestValue as required } from "../../test/factories";
import { MatchingBoard } from "./MatchingBoard";

vi.mock("../../editor/RichTextView", () => ({
  RichTextView: ({ document }: { document: { content: Array<{ content: Array<{ text: string }> }> } }) =>
    <span>{document.content.flatMap((node) => node.content).map((node) => node.text).join(" ")}</span>,
}));
const rich = (text: string) => ({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] });
const question: Extract<PublicQuestion, { kind: "matching" }> = {
  kind: "matching", prompt: rich("Match"),
  items: ["A", "B"].map((id) => ({ id, content: rich(id) })),
  targets: ["X", "Y"].map((id) => ({ id, content: rich(id) })),
};
const originalHitStack = Object.getOwnPropertyDescriptor(document, "elementsFromPoint");
afterEach(() => {
  vi.restoreAllMocks();
  window.getSelection()?.removeAllRanges();
  if (originalHitStack) Object.defineProperty(document, "elementsFromPoint", originalHitStack);
  else Reflect.deleteProperty(document, "elementsFromPoint");
});

/** The press lands on a tile's button; the tile itself captures the pointer and owns the drag. */
function setup() {
  const changed = vi.fn();
  const view = render(<MatchingBoard question={question} assignments={[]} assets={[]} disabled={false}
    evaluation={null} onChange={changed} onError={vi.fn()} />);
  const handle = screen.getByRole("button", { name: "Select A" });
  const tile = required(handle.closest<HTMLElement>("[data-question-item-id]"), "dragged tile");
  const target = required(screen.getByRole("button", { name: "Match with Y" }).closest<HTMLElement>("[data-match-target]"), "target tile");
  const capture = vi.fn();
  Object.defineProperties(tile, {
    setPointerCapture: { configurable: true, value: capture },
    hasPointerCapture: { configurable: true, value: () => true },
    releasePointerCapture: { configurable: true, value: vi.fn() },
  });
  Object.defineProperty(document, "elementsFromPoint", { configurable: true, value: () => [target] });
  return { ...view, handle, tile, target, capture, changed };
}
const down = (node: Element) => fireEvent.pointerDown(node, {
  pointerId: 7, pointerType: "mouse", button: 0, clientX: 5, clientY: 5,
});
const move = (node: Element) => fireEvent.pointerMove(node, {
  pointerId: 7, pointerType: "mouse", clientX: 90, clientY: 90,
});
function selectText(node: Node): void {
  const range = document.createRange();
  range.selectNodeContents(node);
  const selection = required(window.getSelection(), "browser selection");
  selection.removeAllRanges();
  selection.addRange(range);
}

describe("matching drag feedback ownership", () => {
  it("prevents native selection at pointer-down and exposes active source/destination until cancellation", () => {
    const { handle, tile, target, container, changed } = setup();
    expect(down(within(tile).getByText("A"))).toBe(false);
    expect(document.activeElement).toBe(handle);
    expect(tile.dataset.dragging).toBeUndefined();
    move(tile);
    expect(tile.dataset.dragging).toBe("true");
    expect(target.dataset.dropActive).toBe("true");
    expect(container.querySelector('[data-matching-drag-active="true"]')).not.toBeNull();
    fireEvent.pointerCancel(tile, { pointerId: 7 });
    expect(tile.dataset.dragging).toBeUndefined();
    expect(target.dataset.dropActive).toBeUndefined();
    expect(container.querySelector('[data-matching-drag-active="true"]')).toBeNull();
    expect(changed).not.toHaveBeenCalled();
  });

  it("clears an existing board selection only when a drag actually starts", () => {
    const { handle, tile, target } = setup();
    selectText(within(target).getByText("Y"));
    expect(window.getSelection()?.toString()).toBe("Y");
    down(handle);
    expect(window.getSelection()?.toString()).toBe("Y");
    move(tile);
    expect(window.getSelection()?.toString()).toBe("");
  });

  it("does not clear unrelated page selections", () => {
    const { tile } = setup();
    const external = document.createElement("p");
    external.textContent = "Retain unrelated selected text";
    document.body.append(external);
    try {
      selectText(external);
      down(screen.getByRole("button", { name: "Select A" })); move(tile);
      expect(window.getSelection()?.toString()).toBe(external.textContent);
    } finally { external.remove(); }
  });

  it("leaves embedded interactive content out of the drag gesture", () => {
    const { tile, capture } = setup();
    const link = document.createElement("a");
    link.href = "#reference";
    link.textContent = "Reference";
    within(tile).getByText("A").append(link);
    expect(down(link)).toBe(true);
    expect(capture).not.toHaveBeenCalled();
    move(tile);
    expect(tile.dataset.dragging).toBeUndefined();
  });
});
