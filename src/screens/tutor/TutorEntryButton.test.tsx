import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { TutorTarget } from "../../ipc/tutor";
import { TutorEntryButton } from "./TutorEntryButton";

const state = vi.hoisted(() => ({
  tutor: null as null | { busy: boolean; pending: unknown; openContext: ReturnType<typeof vi.fn> },
}));
vi.mock("./TutorContext", () => ({ useOptionalTutor: () => state.tutor }));

const card: TutorTarget = {
  title: "Current deck",
  scope: { kind: "deck", id: "a".repeat(32) },
  focus: { kind: "card", sessionId: "b".repeat(32), cardId: "c".repeat(32), noteRevision: 4 },
  mode: "hint",
};

beforeEach(() => { state.tutor = { busy: false, pending: null, openContext: vi.fn() }; });

describe("Contextual tutor entry", () => {
  it("does not open a chat or call the tutor merely by displaying a card", () => {
    render(<TutorEntryButton target={card} />);
    expect(state.tutor?.openContext).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Get a flashcard hint" })).toBeTruthy();
  });

  it("passes the exact practice identity without sending front or back content", () => {
    render(<TutorEntryButton target={card} />);
    fireEvent.click(screen.getByRole("button", { name: "Get a flashcard hint" }));
    expect(state.tutor?.openContext).toHaveBeenCalledOnce();
    expect(state.tutor?.openContext).toHaveBeenCalledWith(card);
    expect(Object.keys(card.focus ?? {})).toEqual(["kind", "sessionId", "cardId", "noteRevision"]);
  });

  it("uses the current guide section and its revision fingerprint", () => {
    const guide: TutorTarget = {
      title: "Current section", scope: { kind: "guide", id: "d".repeat(32) },
      focus: { kind: "guide", documentId: "d".repeat(32), guideId: "e".repeat(32),
        topicId: "f".repeat(32), contentHash: "1".repeat(64) }, mode: "explain",
    };
    render(<TutorEntryButton target={guide} />);
    fireEvent.click(screen.getByRole("button", { name: "Ask tutor about this material" }));
    expect(state.tutor?.openContext).toHaveBeenCalledWith(guide);
  });

  it("disables entry while the originating material is being changed", () => {
    render(<TutorEntryButton target={card} disabled />);
    const button = screen.getByRole("button", { name: "Get a flashcard hint" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button);
    expect(state.tutor?.openContext).not.toHaveBeenCalled();
  });

  it.each(["busy", "pending"] as const)("does not replace a %s tutor operation", (kind) => {
    if (state.tutor) {
      if (kind === "busy") state.tutor.busy = true;
      else state.tutor.pending = { operationId: "g".repeat(32) };
    }
    render(<TutorEntryButton target={card} />);
    const button = screen.getByRole("button", { name: "Get a flashcard hint" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(button);
    expect(state.tutor?.openContext).not.toHaveBeenCalled();
  });

  it("does not require Tutor in isolated non-AI study components", () => {
    state.tutor = null;
    render(<TutorEntryButton target={card} />);
    expect(screen.queryByRole("button")).toBeNull();
  });
});
