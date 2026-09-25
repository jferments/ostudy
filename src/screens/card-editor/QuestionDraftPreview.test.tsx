import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RichDocument } from "../../ipc/common";
import type { QuestionEditorDraft } from "./questionDraft";
import { QuestionDraftPreview } from "./QuestionDraftPreview";

const mocks = vi.hoisted(() => ({ preview: vi.fn() }));

vi.mock("../../ipc/questions", () => ({ previewQuestionDraft: mocks.preview }));
vi.mock("../practice/QuestionBody", () => ({
  QuestionBody: ({ evaluation, onChange }: { evaluation: { verdict: string } | null; onChange: (value: unknown) => void }) =>
    <div>
      <button onClick={() => onChange({ kind: "true_false", selected: true })}>Choose true</button>
      <span>{evaluation?.verdict ?? "unchecked"}</span>
    </div>,
}));

function doc(text: string): RichDocument {
  return { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] };
}
const draft: QuestionEditorDraft = {
  kind: "true_false",
  statement: doc("Native preview statement"),
  statementText: "Native preview statement",
  correct: true,
  explanation: doc("Because"),
  explanationText: "Because",
  correctedStatement: { type: "doc", content: [] },
  correctedStatementText: "",
};
const question = { kind: "true_false" as const, statement: draft.statement };

beforeEach(() => {
  mocks.preview.mockReset();
  mocks.preview
    .mockResolvedValueOnce({ question, evaluation: null })
    .mockResolvedValueOnce({
      question,
      evaluation: {
        gradingPolicyVersion: 1, verdict: "correct", matchedCount: 1, requiredCount: 1,
        components: [{ kind: "boolean", selected: true, expected: true, correctedStatement: null }],
        explanation: draft.explanation,
      },
    });
});

describe("objective author preview", () => {
  it("compiles publicly in native code and checks with the native evaluator without persistence", async () => {
    render(<QuestionDraftPreview draft={draft} assets={[]} assetDraftId={null}
      disabled={false} onError={vi.fn()} />);
    await screen.findByText("unchecked");
    fireEvent.click(screen.getByRole("button", { name: "Choose true" }));
    fireEvent.click(screen.getByRole("button", { name: "Check answer" }));
    await screen.findByText("correct");
    expect(mocks.preview).toHaveBeenCalledTimes(2);
    expect(mocks.preview.mock.calls[0]?.[0]).toMatchObject({ response: null });
    expect(mocks.preview.mock.calls[1]?.[0]).toMatchObject({
      response: { kind: "true_false", selected: true },
    });
    fireEvent.click(screen.getByRole("button", { name: "Reset preview" }));
    await waitFor(() => expect(screen.getByText("unchecked")).toBeTruthy());
  });
});
