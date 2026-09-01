import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as practiceIpc from "../ipc/practice";
import { makePracticeSession, makeSettings, resetTestEntityIds } from "../test/factories";
import { PracticeScreen } from "./PracticeScreen";


vi.mock("../ipc/common", () => ({
  createMutationId: vi.fn(() => "1".repeat(32)),
  createOperationId: vi.fn(() => "2".repeat(32)),
  createSubmissionId: vi.fn(() => "3".repeat(32)),
  getErrorCode: vi.fn(() => null),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../ipc/practice", () => ({
  completePracticeSession: vi.fn(),
  getPracticeCardWindow: vi.fn(),
  markPracticeCardRevealed: vi.fn(),
  markPracticeCardViewed: vi.fn(),
  startCardPractice: vi.fn(),
  startSelectedPractice: vi.fn(),
  submitPracticeReview: vi.fn(),
}));
vi.mock("../ipc/scheduling", () => ({
  applyScheduleAction: vi.fn(),
}));

vi.mock("./practice/ActivePractice", () => ({
  ActivePractice: () => <div>Active practice</div>,
}));

vi.mock("./practice/PracticeComplete", () => ({
  PracticeComplete: () => <div>Practice complete</div>,
}));

describe("PracticeScreen tag scope", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTestEntityIds();
    vi.mocked(practiceIpc.markPracticeCardViewed).mockResolvedValue(undefined);
    vi.mocked(practiceIpc.startSelectedPractice).mockResolvedValue(makePracticeSession());
  });

  it("auto-starts a tag-only scope without enumerating decks", async () => {
    const tagId = "a".repeat(32);
    const onLaunchConsumed = vi.fn();

    render(
      <PracticeScreen
        decks={[]}
        settings={makeSettings({ practiceShuffle: true })}
        launchRequest={{
          requestId: 1,
          kind: "selected",
          deckIds: [],
          tagIds: [tagId],
          shuffle: true,
          mode: "scheduled",
        }}
        onLaunchConsumed={onLaunchConsumed}
        returnTarget={{ section: "analytics" }}
        onViewAnalytics={vi.fn()}
        onEndEarly={vi.fn()}
        onChooseDecks={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(practiceIpc.startSelectedPractice).toHaveBeenCalledWith({
        operationId: "2".repeat(32),
        deckIds: [],
        tagIds: [tagId],
        shuffle: true,
        mode: "scheduled",
      });
      expect(onLaunchConsumed).toHaveBeenCalledOnce();
    });
  });
});
