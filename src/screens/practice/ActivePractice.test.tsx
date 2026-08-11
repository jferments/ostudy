import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  type PracticeCard,
  type PracticeSession,
  type ReviewPreviewSet,
} from "../../ipc/practice";
import { makeCardPresentation, resetTestEntityIds } from "../../test/factories";
import { ActivePractice } from "./ActivePractice";

vi.mock("../../editor/RichTextView", () => ({
  RichTextView: ({ className = "" }: { className?: string }) => (
    <div className={className}>Rich card content</div>
  ),
}));

const previews: ReviewPreviewSet = {
  scheduleRevision: 2,
  previewedAt: 1,
  previews: [
    { outcome: "wrong", dueAt: 61_000, intervalMs: 60_000 },
    { outcome: "not_confident", dueAt: 3_601_000, intervalMs: 3_600_000 },
    {
      outcome: "somewhat_confident",
      dueAt: 86_401_000,
      intervalMs: 86_400_000,
    },
    {
      outcome: "very_confident",
      dueAt: 2_592_001_000,
      intervalMs: 2_592_000_000,
    },
  ],
};

function makeCurrent(): PracticeCard {
  return {
    presentation: makeCardPresentation(),
    deckName: "Cardiology",
    dueAt: 1,
    scheduleState: 2,
    needsMorePractice: true,
  };
}

function makeSession(): PracticeSession {
  const current = makeCurrent();
  return {
    id: "a".repeat(32),
    cardCount: 1,
    firstWindow: {
      startPosition: 0,
      cards: [current],
      hasMore: false,
    },
    shuffle: false,
    mode: "scheduled",
    startedAt: 1,
  };
}

function renderActive(
  overrides: Partial<React.ComponentProps<typeof ActivePractice>> = {},
) {
  const session = makeSession();
  const props: React.ComponentProps<typeof ActivePractice> = {
    session,
    current: session.firstWindow.cards[0] as PracticeCard,
    position: 0,
    revealed: false,
    ratedCount: 0,
    previewSet: null,
    isRevealing: false,
    isRating: false,
    isEnding: false,
    isManaging: false,
    errorMessage: null,
    onReveal: vi.fn(),
    onScore: vi.fn(),
    onScheduleAction: vi.fn(),
    onEnd: vi.fn(),
    onLinkError: vi.fn(),
    ...overrides,
  };
  render(<ActivePractice {...props} />);
  return props;
}

describe("ActivePractice", () => {
  beforeEach(() => {
    resetTestEntityIds();
    vi.clearAllMocks();
  });

  it("reveals with Space or Enter when focus is not in an interactive control", () => {
    const props = renderActive();

    fireEvent.keyDown(window, { key: " " });
    fireEvent.keyDown(window, { key: "Enter" });

    expect(props.onReveal).toHaveBeenCalledTimes(2);
  });

  it("does not steal reveal shortcuts from interactive controls", () => {
    const props = renderActive();
    const input = document.createElement("input");
    document.body.append(input);

    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.keyDown(screen.getByRole("button", { name: "Show answer" }), {
      key: " ",
    });

    expect(props.onReveal).not.toHaveBeenCalled();
    input.remove();
  });

  it("reveals when the animated card surface is clicked", () => {
    const props = renderActive({ flipAnimationEnabled: true });

    fireEvent.click(screen.getByRole("button", { name: "Reveal answer" }));

    expect(props.onReveal).toHaveBeenCalledOnce();
  });

  it("shows concise ratings and submits 0-3 keyboard scores after reveal", () => {
    const props = renderActive({ revealed: true, previewSet: previews });

    expect(screen.getByRole("button", { name: "0 Wrong" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "1 Hard" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "2 Medium" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "3 Easy" })).toBeTruthy();
    expect(screen.queryByText(/About 1/u)).toBeNull();
    expect(screen.queryByText("I could not recall it.")).toBeNull();

    for (const key of ["0", "1", "2", "3"]) {
      fireEvent.keyDown(window, { key });
    }
    expect(props.onScore).toHaveBeenNthCalledWith(1, 0);
    expect(props.onScore).toHaveBeenNthCalledWith(2, 1);
    expect(props.onScore).toHaveBeenNthCalledWith(3, 2);
    expect(props.onScore).toHaveBeenNthCalledWith(4, 3);
  });

  it("blocks shortcuts and buttons while any review mutation is pending", () => {
    const props = renderActive({
      revealed: true,
      previewSet: previews,
      isRating: true,
    });

    fireEvent.keyDown(window, { key: "2" });
    expect(props.onScore).not.toHaveBeenCalled();
    expect(
      (screen.getByRole("button", { name: /2 Medium/u }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByRole("button", { name: "End session" }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("routes shared menu schedule controls to the exact-card action callback", () => {
    const props = renderActive();
    const onScheduleAction = vi.mocked(props.onScheduleAction);
    const actions = [
      ["Bury until tomorrow", ["bury"]],
      ["Suspend card", ["suspend"]],
      ["Reschedule for tomorrow", ["reschedule", { kind: "tomorrow" }]],
      ["Reset schedule", ["reset"]],
    ] as const;

    for (const [label] of actions) {
      fireEvent.click(screen.getByRole("button", { name: "More" }));
      fireEvent.click(screen.getByRole("menuitem", { name: label }));
    }

    expect(onScheduleAction).toHaveBeenNthCalledWith(1, "bury");
    expect(onScheduleAction).toHaveBeenNthCalledWith(2, "suspend");
    expect(onScheduleAction).toHaveBeenNthCalledWith(3, "reschedule", {
      kind: "tomorrow",
    });
    expect(onScheduleAction).toHaveBeenNthCalledWith(4, "reset");
  });

  it("reports visible card progress with one-based semantics", () => {
    renderActive();

    const progress = screen.getByRole("progressbar", { name: "Practice session progress" });
    expect(progress.getAttribute("aria-valuenow")).toBe("1");
    expect(progress.getAttribute("aria-valuemax")).toBe("1");
  });

  it("renders the authoritative session mode, deck, and weak-card signal", () => {
    renderActive({ session: { ...makeSession(), mode: "extra" } });

    expect(screen.getByText("EXTRA PRACTICE")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Cardiology" })).toBeTruthy();
    expect(screen.getByText("Needs more practice")).toBeTruthy();
  });
});
