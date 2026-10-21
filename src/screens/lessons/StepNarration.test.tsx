import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { narrateLessonStep } from "../../ipc/lessons";
import { getSpeechSettings } from "../../ipc/speech";
import {
  createManagedMediaObjectUrl,
  revokeManagedMediaObjectUrl,
} from "../../media/managedPlayback";
import { StepNarration } from "./StepNarration";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../../ipc/lessons", () => ({ narrateLessonStep: vi.fn() }));
vi.mock("../../ipc/speech", () => ({ getSpeechSettings: vi.fn() }));
vi.mock("../../media/managedPlayback", () => ({
  createManagedMediaObjectUrl: vi.fn(),
  revokeManagedMediaObjectUrl: vi.fn(),
  playbackErrorMessage: (failure: unknown) => (failure instanceof Error ? failure.message : String(failure)),
}));

const first = "1".repeat(32);
const second = "2".repeat(32);

function narrate() {
  render(
    <StepNarration
      lessonId="lesson-1"
      revisionNumber={2}
      stepKey="two-limbs"
    />,
  );
}

function audio(): HTMLAudioElement {
  return document.querySelector("audio")!;
}

describe("StepNarration", () => {
  let play: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.mocked(narrateLessonStep).mockReset();
    vi.mocked(getSpeechSettings).mockResolvedValue({
      profileId: "r".repeat(32),
      modelId: "fish-audio/drama-3",
      voice: "",
      revision: 0,
      models: [],
      connections: [],
      voices: [],
    });
    vi.mocked(createManagedMediaObjectUrl).mockReset();
    vi.mocked(revokeManagedMediaObjectUrl).mockReset();
    vi.mocked(createManagedMediaObjectUrl).mockImplementation(
      async (assetId) => `blob:${assetId}`,
    );
    play = vi.fn(() => Promise.resolve());
    vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(
      play as () => Promise<void>,
    );
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(
      () => undefined,
    );
  });

  afterEach(() => vi.restoreAllMocks());

  it("plays the step's pieces in order, then is ready to listen again", async () => {
    vi.mocked(narrateLessonStep).mockResolvedValue({
      segments: [
        { assetId: first, characters: 40 },
        { assetId: second, characters: 30 },
      ],
      made: 2,
    });
    narrate();
    fireEvent.click(await screen.findByRole("button", { name: "Listen" }));
    expect(screen.getByRole("button", { name: "Preparing…" })).toBeTruthy();

    await screen.findByRole("button", { name: "Stop" });
    expect(narrateLessonStep).toHaveBeenCalledWith("lesson-1", 2, "two-limbs");
    expect(audio().src).toBe(`blob:${first}`);
    expect(play).toHaveBeenCalledTimes(1);

    act(() => audio().dispatchEvent(new Event("ended")));
    expect(audio().src).toBe(`blob:${second}`);
    expect(play).toHaveBeenCalledTimes(2);

    act(() => audio().dispatchEvent(new Event("ended")));
    expect(screen.getByRole("button", { name: "Listen" })).toBeTruthy();
    expect(revokeManagedMediaObjectUrl).toHaveBeenCalledWith(`blob:${first}`);
    expect(revokeManagedMediaObjectUrl).toHaveBeenCalledWith(`blob:${second}`);
  });

  it("stops while the voice is still preparing and never plays late", async () => {
    let finish: (
      value: Awaited<ReturnType<typeof narrateLessonStep>>,
    ) => void = () => undefined;
    vi.mocked(narrateLessonStep).mockReturnValue(
      new Promise((resolve) => (finish = resolve)),
    );
    narrate();
    fireEvent.click(await screen.findByRole("button", { name: "Listen" }));
    fireEvent.click(screen.getByRole("button", { name: "Preparing…" }));
    expect(screen.getByRole("button", { name: "Listen" })).toBeTruthy();

    await act(async () =>
      finish({ segments: [{ assetId: first, characters: 10 }], made: 1 }),
    );
    await waitFor(() =>
      expect(revokeManagedMediaObjectUrl).toHaveBeenCalledWith(`blob:${first}`),
    );
    expect(play).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Listen" })).toBeTruthy();
  });

  it("shows no Listen button until a lesson voice is set up", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue({
      profileId: null,
      modelId: "fish-audio/drama-3",
      voice: "",
      revision: 0,
      models: [],
      connections: [],
      voices: [],
    });
    narrate();
    await waitFor(() => expect(getSpeechSettings).toHaveBeenCalled());
    await act(async () => undefined);
    expect(screen.queryByRole("button", { name: "Listen" })).toBeNull();
  });

  it("says why a step could not be read aloud", async () => {
    vi.mocked(narrateLessonStep).mockRejectedValue(
      new Error("Spoken narration needs an OpenRouter connection."),
    );
    narrate();
    fireEvent.click(await screen.findByRole("button", { name: "Listen" }));
    expect(
      await screen.findByText(
        /This step could not be read aloud: Spoken narration needs an OpenRouter connection\./,
      ),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Listen" })).toBeTruthy();
  });
});
