import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as progressIpc from "../../ipc/progress";
import type { SessionSummary } from "../../ipc/progress";
import { deferred } from "../../test/factories";
import type { ConfidenceCounts } from "./confidence";
import { missedSentence, PracticeComplete, sessionDuration, streakLine } from "./PracticeComplete";

vi.mock("../../ipc/progress", () => ({ getSessionSummary: vi.fn() }));

const confidenceCounts: ConfidenceCounts = [4, 2, 30, 11];
const sessionId = "a".repeat(32);

function summary(overrides: Partial<SessionSummary> = {}): SessionSummary {
  return {
    sessionId,
    mode: "scheduled",
    ratedCount: 47,
    rememberedCount: 43,
    studyTimeMs: 9 * 60_000,
    nowSolidCount: 8,
    missedCardIds: ["1", "2", "3", "4"].map((digit) => digit.repeat(32)),
    missedTopic: { tagId: "b".repeat(32), name: "Muscarinic antagonists", cardCount: 4 },
    dueTodayCount: 0,
    // 4 October 2026 is a Sunday.
    today: { localDate: "2026-10-04", reviewCount: 47, studyTimeMs: 9 * 60_000 },
    currentStreak: { length: 13, startLocalDate: "2026-09-22", endLocalDate: "2026-10-04" },
    longestStreak: { length: 20, startLocalDate: "2026-06-01", endLocalDate: "2026-06-20" },
    lastAsLongStreak: { length: 14, startLocalDate: "2026-07-01", endLocalDate: "2026-07-14" },
    ...overrides,
  };
}

function renderComplete(overrides: Partial<React.ComponentProps<typeof PracticeComplete>> = {}) {
  const props: React.ComponentProps<typeof PracticeComplete> = {
    sessionId,
    ratedCount: 47,
    confidenceCounts,
    isStarting: false,
    canRestart: true,
    errorMessage: null,
    onDone: vi.fn(),
    onViewProgress: vi.fn(),
    onPracticeCards: vi.fn(),
    onRestart: vi.fn(),
    ...overrides,
  };
  render(<PracticeComplete {...props} />);
  return props;
}

describe("PracticeComplete", () => {
  beforeEach(() => {
    vi.mocked(progressIpc.getSessionSummary).mockReset();
  });

  it("reports the session, today's streak and the cards to practice again", async () => {
    vi.mocked(progressIpc.getSessionSummary).mockResolvedValue(summary());
    const props = renderComplete();

    expect(screen.getByRole("heading", { name: "Session complete" })).toBeTruthy();
    expect(await screen.findByText("47 cards in 9 minutes. Everything due today is done.")).toBeTruthy();
    expect(screen.getByText("Review · Sunday")).toBeTruthy();
    expect(screen.getByText("13-day streak, your longest since July")).toBeTruthy();
    const facts = screen.getByLabelText("This session");
    expect(within(facts).getByText("91%")).toBeTruthy();
    expect(within(facts).getByText("8")).toBeTruthy();
    expect(within(facts).getByText("9 min")).toBeTruthy();
    const missed = screen.getByRole("region", { name: "Missed cards" });
    expect(missed.textContent).toBe("The 4 you missed are all on Muscarinic antagonists.Practice 4");

    fireEvent.click(within(missed).getByRole("button", { name: "Practice 4" }));
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    fireEvent.click(screen.getByRole("button", { name: "See your progress" }));
    expect(props.onPracticeCards).toHaveBeenCalledWith(summary().missedCardIds);
    expect(props.onDone).toHaveBeenCalledTimes(1);
    expect(props.onViewProgress).toHaveBeenCalledTimes(1);
    // Nothing is left due, so there is nothing to keep going on.
    expect(screen.queryByRole("button", { name: /Keep going/u })).toBeNull();
  });

  it("shows the counts it kept while the summary loads, and offers more when cards are still due", async () => {
    const pending = deferred<SessionSummary>();
    vi.mocked(progressIpc.getSessionSummary).mockReturnValue(pending.promise);
    const props = renderComplete();
    expect(screen.getByText("47 cards reviewed. Every answer is saved.")).toBeTruthy();
    expect(screen.getByText("91%")).toBeTruthy();

    pending.resolve(summary({ dueTodayCount: 12, missedCardIds: [], missedTopic: null }));
    expect(await screen.findByText("47 cards in 9 minutes. 12 more cards due today.")).toBeTruthy();
    expect(screen.getByText("You remembered every card.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Keep going: 12 cards due" }));
    expect(props.onRestart).toHaveBeenCalledTimes(1);
  });

  it("keeps the completion usable when the summary fails, and retries it", async () => {
    vi.mocked(progressIpc.getSessionSummary)
      .mockRejectedValueOnce(new Error("The library is busy."))
      .mockResolvedValueOnce(summary());
    renderComplete();
    expect(await screen.findByText(/this summary could not load: The library is busy\./u)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Done" }).hasAttribute("disabled")).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("13-day streak, your longest since July")).toBeTruthy();
    expect(screen.queryByText(/could not load/u)).toBeNull();
  });
});

describe("completion sentences", () => {
  it("names the time spent in words", () => {
    expect(sessionDuration(20_000)).toBe("under a minute");
    expect(sessionDuration(60_000)).toBe("1 minute");
    expect(sessionDuration(65 * 60_000)).toBe("1 hr 5 min");
  });

  it("compares the streak with the last one as long", () => {
    const current = { length: 5, startLocalDate: "2026-10-10", endLocalDate: "2026-10-14" };
    expect(streakLine(null, null, "2026-10-14")).toBeNull();
    expect(streakLine(current, null, "2026-10-14")).toBe("5-day streak, your longest yet");
    expect(streakLine(current, { length: 6, startLocalDate: "2026-10-01", endLocalDate: "2026-10-06" }, "2026-10-14"))
      .toBe("5-day streak, your longest since October 6");
    expect(streakLine({ ...current, length: 1 }, null, "2026-10-14")).toBe("Day one of a new streak. Study tomorrow to keep it going.");
  });

  it("names a shared topic only when the missed cards have one", () => {
    const one = summary({ missedCardIds: ["1".repeat(32)], missedTopic: { tagId: "b".repeat(32), name: "Heart", cardCount: 1 } });
    expect(missedSentence(one)).toEqual({ lead: "The one you missed", rest: " is on Heart." });
    const some = summary({ missedTopic: { tagId: "b".repeat(32), name: "Heart", cardCount: 2 } });
    expect(missedSentence(some)).toEqual({ lead: "You missed 4 cards", rest: ", 2 of them on Heart." });
    expect(missedSentence(summary({ missedTopic: null }))).toEqual({ lead: "You missed 4 cards", rest: "." });
    expect(missedSentence(summary({ missedCardIds: [] }))).toBeNull();
  });
});
