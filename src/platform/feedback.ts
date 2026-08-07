import { getPlatformCapabilities } from "./capabilities";

export type FeedbackStrength = "light" | "medium" | "strong";

export interface AudioFeedback {
  play(cue: string): Promise<void>;
}

export interface HapticFeedback {
  pulse(strength: FeedbackStrength): Promise<void>;
}

export const audioFeedback: AudioFeedback = {
  async play(_cue: string): Promise<void> {
    // Stage 3.13 establishes the platform seam. Bundled sound assets and user
    // volume preferences belong to the later experience-feature stage.
  },
};

export const hapticFeedback: HapticFeedback = {
  async pulse(strength: FeedbackStrength): Promise<void> {
    const capabilities = getPlatformCapabilities();
    if (!capabilities.hapticsAvailable || typeof navigator === "undefined") return;
    const duration = strength === "light" ? 8 : strength === "medium" ? 16 : 28;
    navigator.vibrate(duration);
  },
};
