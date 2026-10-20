import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getSpeechSettings, saveSpeechSettings, type SpeechSettings } from "../../ipc/speech";
import { LessonVoiceSettings, voiceLabel } from "./LessonVoiceSettings";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/speech", () => ({ getSpeechSettings: vi.fn(), saveSpeechSettings: vi.fn() }));

const router = "a".repeat(32);

function settings(overrides: Partial<SpeechSettings> = {}): SpeechSettings {
  return {
    profileId: router,
    modelId: "hexgrad/kokoro-82m",
    voice: "af_heart",
    revision: 0,
    models: [
      { id: "hexgrad/kokoro-82m", label: "Kokoro", voices: ["af_heart", "bm_george"], defaultVoice: "af_heart" },
      { id: "fish-audio/drama-3", label: "Drama 3", voices: [], defaultVoice: "" },
    ],
    connections: [{ id: router, name: "OpenRouter" }],
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

  it("switching to a model that picks its own voice saves no voice and hides the choice", async () => {
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
    expect(await screen.findByText(/The lesson voice was not saved: The voice setting changed elsewhere\./)).toBeTruthy();
    await waitFor(() => expect(select("Lesson voice").value).toBe("bm_george"));
  });

  it("names a library with no OpenRouter connection plainly", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue(settings({ profileId: null, connections: [] }));
    render(<LessonVoiceSettings connectionsKey={1} />);
    const connection = await screen.findByRole("combobox", { name: "Lesson voice connection" });
    expect((connection as HTMLSelectElement).options[0]?.text).toBe("No OpenRouter service");
    expect(screen.getByRole("listitem").textContent).toContain("Voice");
  });

  it("labels Kokoro voices and leaves other names alone", () => {
    expect(voiceLabel("af_heart")).toBe("Heart (American female)");
    expect(voiceLabel("bm_lewis")).toBe("Lewis (British male)");
    expect(voiceLabel("narrator")).toBe("narrator");
  });
});
