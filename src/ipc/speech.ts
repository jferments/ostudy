import { invoke } from "./transport";

export interface SpeechModel {
  id: string;
  label: string;
  /** Empty when the model chooses its own voice. */
  voices: string[];
  defaultVoice: string;
  /** Whether the learner's own saved voices speak with this model. */
  clonesVoices: boolean;
}

/** A voice the learner saved, or the one they are trying. */
export interface SpeechVoice {
  id: string;
  name: string;
  modelId: string;
  /** The sample, to listen to; null for a voice that comes with ostudy. */
  assetId: string | null;
  state: "builtIn" | "saved" | "tried";
  /** The setting's `voice` that chooses it. */
  voice: string;
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
  /** Saved voices, oldest first, then the voice being tried. */
  voices: SpeechVoice[];
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

/**
 * Asks the chosen model, which must copy voices, for a sample in a new voice,
 * sounding as `description` says when given. It replaces the voice tried
 * before.
 */
export async function trySpeechVoice(description: string | null = null): Promise<SpeechSettings> {
  return invoke<SpeechSettings>("try_speech_voice", { input: { description } });
}

/** Saves the voice being tried, or chooses a saved one, as the lesson voice. */
export async function keepSpeechVoice(voiceId: string): Promise<SpeechSettings> {
  return invoke<SpeechSettings>("keep_speech_voice", { input: { voiceId } });
}

export async function renameSpeechVoice(voiceId: string, name: string): Promise<SpeechSettings> {
  return invoke<SpeechSettings>("rename_speech_voice", {
    input: { voiceId, name },
  });
}

/** Removes a voice not in use. Lessons spoken in it keep it. */
export async function removeSpeechVoice(voiceId: string): Promise<SpeechSettings> {
  return invoke<SpeechSettings>("remove_speech_voice", { input: { voiceId } });
}
