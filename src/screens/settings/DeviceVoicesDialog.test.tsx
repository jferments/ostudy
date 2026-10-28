import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

import { getDeviceVoices, installDeviceVoice, type SpeechSettings } from "../../ipc/speech";
import { DeviceVoicesDialog } from "./DeviceVoicesDialog";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/speech", () => ({
  getDeviceVoices: vi.fn(),
  installDeviceVoice: vi.fn(),
  saveSpeechSettings: vi.fn(),
}));
vi.mock("../../media/managedPlayback", () => ({
  playbackErrorMessage: (failure: unknown) => String(failure),
}));

const KOKORO = "local/kokoro-82m";

const settings: SpeechSettings = {
  profileId: null,
  modelId: "",
  voice: "",
  revision: 0,
  models: [
    { id: KOKORO, label: "Kokoro", voices: ["af_heart"], defaultVoice: "af_heart", clonesVoices: false, onDevice: true },
  ],
  connections: [],
  voices: [],
};

afterEach(() => {
  vi.useRealTimers();
});

it("stops reading the download's progress once the dialog closes", async () => {
  vi.mocked(getDeviceVoices).mockResolvedValue({
    supported: true,
    models: [
      { modelId: KOKORO, label: "Kokoro", summary: "", downloadBytes: 1, sample: "/voices/device/kokoro.mp3", installed: false, installingBytes: null, installError: null },
    ],
  });
  vi.mocked(installDeviceVoice).mockReturnValue(new Promise(() => undefined));
  const view = render(<DeviceVoicesDialog settings={settings} onChanged={vi.fn()} onClose={vi.fn()} />);
  const download = await screen.findByRole("button", { name: "Download and use" });
  const reads = vi.mocked(getDeviceVoices).mock.calls.length;

  vi.useFakeTimers();
  fireEvent.click(download);
  view.unmount();
  vi.advanceTimersByTime(1_000);

  expect(installDeviceVoice).toHaveBeenCalledWith(KOKORO);
  expect(getDeviceVoices).toHaveBeenCalledTimes(reads);
});
