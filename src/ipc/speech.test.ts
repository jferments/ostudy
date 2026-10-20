import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getSpeechSettings, saveSpeechSettings } from "./speech";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

describe("speech IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("reads the lesson voice", async () => {
    await getSpeechSettings();
    expect(invoke).toHaveBeenCalledWith("get_speech_settings");
  });

  it("saves the lesson voice against the revision it was read at", async () => {
    const profileId = "a".repeat(32);
    await saveSpeechSettings(profileId, "hexgrad/kokoro-82m", "bm_george", 3);
    expect(invoke).toHaveBeenCalledWith("save_speech_settings", {
      input: {
        profileId,
        modelId: "hexgrad/kokoro-82m",
        voice: "bm_george",
        expectedRevision: 3,
      },
    });
  });
});
