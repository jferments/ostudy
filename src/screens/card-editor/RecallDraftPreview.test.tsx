import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { RichDocument } from "../../ipc/common";
import { RecallDraftPreview, type RecallPreviewPresentation } from "./RecallDraftPreview";

vi.mock("../practice/RecallCardSurface", () => ({
  RecallCardSurface: ({ presentationKey, revealed }: { presentationKey: string; revealed: boolean }) =>
    <div>{presentationKey}:{revealed ? "answer" : "question"}</div>,
}));

function doc(text: string): RichDocument {
  return { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] };
}

const presentations: RecallPreviewPresentation[] = [
  {
    presentationKey: "cloze:1", presentationLabel: "Blank group 1", cardLabel: "Fill in the blank",
    frontDoc: doc("First"), backDoc: doc("First answer"), frontText: "First", backText: "First answer", assets: [],
  },
  {
    presentationKey: "cloze:2", presentationLabel: "Blank group 2", cardLabel: "Fill in the blank",
    frontDoc: doc("Second"), backDoc: doc("Second answer"), frontText: "Second", backText: "Second answer", assets: [],
  },
];

describe("RecallDraftPreview", () => {
  it("selects transient presentations and resets reveal state when the presentation changes", () => {
    render(<RecallDraftPreview presentations={presentations} tags={["week-1"]} loading={false}
      disabled={false} onError={vi.fn()} />);
    expect(screen.getByText("cloze:1:question")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Reveal answer" }));
    expect(screen.getByText("cloze:1:answer")).toBeTruthy();
    fireEvent.change(screen.getByRole("combobox", { name: "Presentation" }), { target: { value: "cloze:2" } });
    expect(screen.getByText("cloze:2:question")).toBeTruthy();
    expect(screen.getByText("week-1")).toBeTruthy();
  });
});
