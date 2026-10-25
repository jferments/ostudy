import { invoke } from "./transport";

/**
 * Speech input runs on this device: native code downloads a pinned speech
 * model once, checks it, and turns recordings into text. Nothing is sent to an
 * AI provider.
 */
export interface SpeechInputStatus {
  /** False where this build cannot recognize speech. */
  supported: boolean;
  modelLabel: string;
  /** What the first use downloads, in bytes. */
  downloadBytes: number;
  installed: boolean;
  /** Bytes received so far while the model downloads; null otherwise. */
  installingBytes: number | null;
  /** Why the last download failed, until another one starts. */
  installError: string | null;
}

export interface SpeechTranscript {
  /** Empty when nothing was heard. */
  text: string;
  durationMs: number;
  /** How long recognition took after the recording ended. */
  elapsedMs: number;
}

export function getSpeechInputStatus(): Promise<SpeechInputStatus> {
  return invoke<SpeechInputStatus>("get_speech_input_status");
}

/** Downloads the speech model unless it is installed; resolves when it is done. */
export function installSpeechInputModel(): Promise<SpeechInputStatus> {
  return invoke<SpeechInputStatus>("install_speech_input_model");
}

/** `samplesBase64` is mono 16-bit little-endian PCM at `sampleRate`. */
export function transcribeSpeech(samplesBase64: string, sampleRate: number): Promise<SpeechTranscript> {
  return invoke<SpeechTranscript>("transcribe_speech", { input: { samplesBase64, sampleRate } });
}
