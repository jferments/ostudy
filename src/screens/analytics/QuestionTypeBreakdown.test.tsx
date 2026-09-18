import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { QuestionTypeAnalytics } from "../../ipc/analytics";
import { QuestionTypeBreakdown } from "./QuestionTypeBreakdown";

function result(overrides: Partial<QuestionTypeAnalytics> = {}): QuestionTypeAnalytics {
  return {
    questionType: "matching",
    reviewCount: 3,
    selfAssessedCount: 0,
    objectiveCount: 3,
    correctCount: 1,
    incorrectCount: 1,
    incompleteCount: 1,
    objectiveAccuracy: 1 / 3,
    unassistedObjectiveCount: 2,
    unassistedCorrectCount: 1,
    unassistedAccuracy: 0.5,
    assistanceTrackedCount: 3,
    assistedCount: 1,
    overrideCount: 1,
    remediationCount: 2,
    unavailableEvidenceCount: 0,
    ...overrides,
  };
}

describe("Question type evidence", () => {
  it("keeps objective accuracy, assistance, overrides and retries distinct", () => {
    render(<QuestionTypeBreakdown rows={[result()]} />);
    const row = within(screen.getByRole("row", { name: /Matching/ }));
    expect(row.getByText("33%")).toBeTruthy();
    expect(row.getByText("50%")).toBeTruthy();
    expect(row.getByText("1 / 3 checked")).toBeTruthy();
    expect(row.getByText("1 / 2 checked")).toBeTruthy();
    expect(row.getByText("1 incorrect · 1 incomplete")).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "Overrides" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "Retries" })).toBeTruthy();
  });

  it("does not label self-assessment or missing evidence as zero percent accuracy", () => {
    const empty = result({
      reviewCount: 1, objectiveCount: 0, correctCount: 0, incorrectCount: 0,
      incompleteCount: 0, objectiveAccuracy: null, unassistedObjectiveCount: 0,
      unassistedCorrectCount: 0, unassistedAccuracy: null, assistanceTrackedCount: 0,
      assistedCount: 0, overrideCount: 0, remediationCount: 0,
    });
    render(<QuestionTypeBreakdown rows={[
      { ...empty, questionType: "image_occlusion", selfAssessedCount: 1 },
      { ...empty, questionType: "ordering", unavailableEvidenceCount: 1 },
    ]} />);
    expect(screen.getByText("Self-assessed only")).toBeTruthy();
    expect(screen.getByText("No objective evidence")).toBeTruthy();
    expect(screen.getByText("1 without assessment evidence")).toBeTruthy();
    expect(screen.queryByText("0%")).toBeNull();
  });

  it("renders every supported question type with no fallback to Basic", () => {
    const kinds: QuestionTypeAnalytics["questionType"][] = [
      "basic", "cloze", "single_choice", "select_all", "true_false", "matching",
      "grouping", "ordering", "image_occlusion",
    ];
    render(<QuestionTypeBreakdown rows={kinds.map((questionType) => result({ questionType }))} />);
    expect(screen.getAllByRole("rowheader")).toHaveLength(9);
    expect(screen.getByRole("rowheader", { name: "Select all" })).toBeTruthy();
    expect(screen.getByRole("rowheader", { name: "Single best answer" })).toBeTruthy();
  });

  it("omits empty type statistics instead of adding another empty-state panel", () => {
    const { container } = render(<QuestionTypeBreakdown rows={[]} />);
    expect(container.childElementCount).toBe(0);
  });
});
