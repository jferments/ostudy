import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RichDocument } from "../../ipc/common";
import type { PracticeCard, PracticeSession, ReviewPreviewSet } from "../../ipc/practice";
import { makeCardPresentation, resetTestEntityIds } from "../../test/factories";
import { ActivePractice } from "./ActivePractice";

vi.mock("../../editor/RichTextView", () => ({
  RichTextView: ({
    document,
    mediaPlaybackActive = true,
  }: {
    document: RichDocument;
    mediaPlaybackActive?: boolean;
  }) => {
    const text = JSON.stringify(document);
    const side = text.includes("FRONT-MEDIA") ? "front" : "back";
    return (
      <div data-testid={`rich-${side}`} data-media-active={String(mediaPlaybackActive)}>
        {mediaPlaybackActive ? <audio data-testid={`media-${side}`} /> : null}
      </div>
    );
  },
}));

const previews: ReviewPreviewSet = {
  scheduleRevision: 2,
  previewedAt: 1,
  previews: [
    { outcome: "wrong", dueAt: 61_000, intervalMs: 60_000 },
    { outcome: "not_confident", dueAt: 3_601_000, intervalMs: 3_600_000 },
    { outcome: "somewhat_confident", dueAt: 86_401_000, intervalMs: 86_400_000 },
    { outcome: "very_confident", dueAt: 2_592_001_000, intervalMs: 2_592_000_000 },
  ],
};

function textDocument(text: string): RichDocument {
  return {
    type: "doc",
    attrs: { schemaVersion: 3 },
    content: [{ type: "paragraph", content: [{ type: "text", text }] }],
  };
}

function currentCard(): PracticeCard {
  const presentation = makeCardPresentation();
  return {
    presentation: {
      ...presentation,
      frontDoc: textDocument("FRONT-MEDIA"),
      backDoc: textDocument("BACK-MEDIA"),
    },
    deckName: "Media deck",
    dueAt: 1,
    scheduleState: 2,
    needsMorePractice: false,
  };
}

function sessionFor(current: PracticeCard): PracticeSession {
  return {
    id: "a".repeat(32),
    cardCount: 1,
    firstWindow: { startPosition: 0, cards: [current], hasMore: false },
    shuffle: false,
    mode: "scheduled",
    startedAt: 1,
  };
}

function renderPractice(revealed = false) {
  const current = currentCard();
  const session = sessionFor(current);
  const props: React.ComponentProps<typeof ActivePractice> = {
    session,
    current,
    position: 0,
    revealed,
    ratedCount: 0,
    previewSet: revealed ? previews : null,
    isRevealing: false,
    isRating: false,
    isEnding: false,
    isManaging: false,
    flipAnimationEnabled: true,
    errorMessage: null,
    onReveal: vi.fn(),
    onScore: vi.fn(),
    onEdit: vi.fn(),
    onScheduleAction: vi.fn(),
    onEnd: vi.fn(),
    onLinkError: vi.fn(),
  };
  return { props, ...render(<ActivePractice {...props} />) };
}

describe("ActivePractice media isolation", () => {
  beforeEach(() => {
    resetTestEntityIds();
    vi.clearAllMocks();
  });

  it("mounts media only on the visible animated face and swaps ownership after reveal", async () => {
    const view = renderPractice(false);

    expect(screen.getByTestId("rich-front").getAttribute("data-media-active")).toBe("true");
    expect(screen.getByTestId("rich-back").getAttribute("data-media-active")).toBe("false");
    expect(screen.getByTestId("media-front")).toBeTruthy();
    expect(screen.queryByTestId("media-back")).toBeNull();

    view.rerender(<ActivePractice {...view.props} revealed previewSet={previews} />);

    await waitFor(() => {
      expect(screen.getByTestId("rich-front").getAttribute("data-media-active")).toBe("false");
      expect(screen.getByTestId("rich-back").getAttribute("data-media-active")).toBe("true");
    });
    expect(screen.queryByTestId("media-front")).toBeNull();
    expect(screen.getByTestId("media-back")).toBeTruthy();
  });

  it("does not turn media clicks or keys into reveal, flip, or rating actions", async () => {
    const view = renderPractice(false);
    const frontMedia = screen.getByTestId("media-front");

    fireEvent.click(frontMedia);
    fireEvent.keyDown(frontMedia, { key: "Enter" });
    fireEvent.keyDown(frontMedia, { key: " " });
    expect(view.props.onReveal).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Show answer" })).toBeTruthy();

    view.rerender(<ActivePractice {...view.props} revealed previewSet={previews} />);
    await waitFor(() => expect(screen.getByTestId("media-back")).toBeTruthy());
    const backMedia = screen.getByTestId("media-back");

    fireEvent.click(backMedia);
    fireEvent.keyDown(backMedia, { key: "2" });
    fireEvent.keyDown(backMedia, { key: "Enter" });
    expect(view.props.onScore).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Show question" })).toBeTruthy();
  });
});
