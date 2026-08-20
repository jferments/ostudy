import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CardPreviewRow } from "./CardPreviewRow";

const card = {
  noteId: "a".repeat(32),
  questionExcerpt: "A complete question that should be constrained by layout rather than JavaScript truncation.",
  answerExcerpt: "A complete answer that remains available to assistive technology.",
  imageCount: 0,
  generatedCardCount: 1,
  reversible: false,
  primaryCardId: "b".repeat(32),
  updatedAt: 1,
};

describe("CardPreviewRow", () => {
  it("renders compact Q/A lines without an arrow or arbitrary substring truncation", () => {
    render(<CardPreviewRow card={card} onEdit={vi.fn()} onDelete={vi.fn()} />);
    expect(screen.getByText("Q:")).toBeTruthy();
    expect(screen.getByText("A:")).toBeTruthy();
    expect(screen.queryByText("→")).toBeNull();
    expect(screen.getByText(card.questionExcerpt)).toBeTruthy();
    expect(screen.getByText(card.answerExcerpt)).toBeTruthy();
  });

  it("keeps edit and delete operations accessible", () => {
    const onEdit = vi.fn();
    const onDelete = vi.fn();
    render(<CardPreviewRow card={card} onEdit={onEdit} onDelete={onDelete} />);
    fireEvent.click(screen.getByRole("button", { name: /edit flashcard/i }));
    fireEvent.click(screen.getAllByRole("button", { name: "Delete" })[0]);
    expect(onEdit).toHaveBeenCalledOnce();
    expect(onDelete).toHaveBeenCalledOnce();
  });
});
