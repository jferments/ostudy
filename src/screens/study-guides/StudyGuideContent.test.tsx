import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { StudyGuideSectionContent } from "../../ipc/studyGuides";
import { StudyGuideStructuredContent } from "./StudyGuideContent";

function content(overrides: Partial<StudyGuideSectionContent> = {}): StudyGuideSectionContent {
  return {
    overview: "The glomerulus filters plasma.",
    keyPoints: ["Filtration depends on pressure."],
    explanations: [{ heading: "Starling forces", body: "Hydrostatic pressure pushes fluid out." }],
    importantTerms: [{ term: "GFR", explanation: "Glomerular filtration rate." }],
    processSteps: ["Blood enters the afferent arteriole.", "Plasma crosses the filtration barrier."],
    comparisons: [{ subjectA: "Afferent", subjectB: "Efferent", relationship: "Constricting them moves GFR in opposite directions." }],
    commonMisconceptions: [],
    callouts: [
      { kind: "key_idea", title: "Pressure wins", body: "Net pressure sets filtration." },
      { kind: "tip", title: "Remember the arterioles", body: "In through afferent, out through efferent." },
    ],
    tables: [{ title: "Arteriole changes", columns: ["Change", "GFR"], rows: [["Afferent constriction", "Falls"]], caption: "Holding other factors equal." }],
    workedExamples: [],
    retrievalChecks: [{ prompt: "What sets filtration?", answer: "The net filtration pressure." }],
    visualExplanations: [{ title: "Nephron", explanation: "The filtering unit." }],
    ...overrides,
  };
}

describe("StudyGuideStructuredContent", () => {
  it("draws the section blocks with headings at the requested level", () => {
    render(<StudyGuideStructuredContent content={content()} headingLevel={3} />);

    const minute = screen.getByRole("region", { name: "In one minute" });
    expect(within(minute).getByText("The glomerulus filters plasma.")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 3, name: "Starling forces" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 3, name: "Comparisons" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 3, name: "Important terms" })).toBeTruthy();
    expect(within(screen.getByRole("region", { name: "Process" })).getAllByRole("listitem")).toHaveLength(2);
    const table = screen.getByRole("table", { name: "Arteriole changes" });
    expect(within(table).getByRole("columnheader", { name: "GFR" })).toBeTruthy();
    expect(screen.getByText("Holding other factors equal.")).toBeTruthy();
    expect(within(screen.getByRole("note", { name: "Key idea" })).getByText("Pressure wins")).toBeTruthy();
    expect(within(screen.getByRole("note", { name: "Tip" })).getByText("Remember the arterioles")).toBeTruthy();
    expect(screen.getByRole("note", { name: "Figure 1" })).toBeTruthy();
  });

  it("numbers figures after their section and can leave the terms to a margin", () => {
    render(<StudyGuideStructuredContent content={content()} headingLevel={2} sectionNumber={3} showTerms={false} />);

    expect(screen.getByRole("note", { name: "Figure 3.1" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Important terms" })).toBeNull();
    expect(screen.getByRole("heading", { level: 2, name: "Check yourself" })).toBeTruthy();
  });

  it("shows a designed page over text kept for screen readers, with the checks still live", () => {
    const { container } = render(<StudyGuideStructuredContent content={content()} headingLevel={2} showTerms={false}
      designedPage="blob:page" />);

    expect(container.querySelector("img")?.getAttribute("src")).toBe("blob:page");
    expect(screen.getByRole("heading", { level: 2, name: "Starling forces" }).closest(".tw\\:sr-only")).not.toBeNull();
    const check = screen.getByRole("region", { name: "Check yourself" });
    expect(check.closest(".tw\\:sr-only")).toBeNull();
    fireEvent.click(within(check).getByRole("button", { name: "Show answer" }));
    expect(within(check).getByText("The net filtration pressure.")).toBeTruthy();
  });

  it("keeps its own check-yourself progress when no caller owns it", () => {
    render(<StudyGuideStructuredContent content={content()} />);

    const check = screen.getByRole("region", { name: "Check yourself" });
    expect(within(check).getByText("0 of 1 answered")).toBeTruthy();
    fireEvent.click(within(check).getByRole("button", { name: "Show answer" }));
    expect(within(check).getByText("The net filtration pressure.")).toBeTruthy();
    expect(within(check).getByText("1 of 1 answered")).toBeTruthy();
    fireEvent.click(within(check).getByRole("radio", { name: "Yes" }));
    expect(within(check).getByRole("radio", { name: "Yes" }).getAttribute("aria-checked")).toBe("true");
  });
});
