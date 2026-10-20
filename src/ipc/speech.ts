import { invoke } from "./transport";

export interface SpeechModel {
  id: string;
  label: string;
  /** Empty when the model chooses its own voice. */
  voices: string[];
  defaultVoice: string;
}

export interface SpeechConnection {
  id: string;
  name: string;
}

/** Which voice speaks lesson narration, and the choices for it. */
export interface SpeechSettings {
  /** Null when no OpenRouter connection can speak. */
  profileId: string | null;
  modelId: string;
  voice: string;
  /** 0 until the setting is first saved. */
  revision: number;
  models: SpeechModel[];
  connections: SpeechConnection[];
}

export async function getSpeechSettings(): Promise<SpeechSettings> {
  return invoke<SpeechSettings>("get_speech_settings");
}

/** Saves the lesson voice; refused when it changed since `expectedRevision`. */
export async function saveSpeechSettings(
  profileId: string | null,
  modelId: string,
  voice: string,
  expectedRevision: number,
): Promise<SpeechSettings> {
  return invoke<SpeechSettings>("save_speech_settings", {
    input: { profileId, modelId, voice, expectedRevision },
  });
}
