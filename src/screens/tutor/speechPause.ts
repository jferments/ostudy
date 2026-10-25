/** Loudness a quiet room stays under once noise suppression has run. */
const MIN_SPEAKING_LEVEL = 0.06;
/** Speech must last this long before a pause can end the recording. */
const MIN_SPEECH_MS = 250;

/**
 * Watches the microphone's loudness and says when the learner has spoken and
 * then stayed quiet for `pauseMs`. Silence before any speech never ends the
 * recording, so a learner can take a moment to start. The room's own noise
 * raises the bar for what counts as speech.
 */
export function createPauseDetector(pauseMs: number) {
  let floor: number | null = null;
  let speechStarted: number | null = null;
  let heard = false;
  let lastVoice = 0;

  return (level: number, now: number): boolean => {
    // The quietest moment so far stands for the room.
    floor = floor === null ? level : Math.min(level, floor);
    const speaking = level >= Math.max(MIN_SPEAKING_LEVEL, floor * 3);
    if (speaking) {
      speechStarted ??= now;
      if (now - speechStarted >= MIN_SPEECH_MS) heard = true;
      lastVoice = now;
      return false;
    }
    // A short quiet stretch before speech counted resets it: a click is not speech.
    if (!heard && now - lastVoice > MIN_SPEECH_MS) speechStarted = null;
    return heard && now - lastVoice >= pauseMs;
  };
}
