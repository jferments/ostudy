import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TutorWorkspace } from "./TutorWorkspace";

const state = vi.hoisted(() => ({ send: vi.fn(), draft: "Test my knowledge", target: null as unknown, detail: null as unknown }));
vi.mock("./TutorContext", () => ({ useTutor: () => ({
  ...state, loading: false, busy: false, pending: null, preparation: { locked: false },
  scope: { kind: "general", id: null }, choices: [], profiles: [{}], conversations: [],
  needsRefresh: false, error: null, initialize: vi.fn(), setDraft: vi.fn(),
}) }));
vi.mock("../../platform/externalLinks", () => ({ openExternalUrl: vi.fn() }));
vi.mock("../../ui/overlays/overlayContext", () => ({ useConfirm: () => vi.fn() }));

beforeEach(() => {
  vi.clearAllMocks(); state.target = null;
  state.detail = { conversation: { id: "a", title: "Saved chat" }, turns: [{
    id: "b", mode: "explain", userText: "Ask me questions", assistantText: "**Question 1**\n\nExplain this relationship.",
    state: "complete", citations: [], evidenceCurrent: true, pendingQuestion: false, feedback: null,
    errorMessage: "", model: "fixture",
  }] };
});

describe("Conversational Tutor presentation", () => {
  it("formats existing replies and offers Send without a Quiz me button", () => {
    render(<TutorWorkspace />);
    expect(screen.getByText("Question 1").tagName).toBe("STRONG");
    expect(screen.queryByRole("button", { name: "Quiz me" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(state.send).toHaveBeenCalledWith("explain");
  });

  it("keeps hint-only messages plain and does not add a quiz shortcut", () => {
    state.target = { mode: "hint" };
    state.detail = { conversation: { id: "a", title: "Hints" }, turns: [{
      id: "h", mode: "hint", userText: "Hint", assistantText: "**Think about the relationship**", state: "complete",
      citations: [], evidenceCurrent: true, pendingQuestion: false, feedback: null, errorMessage: "", model: "fixture",
    }] };
    render(<TutorWorkspace />);
    expect(screen.queryByRole("button", { name: "Quiz me" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Sources" })).toBeNull();
    expect(screen.getByRole("button", { name: "Request hint" })).toBeTruthy();
  });
});
