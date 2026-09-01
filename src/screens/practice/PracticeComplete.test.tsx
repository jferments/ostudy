import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ConfidenceCounts } from "./confidence";
import { PracticeComplete } from "./PracticeComplete";

const confidenceCounts: ConfidenceCounts = [1, 2, 3, 4];

describe("PracticeComplete", () => {
  it("shows labeled session analytics below the completion message", () => {
    render(
      <PracticeComplete
        ratedCount={10}
        confidenceCounts={confidenceCounts}
        isStarting={false}
        canRestart
        errorMessage={null}
        onViewAnalytics={vi.fn()}
        onChooseDecks={vi.fn()}
        onRestart={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Practice complete" })).toBeTruthy();
    const analytics = screen.getByRole("region", { name: "How this session went" });
    expect(within(analytics).getByText("Cards studied")).toBeTruthy();
    expect(within(analytics).getByText("70%")).toBeTruthy();
    expect(within(analytics).getByText("Confidence distribution")).toBeTruthy();
    expect(within(analytics).getByText("Wrong")).toBeTruthy();
  });

  it("keeps completion navigation actions explicit", () => {
    const onViewAnalytics = vi.fn();
    const onChooseDecks = vi.fn();
    const onRestart = vi.fn();
    render(
      <PracticeComplete
        ratedCount={10}
        confidenceCounts={confidenceCounts}
        isStarting={false}
        canRestart
        errorMessage={null}
        onViewAnalytics={onViewAnalytics}
        onChooseDecks={onChooseDecks}
        onRestart={onRestart}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "View analytics" }));
    fireEvent.click(screen.getByRole("button", { name: "Choose decks" }));
    fireEvent.click(screen.getByRole("button", { name: "Practice this scope again" }));
    expect(onViewAnalytics).toHaveBeenCalledTimes(1);
    expect(onChooseDecks).toHaveBeenCalledTimes(1);
    expect(onRestart).toHaveBeenCalledTimes(1);
  });
});
