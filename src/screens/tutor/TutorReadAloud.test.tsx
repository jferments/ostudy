import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TutorWorkspace } from "./TutorWorkspace";

const state = vi.hoisted(() => ({ voice: true, detail: null as unknown }));
const speech = vi.hoisted(() => ({ speakTutorReply: vi.fn() }));
vi.mock("./TutorContext", () => ({ useTutor: () => ({
  detail: state.detail, send: vi.fn(), draft: "", target: null, loading: false, busy: false, organizing: false,
  pending: null, preparation: { locked: false }, scope: { kind: "general", id: null }, choices: [], profiles: [{}],
  conversations: [], projects: [], needsRefresh: false, error: null, initialize: vi.fn(), setDraft: vi.fn(),
}) }));
vi.mock("../lessons/useVoiceAvailable", () => ({ useVoiceAvailable: () => state.voice }));
vi.mock("../../ipc/tutor", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../ipc/tutor")>(),
  speakTutorReply: speech.speakTutorReply,
}));
vi.mock("../../media/managedPlayback", async (importOriginal) => ({
  ...await importOriginal<typeof import("../../media/managedPlayback")>(),
  createManagedMediaObjectUrl: (assetId: string) => Promise.resolve(`blob:${assetId}`),
  revokeManagedMediaObjectUrl: vi.fn(),
}));
vi.mock("../../platform/externalLinks", () => ({ openExternalUrl: vi.fn() }));

const piece = (index: number, pieces: number) => ({
  segment: { assetId: `audio-${index}`, characters: 10 }, pieces, made: true,
});

function audio(index = 0): HTMLAudioElement {
  return document.querySelectorAll("audio")[index] as HTMLAudioElement;
}

beforeEach(() => {
  vi.clearAllMocks();
  state.voice = true;
  state.detail = { conversation: { id: "a", title: "Saved chat" }, turns: [{
    id: "b".repeat(32), mode: "explain", userText: "Why?", assistantText: "Because **it** works.",
    state: "complete", citations: [], evidenceCurrent: true, pendingQuestion: false, feedback: null,
    errorMessage: "", model: "fixture",
  }] };
  vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => undefined);
});

afterEach(() => vi.restoreAllMocks());

describe("Tutor replies read aloud", () => {
  it("plays each piece in turn, loading the next in the other player while one plays", async () => {
    const played: string[] = [];
    vi.mocked(HTMLMediaElement.prototype.play).mockImplementation(function (this: HTMLMediaElement) {
      played.push(this.getAttribute("src") ?? "");
      return Promise.resolve();
    });
    speech.speakTutorReply.mockImplementation((_turn: string, index: number) => Promise.resolve(piece(index, 3)));
    render(<TutorWorkspace />);
    fireEvent.click(screen.getByRole("button", { name: "Read aloud" }));
    await waitFor(() => expect(played).toEqual(["blob:audio-0"]));
    expect(speech.speakTutorReply).toHaveBeenCalledWith("b".repeat(32), 1);
    // The next piece waits, loaded, in the second player.
    await waitFor(() => expect(audio(1).getAttribute("src")).toBe("blob:audio-1"));
    expect(screen.getByRole("button", { name: "Stop reading" }).getAttribute("aria-pressed")).toBe("true");
    await act(async () => { audio(0).onended?.(new Event("ended")); });
    await waitFor(() => expect(played).toEqual(["blob:audio-0", "blob:audio-1"]));
    await waitFor(() => expect(audio(0).getAttribute("src")).toBe("blob:audio-2"));
    await act(async () => { audio(1).onended?.(new Event("ended")); });
    await waitFor(() => expect(played).toEqual(["blob:audio-0", "blob:audio-1", "blob:audio-2"]));
    expect(speech.speakTutorReply).toHaveBeenCalledTimes(3);
    act(() => { audio(0).onended?.(new Event("ended")); });
    expect(screen.getByRole("button", { name: "Read aloud" })).toBeTruthy();
  });

  it("stops when pressed again", async () => {
    speech.speakTutorReply.mockResolvedValue(piece(0, 1));
    render(<TutorWorkspace />);
    fireEvent.click(screen.getByRole("button", { name: "Read aloud" }));
    await waitFor(() => expect(audio().getAttribute("src")).toBe("blob:audio-0"));
    fireEvent.click(screen.getByRole("button", { name: "Stop reading" }));
    expect(audio().getAttribute("src")).toBeNull();
    expect(screen.getByRole("button", { name: "Read aloud" })).toBeTruthy();
  });

  it("says why a reply could not be read", async () => {
    speech.speakTutorReply.mockRejectedValue(new Error("Spoken narration needs an OpenRouter connection."));
    render(<TutorWorkspace />);
    fireEvent.click(screen.getByRole("button", { name: "Read aloud" }));
    expect(await screen.findByText("Spoken narration needs an OpenRouter connection.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Read aloud" })).toBeTruthy();
  });

  it("is not offered without a voice", () => {
    state.voice = false;
    render(<TutorWorkspace />);
    expect(screen.getByRole("button", { name: "Copy" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Read aloud" })).toBeNull();
  });
});
