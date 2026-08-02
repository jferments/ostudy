import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { PracticeScreen } from "./PracticeScreen";

vi.mock("../editor/RichTextView", () => ({
  RichTextView: () => <div>Card content</div>,
}));

vi.mock("../ipc", () => ({
  completePracticeSession: vi.fn(),
  createSubmissionId: () => "f".repeat(32),
  getAnalytics: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  markPracticeCardRevealed: vi.fn(),
  markPracticeCardViewed: vi.fn(),
  startCardPractice: vi.fn(),
  startSelectedPractice: vi.fn(),
  submitPracticeReview: vi.fn(),
}));

const deck: ipc.Deck = {
  id: "a".repeat(32),
  name: "Dog facts",
  description: "",
  cardCount: 2,
  dueCount: 1,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const noDueDeck: ipc.Deck = {
  ...deck,
  id: "9".repeat(32),
  name: "Reviewed dog facts",
  dueCount: 0,
};

const settings: ipc.AppSettings = {
  theme: "synthwave",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
};

const card: ipc.Flashcard = {
  id: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  direction: "forward",
  cardType: "basic",
  cardLabel: "Basic",
  generatedCardCount: 1,
  frontDoc: { type: "doc", content: [] },
  backDoc: { type: "doc", content: [] },
  frontText: "Do dogs have fur?",
  backText: "Yes",
  assets: [],
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const session: ipc.PracticeSession = {
  id: "d".repeat(32),
  cards: [{ card, deckName: deck.name, dueAt: Date.now() }],
  shuffle: true,
  startedAt: Date.now(),
};

const analytics: ipc.Analytics = {
  deckCount: 1,
  cardCount: 2,
  dueNow: 1,
  reviewCount: 5,
  correctCount: 4,
  correctRate: 0.8,
  averageResponseTimeMs: 1_000,
  ratingDistribution: { again: 1, hard: 0, good: 1, easy: 3 },
  deckStats: [
    {
      deckId: deck.id,
      deckName: deck.name,
      cardCount: 2,
      dueCount: 1,
      reviewCount: 5,
      correctCount: 4,
      correctRate: 0.8,
      averageResponseTimeMs: 1_000,
    },
  ],
  recentReviews: [
    {
      id: "e".repeat(32),
      deckName: deck.name,
      cardId: card.id,
      frontText: card.frontText,
      outcome: "very_confident",
      rating: 4,
      responseTimeMs: 1_000,
      reviewedAt: Date.now(),
      dueAfter: Date.now() + 86_400_000,
    },
    {
      id: "f".repeat(32),
      deckName: deck.name,
      cardId: card.id,
      frontText: card.frontText,
      outcome: "somewhat_confident",
      rating: 3,
      responseTimeMs: 1_000,
      reviewedAt: Date.now() - 1_000,
      dueAfter: Date.now() + 86_400_000,
    },
  ],
};

describe("PracticeScreen", () => {
  beforeEach(() => {
    vi.mocked(ipc.getAnalytics).mockResolvedValue(analytics);
    vi.mocked(ipc.markPracticeCardViewed).mockResolvedValue(undefined);
    vi.mocked(ipc.markPracticeCardRevealed).mockResolvedValue(undefined);
    vi.mocked(ipc.completePracticeSession).mockResolvedValue(undefined);
    vi.mocked(ipc.startSelectedPractice).mockResolvedValue(session);
    vi.mocked(ipc.startCardPractice).mockResolvedValue(session);
    vi.mocked(ipc.submitPracticeReview).mockResolvedValue({
      cardId: card.id,
      rating: 2,
      outcome: "not_confident",
      dueAt: Date.now() + 86_400_000,
      completed: true,
      ratedCount: 1,
      correctCount: 1,
    });
  });

  it("uses a compact shuffle control and shows recent deck confidence", async () => {
    render(
      <PracticeScreen
        decks={[deck]}
        settings={settings}
        launchRequest={null}
        onLaunchConsumed={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    expect(await screen.findByText("Recently practiced")).toBeTruthy();
    expect(screen.queryByText("Session options")).toBeNull();
    expect(screen.getByRole("checkbox", { name: "Shuffle decks" })).toBeTruthy();
    expect(await screen.findByText("2.5 / 3")).toBeTruthy();
  });

  it("allows a deck with no due cards to be selected and practiced", async () => {
    render(
      <PracticeScreen
        decks={[noDueDeck]}
        settings={settings}
        launchRequest={null}
        onLaunchConsumed={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    const deckChoice = await screen.findByRole("checkbox", {
      name: /Reviewed dog facts/u,
    });
    expect(deckChoice.hasAttribute("disabled")).toBe(false);
    fireEvent.click(deckChoice);

    const sessionCountLabel = screen.getByText("cards in session");
    expect(sessionCountLabel.previousElementSibling?.textContent).toBe("2");
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));

    await waitFor(() => {
      expect(ipc.startSelectedPractice).toHaveBeenCalledWith({
        deckIds: [noDueDeck.id],
        shuffle: true,
      });
    });
  });

  it("auto-starts a deck-level request even when that deck has nothing due", async () => {
    const onLaunchConsumed = vi.fn();
    render(
      <PracticeScreen
        decks={[noDueDeck]}
        settings={settings}
        launchRequest={{
          requestId: 7,
          deckIds: [noDueDeck.id],
          cardIds: [],
          autoStart: true,
        }}
        onLaunchConsumed={onLaunchConsumed}
        showToast={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(onLaunchConsumed).toHaveBeenCalledTimes(1);
      expect(ipc.startSelectedPractice).toHaveBeenCalledWith({
        deckIds: [noDueDeck.id],
        shuffle: true,
      });
    });
  });

  it("auto-starts a temporary session from explicit filtered card IDs", async () => {
    const onLaunchConsumed = vi.fn();
    render(
      <PracticeScreen
        decks={[deck]}
        settings={settings}
        launchRequest={{
          requestId: 8,
          deckIds: [],
          cardIds: [card.id],
          autoStart: true,
        }}
        onLaunchConsumed={onLaunchConsumed}
        showToast={vi.fn()}
      />,
    );

    await waitFor(() => {
      expect(onLaunchConsumed).toHaveBeenCalledTimes(1);
      expect(ipc.startCardPractice).toHaveBeenCalledWith({
        cardIds: [card.id],
        shuffle: true,
      });
    });
  });

  it("asks one 0–3 question and maps 1 Hard to the durable Hard outcome", async () => {
    render(
      <PracticeScreen
        decks={[deck]}
        settings={settings}
        launchRequest={null}
        onLaunchConsumed={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));

    fireEvent.click(await screen.findByRole("button", { name: "Show answer" }));

    expect(await screen.findByRole("button", { name: "0 Wrong" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "1 Hard" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "2 Medium" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "3 Easy" })).toBeTruthy();
    expect(screen.queryByText("Did you know it?")).toBeNull();
    expect(screen.queryByRole("button", { name: "Right" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "1 Hard" }));

    await waitFor(() => {
      expect(ipc.submitPracticeReview).toHaveBeenCalledWith(
        expect.objectContaining({ outcome: "not_confident" }),
      );
    });
    expect(await screen.findByText("Session complete")).toBeTruthy();
  });
});
