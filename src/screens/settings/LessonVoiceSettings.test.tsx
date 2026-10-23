import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getSpeechSettings,
  keepSpeechVoice,
  removeSpeechVoice,
  renameSpeechVoice,
  saveSpeechSettings,
  trySpeechVoice,
  type SpeechSettings,
  type SpeechVoice,
} from "../../ipc/speech";
import { createManagedMediaObjectUrl } from "../../media/managedPlayback";
import { LessonVoiceSettings, voiceLabel } from "./LessonVoiceSettings";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/speech", () => ({
  getSpeechSettings: vi.fn(),
  saveSpeechSettings: vi.fn(),
  trySpeechVoice: vi.fn(),
  keepSpeechVoice: vi.fn(),
  renameSpeechVoice: vi.fn(),
  removeSpeechVoice: vi.fn(),
}));
vi.mock("../../media/managedPlayback", () => ({
  createManagedMediaObjectUrl: vi.fn(async (assetId: string) => `blob:${assetId}`),
  revokeManagedMediaObjectUrl: vi.fn(),
  playbackErrorMessage: (failure: unknown) => (failure instanceof Error ? failure.message : String(failure)),
}));

const router = "a".repeat(32);
const DRAMA = "fish-audio/drama-3";

function voice(id: string, name: string, state: SpeechVoice["state"] = "saved"): SpeechVoice {
  const key = id.repeat(32);
  return {
    id: key,
    name,
    modelId: DRAMA,
    assetId: `asset-${id}`,
    state,
    voice: `saved:${key}`,
  };
}

function settings(overrides: Partial<SpeechSettings> = {}): SpeechSettings {
  return {
    profileId: router,
    modelId: "hexgrad/kokoro-82m",
    voice: "af_heart",
    revision: 0,
    models: [
      {
        id: "hexgrad/kokoro-82m",
        label: "Kokoro",
        voices: ["af_heart", "bm_george"],
        defaultVoice: "af_heart",
        clonesVoices: false,
      },
      {
        id: DRAMA,
        label: "Drama 3",
        voices: [],
        defaultVoice: "",
        clonesVoices: true,
      },
    ],
    connections: [{ id: router, name: "OpenRouter" }],
    voices: [],
    ...overrides,
  };
}

function select(name: string): HTMLSelectElement {
  return screen.getByRole("combobox", { name }) as HTMLSelectElement;
}

