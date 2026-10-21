import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { getPodcast, narratePodcastTurn, type Podcast } from "../../ipc/podcasts";
import { createManagedMediaObjectUrl, revokeManagedMediaObjectUrl } from "../../media/managedPlayback";
import { PodcastPlayer } from "./PodcastPlayer";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/podcasts", () => ({ getPodcast: vi.fn(), narratePodcastTurn: vi.fn() }));
vi.mock("../../media/managedPlayback", () => ({
  createManagedMediaObjectUrl: vi.fn(),
  revokeManagedMediaObjectUrl: vi.fn(),
  playbackErrorMessage: (failure: unknown) => (failure instanceof Error ? failure.message : String(failure)),
}));

const podcast: Podcast = {
  id: "podcast-1",
  subjectId: "subject-1",
  title: "From filtration to urine",
  targetMinutes: 15,
  minutes: 14,
  hosts: [
    { name: "Ada", voice: "builtin:calm-older-woman" },
    { name: "Ben", voice: "builtin:relaxed-man" },
  ],
  chapters: [
    {
      lessonId: "lesson-1",
      title: "Filtration",
      turns: [
        { key: "0.0", host: 0, text: "Welcome. Today we follow water through the kidney." },
        { key: "0.1", host: 1, text: "So where does it start?" },
      ],
    },
    {
      lessonId: "lesson-2",
      title: "The loop of Henle",
      turns: [{ key: "1.0", host: 0, text: "The loop builds a salt gradient." }],
    },
  ],
  createdAt: 1,
};

function audio(): HTMLAudioElement {
  return document.querySelector("audio")!;
}

function open() {
  const onClose = vi.fn();
  render(<PodcastPlayer podcastId="podcast-1" subjectName="Renal physiology" onClose={onClose} />);
  return onClose;
}

describe("PodcastPlayer", () => {
  beforeEach(() => {
    vi.mocked(getPodcast).mockReset();
    vi.mocked(getPodcast).mockResolvedValue(podcast);
    vi.mocked(narratePodcastTurn).mockReset();
    vi.mocked(narratePodcastTurn).mockImplementation(async (_podcast, turnKey) => ({
      segments: [{ assetId: `turn:${turnKey}`, characters: 20 }],
      made: 1,
    }));
    vi.mocked(createManagedMediaObjectUrl).mockReset();
    vi.mocked(revokeManagedMediaObjectUrl).mockReset();
    vi.mocked(createManagedMediaObjectUrl).mockImplementation(async (assetId) => `blob:${assetId}`);
    vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(() => Promise.resolve());
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  });

  afterEach(() => vi.restoreAllMocks());

  it("shows the podcast's chapters and every turn with its host", async () => {
    const onClose = open();
    expect(await screen.findByRole("heading", { name: "From filtration to urine" })).toBeTruthy();
    expect(getPodcast).toHaveBeenCalledWith("podcast-1");
    expect(screen.getByText("2 lessons · Ada and Ben · 14 min")).toBeTruthy();
    expect(screen.getByRole("button", { name: "1 · Filtration" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Ben: So where does it start?" })).toBeTruthy();
    expect(narratePodcastTurn).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /Renal physiology/ }));
    expect(onClose).toHaveBeenCalled();
  });

  it("plays turn after turn in each host's voice, making the next turns ahead", async () => {
    open();
    fireEvent.click(await screen.findByRole("button", { name: "Play" }));
    await screen.findByRole("button", { name: "Pause" });
    expect(audio().src).toBe("blob:turn:0.0");
    // The next two turns are made while the first plays.
    await vi.waitFor(() => expect(narratePodcastTurn).toHaveBeenCalledTimes(3));
    expect(screen.getByRole("button", { name: /^Ada: Welcome/ }).getAttribute("aria-current")).toBe("true");

    act(() => audio().dispatchEvent(new Event("ended")));
    await vi.waitFor(() => expect(audio().src).toBe("blob:turn:0.1"));
    // Kept turns are not asked for again.
    expect(narratePodcastTurn).toHaveBeenCalledTimes(3);

    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(screen.getByRole("button", { name: "Play" })).toBeTruthy();
  });

  it("plays from a chosen chapter or turn and changes speed", async () => {
    open();
    fireEvent.click(await screen.findByRole("button", { name: "2 · The loop of Henle" }));
    await vi.waitFor(() => expect(audio().src).toBe("blob:turn:1.0"));

    fireEvent.click(screen.getByRole("button", { name: "Ben: So where does it start?" }));
    await vi.waitFor(() => expect(audio().src).toBe("blob:turn:0.1"));

    fireEvent.click(screen.getByRole("button", { name: "Speed 1 times" }));
    expect(audio().playbackRate).toBe(1.25);
    expect(screen.getByRole("button", { name: "Speed 1.25 times" })).toBeTruthy();
  });

  it("says when a turn cannot be spoken", async () => {
    vi.mocked(narratePodcastTurn).mockRejectedValue(new Error("Spoken narration needs an OpenRouter connection."));
    open();
    fireEvent.click(await screen.findByRole("button", { name: "Play" }));
    expect(
      await screen.findByText("The podcast could not be played: Spoken narration needs an OpenRouter connection."),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Play" })).toBeTruthy();
  });
});
