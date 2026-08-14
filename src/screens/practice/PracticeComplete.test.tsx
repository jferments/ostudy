import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { PracticeComplete } from "./PracticeComplete";

const emit = vi.fn();
vi.mock("../../ui/feedback/ExperienceProvider", () => ({
  useExperience: () => ({ emit }),
}));

const confidenceCounts = { 0: 1, 1: 2, 2: 3, 3: 4 } as const;

describe("PracticeComplete", () => {
  it("shows labeled session analytics below the completion message", () => {
    render(
      <PracticeComplete
        ratedCount={10}
        confidenceCounts={confidenceCounts}
        isStarting={false}
        errorMessage={null}
        onViewAnalytics={vi.fn()}
        onChooseDecks={vi.fn()}
        onRestart={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "Practice complete" })).toBeInTheDocument();
    const analytics = screen.getByRole("region", { name: "How this session went" });
    expect(within(analytics).getByText("Cards studied")).toBeInTheDocument();
    expect(within(analytics).getByText("70%")).toBeInTheDocument();
    expect(within(analytics).getByText("Confidence distribution")).toBeInTheDocument();
    expect(within(analytics).getByText("Wrong")).toBeInTheDocument();
    expect(emit).toHaveBeenCalledWith({ type: "session_completed" });
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
        errorMessage={null}
        onViewAnalytics={onViewAnalytics}
        onChooseDecks={onChooseDecks}
        onRestart={onRestart}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "View full analytics" }));
    fireEvent.click(screen.getByRole("button", { name: "Back to practice" }));
    fireEvent.click(screen.getByRole("button", { name: "Practice this scope again" }));
    expect(onViewAnalytics).toHaveBeenCalledTimes(1);
    expect(onChooseDecks).toHaveBeenCalledTimes(1);
    expect(onRestart).toHaveBeenCalledTimes(1);
  });
});
