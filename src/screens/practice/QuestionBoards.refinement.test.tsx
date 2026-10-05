import { fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import type { PublicQuestion } from "../../ipc/questionAttempts";
import { GroupingBoard } from "./GroupingBoard";
import { MatchingBoard } from "./MatchingBoard";
import { matchingRows, rematchMatching, type Placement } from "./questionPresentation";

vi.mock("../../editor/RichTextView", () => ({ RichTextView: ({ document }: { document: { text: string } }) => <span>{document.text}</span> }));
const item = (id: string) => ({ id, content: { text: id } });
const matching: Extract<PublicQuestion, { kind: "matching" }> = {
  kind: "matching", prompt: { text: "Match" }, items: [item("A"), item("B"), item("C")], targets: [item("X"), item("Y"), item("Z")],
};
const initial = [{ itemId: "A", targetId: "X" }, { itemId: "B", targetId: "Y" }, { itemId: "C", targetId: "Z" }];
function Harness() {
  const [assignments, setAssignments] = useState<Placement[]>(initial);
  return <MatchingBoard question={matching} assignments={assignments} assets={[]} disabled={false}
    evaluation={null} onChange={setAssignments} onError={vi.fn()} />;
}
function drag(tile: HTMLElement, target: HTMLElement) {
  for (const name of ["setPointerCapture", "releasePointerCapture", "hasPointerCapture"]) {
    Object.defineProperty(tile, name, { configurable: true, value: vi.fn(() => true) });
  }
  Object.defineProperty(document, "elementFromPoint", { configurable: true, value: vi.fn(() => target) });
  fireEvent.pointerDown(tile, { pointerId: 1, button: 0, clientX: 1, clientY: 1, pointerType: "touch" });
  fireEvent.pointerMove(tile, { pointerId: 1, clientX: 30, clientY: 30, pointerType: "touch" });
  fireEvent.pointerUp(tile, { pointerId: 1, clientX: 30, clientY: 30, pointerType: "touch" });
}

describe("clear category targets and editable connected matches", () => {
  it("groups have their own headings and drop areas while unplaced items stay neutral and outside them", () => {
    const question: Extract<PublicQuestion, { kind: "grouping" }> = {
      kind: "grouping", prompt: { text: "Group" }, categories: [item("Dogs"), item("Cats")], items: [item("Dachshund"), item("Siamese")],
    };
    const { container } = render(<GroupingBoard question={question} assets={[]} assignments={[]} disabled={false}
      evaluation={null} onChange={vi.fn()} onError={vi.fn()} />);
    const categories = [...container.querySelectorAll<HTMLElement>("[data-group-category]")];
    expect(categories.map((category) => category.dataset.practiceDropTargetId)).toEqual(["Dogs", "Cats"]);
    expect(screen.getAllByText("Group")).toHaveLength(2);
    for (const category of categories) expect(within(category).getByRole("button").getAttribute("aria-expanded")).toBe("false");
    const tiles = [...container.querySelectorAll<HTMLElement>("[data-question-item-id]")];
    expect(tiles.map((tile) => tile.dataset.questionItemId)).toEqual(["Dachshund", "Siamese"]);
    for (const tile of tiles) {
      expect(tile.closest("[data-group-category]")).toBeNull();
      expect(within(tile).getByRole("button").getAttribute("aria-pressed")).toBe("false");
      expect(tile.dataset.connected).toBeUndefined();
      expect(tile.dataset.result).toBeUndefined();
    }
  });

  it("connected pairs are visibly accented on both sides and can be dragged to rematch", () => {
    const { container } = render(<Harness />);
    const left = container.querySelector<HTMLElement>('[data-question-item-id="A"]')!;
    const right = container.querySelector<HTMLElement>('[data-match-target="Y"]')!;
    expect((within(left).getByRole("button") as HTMLButtonElement).disabled).toBe(false);
    expect(left.dataset.connected).toBe("true");
    expect(right.dataset.connected).toBe("true");
    expect(left.className).toContain("tw:bg-accent");
    expect(right.className).toContain("tw:bg-accent");
    expect(left.getAttribute("data-result")).toBeNull();
    drag(left, right);
    const rowY = container.querySelector('[data-match-target="Y"]')!.parentElement!;
    const rowX = container.querySelector('[data-match-target="X"]')!.parentElement!;
    const rowZ = container.querySelector('[data-match-target="Z"]')!.parentElement!;
    expect(rowY.querySelector('[data-question-item-id="A"]')).not.toBeNull();
    expect(rowX.querySelector('[data-question-item-id="B"]')).not.toBeNull();
    expect(rowZ.querySelector('[data-question-item-id="C"]')).not.toBeNull();
  });

  it("explicit rematching preserves a bijection and unrelated rows for every pair of destinations", () => {
    for (const itemId of ["A", "B", "C"]) for (const targetId of ["X", "Y", "Z"]) {
      const next = rematchMatching(initial, itemId, targetId);
      if (!next) { expect(initial.some((pair) => pair.itemId === itemId && pair.targetId === targetId)).toBe(true); continue; }
      expect(new Set(next.map((pair) => pair.itemId)).size).toBe(3);
      expect(new Set(next.map((pair) => pair.targetId)).size).toBe(3);
      expect(next.find((pair) => pair.itemId === itemId)?.targetId).toBe(targetId);
      for (const pair of initial.filter((entry) => entry.itemId !== itemId && entry.targetId !== targetId)) {
        expect(next).toContainEqual(pair);
      }
      expect(matchingRows(["A", "B", "C"], ["X", "Y", "Z"], next)).toHaveLength(3);
    }
    expect(initial).toEqual([{ itemId: "A", targetId: "X" }, { itemId: "B", targetId: "Y" }, { itemId: "C", targetId: "Z" }]);
  });

  it("moving a loose item onto a connection releases only that displaced item", () => {
    expect(rematchMatching(initial.slice(1), "A", "Y")).toEqual([
      { itemId: "C", targetId: "Z" }, { itemId: "A", targetId: "Y" },
    ]);
  });
});
