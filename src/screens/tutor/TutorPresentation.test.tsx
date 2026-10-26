import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { TutorWorkspace } from "./TutorWorkspace";

const state = vi.hoisted(() => ({ send: vi.fn(), draft: "Test my knowledge", target: null as unknown, detail: null as unknown }));
vi.mock("./TutorContext", () => ({ useTutor: () => ({
  ...state, loading: false, busy: false, organizing: false, pending: null, preparation: { locked: false },
  scope: { kind: "general", id: null }, choices: [], profiles: [{}], conversations: [], projects: [],
  attachments: { items: [], adding: false, imagesBlocked: false, error: null, add: vi.fn(), remove: vi.fn() },
  needsRefresh: false, error: null, initialize: vi.fn(), setDraft: vi.fn(),
}) }));
vi.mock("../../platform/externalLinks", () => ({ openExternalUrl: vi.fn() }));
vi.mock("../../ui/overlays/overlayContext", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ui/overlays/overlayContext")>(),
  useConfirm: () => vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks(); state.target = null;
  state.detail = { conversation: { id: "a", title: "Saved chat" }, turns: [{
    id: "b", mode: "explain", userText: "Ask me questions", assistantText: "**Question 1**\n\nExplain this relationship.",
    state: "complete", citations: [], evidenceCurrent: true, pendingQuestion: false, feedback: null,
    errorMessage: "", model: "fixture", attachments: [],
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

  it("never requires a source and keeps chat actions in one menu", () => {
    render(<TutorWorkspace />);
    expect(screen.queryByText("No sources")).toBeNull();
    expect(screen.getByRole("button", { name: "Add sources" })).toBeTruthy();
    expect((screen.getByRole("button", { name: "Send" }) as HTMLButtonElement).disabled).toBe(false);
    const actions = screen.getByRole("button", { name: "Chat actions" });
    const icon = actions.querySelector("svg") as SVGElement;
    expect(icon.getAttribute("width")).toBe("20");
    expect(screen.queryByText("fixture")).toBeNull();
  });

  it("offers a quiz on the latest answer without sending the draft", () => {
    render(<TutorWorkspace />);
    fireEvent.click(screen.getByRole("button", { name: "Quiz me on this" }));
    expect(state.send).toHaveBeenCalledWith("quiz", "");
    expect(screen.getByRole("button", { name: "Copy" })).toBeTruthy();
  });

  it("keeps hint-only messages plain and does not add a quiz shortcut", () => {
    state.target = { mode: "hint" };
    state.detail = { conversation: { id: "a", title: "Hints" }, turns: [{
      id: "h", mode: "hint", userText: "Hint", assistantText: "**Think about the relationship**", state: "complete",
      citations: [], evidenceCurrent: true, pendingQuestion: false, feedback: null, errorMessage: "", model: "fixture", attachments: [],
    }] };
    render(<TutorWorkspace />);
    expect(screen.queryByRole("button", { name: "Quiz me" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Sources" })).toBeNull();
    expect(screen.getByRole("button", { name: "Request hint" })).toBeTruthy();
  });
});


describe("Tutor saved mathematical replies", () => {
  it("renders the saved answer without rewriting evidence, hint visibility or triggering a request", async () => {
    const detail = { conversation: { id: "a", title: "Saved math" }, turns: [{
      id: "m", mode: "explain", userText: "Explain", assistantText: String.raw`Answer \(x^2\)`,
      state: "complete", citations: [{ id: "s", title: "Original source" }], evidenceCurrent: false,
      pendingQuestion: false, feedback: null, errorMessage: "", model: "fixture", attachments: [],
    }] };
    const before = JSON.stringify(detail);
    state.detail = detail;
    const { container } = render(<TutorWorkspace />);
    await waitFor(() => expect(container.querySelector("math annotation")?.textContent).toBe("x^2"));
    expect((screen.getByRole("button", { name: "Source 1: Original source" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText(/This answer is historical/u)).toBeTruthy();
    expect(state.send).not.toHaveBeenCalled();
    expect(JSON.stringify(detail)).toBe(before);
  });
});
