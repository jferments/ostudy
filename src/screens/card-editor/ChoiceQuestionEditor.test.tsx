import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { RichFieldListProps } from "../../editor/RichFieldList";
import { extractRichDocumentText } from "../../editor/richText";
import type { QuestionEditorDraft } from "./questionDraft";
import { newQuestionDraft } from "./questionDraft";
import { ChoiceQuestionEditor } from "./ChoiceQuestionEditor";

vi.mock("../../editor/RichFieldList", () => ({
  RichFieldList: ({ fields }: RichFieldListProps) => <div>{fields.map((field) =>
    <span key={field.id} data-optional={field.optional ? "true" : "false"}>{field.label}</span>)}</div>,
}));
vi.mock("../../editor/RichTextView", () => ({
  RichTextView: ({ document }: { document: Parameters<typeof extractRichDocumentText>[0] }) => <span>{extractRichDocumentText(document)}</span>,
}));

type ChoiceDraft = Extract<QuestionEditorDraft, { kind: "single_choice" | "select_all" }>;
const rich = (text: string) => ({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] });
function filled(kind: ChoiceDraft["kind"]): ChoiceDraft {
  const draft = newQuestionDraft(kind) as ChoiceDraft;
  return {
    ...draft,
    prompt: rich("Which answer?") as ChoiceDraft["prompt"],
    promptText: "Which answer?",
    options: draft.options.map((option, index) => ({ ...option, doc: rich(`Choice ${index + 1}`), text: `Choice ${index + 1}` })),
  };
}
function Harness({ kind }: { kind: ChoiceDraft["kind"] }) {
  const [draft, setDraft] = useState(() => filled(kind));
  return <><ChoiceQuestionEditor draft={draft} assets={[]} assetDraftId={null} disabled={false}
    onAssetImported={vi.fn()} onChange={setDraft} />
    <output data-testid="key">{draft.correctOptionIds.join(",")}</output></>;
}

describe("choice authoring answer-key preview", () => {
  it("uses the Practice card controls at the bottom to select the single correct answer", () => {
    render(<Harness kind="single_choice" />);
    expect(screen.getByText(/Select the correct answer below\./)).toBeTruthy();
    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(2);
    fireEvent.click(radios[1]);
    expect(radios[1].getAttribute("aria-checked")).toBe("true");
    expect(screen.getByTestId("key").textContent).not.toBe("");
    expect(screen.queryByText(/^Correct$/)).toBeNull();
  });

  it("uses the same bottom tiles for an exact multi-answer key and keeps explanations optional", () => {
    const { container } = render(<Harness kind="select_all" />);
    expect(screen.getByText(/Select every correct answer below\./)).toBeTruthy();
    const choices = screen.getByRole("group", { name: "Correct answers" });
    const boxes = choices.querySelectorAll('[role="checkbox"]');
    expect(boxes).toHaveLength(2);
    fireEvent.click(boxes[0]);
    fireEvent.click(boxes[1]);
    expect(screen.getByTestId("key").textContent?.split(",")).toHaveLength(2);
    const optional = [...container.querySelectorAll('[data-optional="true"]')].map((node) => node.textContent);
    expect(optional).toContain("Overall explanation");
    expect(optional.filter((label) => label === "Option explanation")).toHaveLength(2);
  });
});
