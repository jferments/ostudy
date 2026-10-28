import { getDeviceVoices, getSpeechSettings, type DeviceVoices, type SpeechSettings } from "../../ipc/speech";

/**
 * Whether the lesson voice can speak now: a voice on this device once it is
 * downloaded, or a provider's voice through its connection.
 */
export function voiceReady(speech: SpeechSettings, device: DeviceVoices | null): boolean {
  const model = speech.models.find((candidate) => candidate.id === speech.modelId);
  if (model?.onDevice) {
    return device?.models.some((status) => status.modelId === model.id && status.installed) === true;
  }
  return speech.profileId !== null;
}

/** The lesson voice setting and whether it can speak now. */
export async function loadVoice(): Promise<{ speech: SpeechSettings; ready: boolean }> {
  const [speech, device] = await Promise.all([getSpeechSettings(), getDeviceVoices().catch(() => null)]);
  return { speech, ready: voiceReady(speech, device) };
}
