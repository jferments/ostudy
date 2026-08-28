import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { StudyGuidesRoute } from "./StudyGuidesRoute";

vi.mock("./StudyGuideLibrarySection", () => ({
  StudyGuideLibrarySection: ({
    heading,
    showToast,
    onPracticeCards,
  }: {
    heading: string;
    showToast: (kind: "success", text: string) => void;
    onPracticeCards: (cardIds: string[]) => Promise<void>;
  }) => (
    <section aria-label="Guide library mock">
      <h2>{heading}</h2>
      <button type="button" onClick={() => showToast("success", "Guide opened")}>
        Show toast
      </button>
      <button type="button" onClick={() => void onPracticeCards(["linked-card"])}>
        Study linked cards
      </button>
    </section>
  ),
}));

describe("StudyGuidesRoute", () => {
  it("renders one route heading and delegates existing guide behavior", () => {
    const showToast = vi.fn();
    const onPracticeCards = vi.fn().mockResolvedValue(undefined);

    const { container } = render(
      <StudyGuidesRoute showToast={showToast} onPracticeCards={onPracticeCards} />,
    );

    expect(container.firstElementChild?.getAttribute("aria-label")).toBe("Study Guides");
    expect(container.firstElementChild?.className).toContain("full");
    expect(screen.getAllByRole("heading", { level: 1 })).toHaveLength(1);
    expect(screen.getByRole("heading", { level: 1, name: "Study Guides" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: "Generated guides" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Show toast" }));
    fireEvent.click(screen.getByRole("button", { name: "Study linked cards" }));
    expect(showToast).toHaveBeenCalledWith("success", "Guide opened");
    expect(onPracticeCards).toHaveBeenCalledWith(["linked-card"]);
  });
});
