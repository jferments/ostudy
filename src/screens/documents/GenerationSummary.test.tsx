import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { GenerationReport } from "../../ipc/documents";
import { GenerationSummary } from "./GenerationSummary";

function report(): GenerationReport {
  return {
    schemaVersion: 1, generatedAt: 1,
    requestedPolicy: {
      allowedTypes: ["recall", "matching", "ordering"],
      weights: { recall: 50, matching: 25, ordering: 25 },
      maxCompoundItems: 8,
    },
    candidateNoteCount: 4, selectedNoteCount: 3, curationRemovedCount: 1,
    savedNoteCount: 2, savedCardCount: 3, skippedDuplicateCount: 1,
    availableKnowledgeUnits: 2, linkedKnowledgeUnits: 1, evidenceBindingCount: 2,
    itemCount: 4, types: [
      { family: "recall", noteCount: 1, cardCount: 2, itemCount: 2 },
      { family: "matching", noteCount: 1, cardCount: 1, itemCount: 2 },
    ],
  };
}

describe("Generation summary metadata", () => {
  it("shows captured preferences separately from saved presentations and components", () => {
    render(<GenerationSummary report={report()} />);
    expect(screen.getByRole("region", { name: "Latest card generation" })).toBeTruthy();
    expect(screen.getByText("3 cards from 2 notes")).toBeTruthy();
    expect(screen.getByText("1 / 2 available knowledge units linked")).toBeTruthy();
    const matching = within(screen.getByRole("row", { name: /^Matching/ }));
    expect(matching.getByText("25%")).toBeTruthy();
    expect(matching.getByText("33%")).toBeTruthy();
    expect(matching.getByText("1")).toBeTruthy();
    expect(matching.getByText("2")).toBeTruthy();
    expect(screen.getByText(/not a quota/)).toBeTruthy();
    expect(screen.getByText(/do not certify/)).toBeTruthy();
  });

  it("displays a zero saved format without inventing an explanation or filler", () => {
    render(<GenerationSummary report={report()} />);
    const ordering = within(screen.getByRole("row", { name: /^Ordering/ }));
    expect(ordering.getByText("25%")).toBeTruthy();
    expect(ordering.getByText("0%")).toBeTruthy();
    expect(ordering.getAllByText("0")).toHaveLength(2);
  });

  it("does not invent percentages or source coverage when no card was inserted", () => {
    render(<GenerationSummary report={{ ...report(), savedNoteCount: 0, savedCardCount: 0,
      skippedDuplicateCount: 3, availableKnowledgeUnits: 0, linkedKnowledgeUnits: 0,
      evidenceBindingCount: 0, itemCount: 0, types: [] }} />);
    expect(screen.getByText("No source coverage recorded")).toBeTruthy();
    expect(screen.queryByText("NaN%")).toBeNull();
    expect(screen.queryByText("0%")).toBeNull();
  });

  it("adds no empty metadata for older jobs without a recorded summary", () => {
    const { container, rerender } = render(<GenerationSummary report={null} />);
    expect(container.childElementCount).toBe(0);
    rerender(<GenerationSummary report={undefined} />);
    expect(container.childElementCount).toBe(0);
  });
});