describe("LessonVoiceSettings", () => {
  beforeEach(() => {
    vi.mocked(getSpeechSettings).mockReset();
    vi.mocked(saveSpeechSettings).mockReset();
    vi.mocked(trySpeechVoice).mockReset();
    vi.mocked(keepSpeechVoice).mockReset();
    vi.mocked(renameSpeechVoice).mockReset();
    vi.mocked(removeSpeechVoice).mockReset();
    vi.mocked(createManagedMediaObjectUrl).mockClear();
    vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  });

  it("offers lecture audio with new lessons once a voice is chosen", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue(settings());
    const onChange = vi.fn();
    const view = render(<LessonVoiceSettings connectionsKey={1} lectureAudio={{ checked: true, onChange }} />);
    const toggle = await screen.findByRole("switch", { name: "Make lecture audio with new lessons" });
    fireEvent.click(toggle);
    expect(onChange.mock.calls[0]?.[0]).toBe(false);
    view.unmount();

    vi.mocked(getSpeechSettings).mockResolvedValue(settings({ profileId: null }));
    render(<LessonVoiceSettings connectionsKey={2} lectureAudio={{ checked: true, onChange }} />);
    await screen.findByRole("combobox", { name: "Lesson voice connection" });
    expect(screen.queryByRole("switch", { name: "Make lecture audio with new lessons" })).toBeNull();
  });

  it("saves a voice against the revision it showed", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue(settings());
    vi.mocked(saveSpeechSettings).mockResolvedValue(settings({ voice: "bm_george", revision: 1 }));
    render(<LessonVoiceSettings connectionsKey={1} />);

    const voice = await screen.findByRole("combobox", { name: "Lesson voice" });
    expect([...(voice as HTMLSelectElement).options].map((option) => option.text)).toEqual([
      "Heart (American female)",
      "George (British male)",
    ]);
    fireEvent.change(voice, { target: { value: "bm_george" } });
    expect(saveSpeechSettings).toHaveBeenCalledWith(router, "hexgrad/kokoro-82m", "bm_george", 0);
    await waitFor(() => expect(select("Lesson voice").value).toBe("bm_george"));
  });

  it("switching to a model that copies voices with none saved offers to try one", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue(settings({ revision: 2 }));
    vi.mocked(saveSpeechSettings).mockResolvedValue(
      settings({ modelId: "fish-audio/drama-3", voice: "", revision: 3 }),
    );
    render(<LessonVoiceSettings connectionsKey={1} />);

    fireEvent.change(await screen.findByRole("combobox", { name: "Lesson voice model" }), {
      target: { value: "fish-audio/drama-3" },
    });
    expect(saveSpeechSettings).toHaveBeenCalledWith(router, "fish-audio/drama-3", "", 2);
    await waitFor(() => expect(screen.queryByRole("combobox", { name: "Lesson voice" })).toBeNull());
    expect(screen.getByRole("button", { name: "Try a voice" })).toBeTruthy();
  });

  it("switching to a model that copies voices chooses the first saved one", async () => {
    const first = voice("1", "Voice 1");
    vi.mocked(getSpeechSettings).mockResolvedValue(settings({ revision: 2, voices: [first, voice("2", "Voice 2")] }));
    vi.mocked(saveSpeechSettings).mockResolvedValue(settings());
    render(<LessonVoiceSettings connectionsKey={1} />);

    fireEvent.change(await screen.findByRole("combobox", { name: "Lesson voice model" }), {
      target: { value: DRAMA },
    });
    expect(saveSpeechSettings).toHaveBeenCalledWith(router, DRAMA, first.voice, 2);
  });

  it("tries a voice, plays it and keeps it", async () => {
    const tried = voice("3", "New voice", "tried");
    const kept = { ...tried, name: "Voice 1", state: "saved" as const };
    vi.mocked(getSpeechSettings).mockResolvedValue(settings({ modelId: DRAMA, voice: "" }));
    vi.mocked(trySpeechVoice).mockResolvedValue(settings({ modelId: DRAMA, voice: "", voices: [tried] }));
    vi.mocked(keepSpeechVoice).mockResolvedValue(
      settings({
        modelId: DRAMA,
        voice: kept.voice,
        revision: 1,
        voices: [kept],
      }),
    );
    render(<LessonVoiceSettings connectionsKey={1} />);

    fireEvent.click(await screen.findByRole("button", { name: "Try a voice" }));
    expect(trySpeechVoice).toHaveBeenCalledWith(null);
    const dialog = await screen.findByRole("dialog", { name: "Lesson voices" });
    await waitFor(() => expect(createManagedMediaObjectUrl).toHaveBeenCalledWith("asset-3", "audio"));

    fireEvent.click(within(dialog).getByRole("button", { name: "Keep this voice" }));
    expect(keepSpeechVoice).toHaveBeenCalledWith(tried.id);
    expect(await within(dialog).findByText("In use")).toBeTruthy();
    expect(within(dialog).getByRole("list", { name: "Your voices" }).textContent).toContain("Voice 1");
    expect(within(dialog).queryByRole("button", { name: "Keep this voice" })).toBeNull();

    // A description shapes the next voice tried.
    const described = { ...voice("4", "a deep calm voice", "tried") };
    vi.mocked(trySpeechVoice).mockResolvedValueOnce(
      settings({
        modelId: DRAMA,
        voice: kept.voice,
        revision: 1,
        voices: [kept, described],
      }),
    );
    fireEvent.change(
      within(dialog).getByRole("textbox", {
        name: "Describe the voice (optional)",
      }),
      {
        target: { value: " a [deep] calm voice " },
      },
    );
    fireEvent.click(within(dialog).getByRole("button", { name: "Try another voice" }));
    expect(trySpeechVoice).toHaveBeenLastCalledWith("a deep calm voice");
    expect(await within(dialog).findByText("a deep calm voice")).toBeTruthy();

    fireEvent.click(within(dialog).getByRole("button", { name: "Done" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(select("Lesson voice").value).toBe(kept.voice);
  });

  it("chooses, renames and removes saved voices, never the one in use", async () => {
    const first = voice("1", "Voice 1");
    const second = voice("2", "Voice 2");
    const current = settings({
      modelId: DRAMA,
      voice: first.voice,
      revision: 3,
      voices: [first, second],
    });
    vi.mocked(getSpeechSettings).mockResolvedValue(current);
    vi.mocked(keepSpeechVoice).mockResolvedValue({
      ...current,
      voice: second.voice,
      revision: 4,
    });
    vi.mocked(renameSpeechVoice).mockResolvedValue({
      ...current,
      voice: second.voice,
      revision: 4,
      voices: [{ ...first, name: "Ms. Rivera" }, second],
    });
    vi.mocked(removeSpeechVoice).mockResolvedValue({
      ...current,
      voice: second.voice,
      revision: 4,
      voices: [second],
    });
    render(<LessonVoiceSettings connectionsKey={1} />);

    await waitFor(() =>
      expect([...select("Lesson voice").options].map((option) => option.text)).toEqual(["Voice 1", "Voice 2"]),
    );
    fireEvent.click(screen.getByRole("button", { name: "More for Voice" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Lesson voices…" }));
    const dialog = await screen.findByRole("dialog", { name: "Lesson voices" });
    expect(trySpeechVoice).not.toHaveBeenCalled();

    fireEvent.click(within(dialog).getByRole("button", { name: "More for Voice 1" }));
    expect((await screen.findByRole("menuitem", { name: "Remove" })).getAttribute("aria-disabled")).toBe("true");
    fireEvent.keyDown(document.activeElement ?? document.body, {
      key: "Escape",
    });

    fireEvent.click(within(dialog).getByRole("button", { name: "Use" }));
    expect(keepSpeechVoice).toHaveBeenCalledWith(second.id);
    await waitFor(() => expect(within(dialog).getAllByRole("button", { name: "Use" })).toHaveLength(1));

    fireEvent.click(within(dialog).getByRole("button", { name: "More for Voice 1" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Rename…" }));
    const name = within(dialog).getByRole("textbox", {
      name: "Name for Voice 1",
    });
    fireEvent.change(name, { target: { value: "Ms. Rivera" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    expect(renameSpeechVoice).toHaveBeenCalledWith(first.id, "Ms. Rivera");
    expect(await within(dialog).findByText("Ms. Rivera")).toBeTruthy();

    fireEvent.click(within(dialog).getByRole("button", { name: "More for Ms. Rivera" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
    expect(removeSpeechVoice).toHaveBeenCalledWith(first.id);
    await waitFor(() => expect(within(dialog).queryByText("Ms. Rivera")).toBeNull());
  });

  it("shows what is saved after a refused save", async () => {
    vi.mocked(getSpeechSettings)
      .mockResolvedValueOnce(settings())
      .mockResolvedValueOnce(settings({ voice: "bm_george", revision: 4 }));
    vi.mocked(saveSpeechSettings).mockRejectedValue(new Error("The voice setting changed elsewhere."));
    render(<LessonVoiceSettings connectionsKey={1} />);

    fireEvent.change(await screen.findByRole("combobox", { name: "Lesson voice connection" }), {
      target: { value: "" },
    });
    expect(saveSpeechSettings).toHaveBeenCalledWith(null, "hexgrad/kokoro-82m", "af_heart", 0);
    expect(
      await screen.findByText(/The lesson voice was not saved: The voice setting changed elsewhere\./),
    ).toBeTruthy();
    await waitFor(() => expect(select("Lesson voice").value).toBe("bm_george"));
  });

  it("names a library with no OpenRouter connection plainly", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue(settings({ profileId: null, connections: [] }));
    render(<LessonVoiceSettings connectionsKey={1} />);
    const connection = await screen.findByRole("combobox", {
      name: "Lesson voice connection",
    });
    expect((connection as HTMLSelectElement).options[0]?.text).toBe("No OpenRouter service");
    expect(screen.getByRole("listitem").textContent).toContain("Voice");
  });

  it("shows Voice as off with Add OpenRouter when no connection can speak", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue(settings({ profileId: null, connections: [] }));
    const onAddOpenRouter = vi.fn();
    render(<LessonVoiceSettings connectionsKey={1} onAddOpenRouter={onAddOpenRouter} />);
    fireEvent.click(await screen.findByRole("button", { name: "Add OpenRouter" }));
    expect(onAddOpenRouter).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("listitem").textContent).toContain("Off · needs OpenRouter");
    expect(screen.queryByRole("combobox", { name: "Lesson voice connection" })).toBeNull();
  });

  it("offers the voices that come with oStudy, played from the app's own files", async () => {
    const relaxed: SpeechVoice = {
      id: "relaxed-man",
      name: "Relaxed man",
      modelId: DRAMA,
      assetId: null,
      state: "builtIn",
      voice: "builtin:relaxed-man",
    };
    const calm: SpeechVoice = {
      ...relaxed,
      id: "calm-older-woman",
      name: "Calm older woman",
      voice: "builtin:calm-older-woman",
    };
    const current = settings({ modelId: DRAMA, voice: relaxed.voice, voices: [relaxed, calm] });
    vi.mocked(getSpeechSettings).mockResolvedValue(current);
    vi.mocked(keepSpeechVoice).mockResolvedValue({ ...current, voice: calm.voice, revision: 1 });
    render(<LessonVoiceSettings connectionsKey={1} />);

    await waitFor(() =>
      expect([...select("Lesson voice").options].map((option) => option.text)).toEqual([
        "Relaxed man",
        "Calm older woman",
      ]),
    );
    expect(select("Lesson voice").value).toBe(relaxed.voice);
    fireEvent.click(screen.getByRole("button", { name: "More for Voice" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Lesson voices…" }));
    const dialog = await screen.findByRole("dialog", { name: "Lesson voices" });
    expect(within(dialog).queryByRole("button", { name: "More for Relaxed man" })).toBeNull();

    const play = vi.mocked(HTMLMediaElement.prototype.play);
    fireEvent.click(within(dialog).getByRole("button", { name: "Listen to Calm older woman" }));
    await waitFor(() => expect(play).toHaveBeenCalled());
    expect(createManagedMediaObjectUrl).not.toHaveBeenCalled();
    expect(dialog.querySelector("audio")?.getAttribute("src")).toBe("/voices/calm-older-woman.mp3");

    fireEvent.click(within(dialog).getByRole("button", { name: "Use" }));
    expect(keepSpeechVoice).toHaveBeenCalledWith("calm-older-woman");
    expect(await within(dialog).findAllByText("In use")).toHaveLength(1);
  });

  it("labels Kokoro voices and leaves other names alone", () => {
    expect(voiceLabel("af_heart")).toBe("Heart (American female)");
    expect(voiceLabel("bm_lewis")).toBe("Lewis (British male)");
    expect(voiceLabel("narrator")).toBe("narrator");
  });
});
