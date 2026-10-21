import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { useNarrationPlayer } from "./useNarrationPlayer";
import { useVoiceAvailable } from "./useVoiceAvailable";

/**
 * Speaks one step's narration. The first listen asks the lesson voice and
 * keeps its audio; later listens play what was kept. Remount it for another
 * step, so moving on stops the voice. Nothing shows until a lesson voice is
 * set up.
 */
export function StepNarration({
  lessonId,
  revisionNumber,
  stepKey,
}: {
  lessonId: string;
  revisionNumber: number;
  stepKey: string;
}) {
  const { audioRef, listening, error, play, stop } = useNarrationPlayer(lessonId, revisionNumber);
  const voiceAvailable = useVoiceAvailable();
  if (!voiceAvailable) return null;

  return (
    <div className="tw:flex tw:flex-col tw:items-start tw:gap-2">
      {listening === "idle" ? (
        <Button size="small" className="tw:rounded-full" onClick={() => play(stepKey)}>
          <Icon name="listen" size={16} /> Listen
        </Button>
      ) : (
        // Preparing can take a while, so it stays stoppable; what the voice
        // finishes is still kept for the next listen.
        <Button size="small" className="tw:rounded-full" onClick={stop}>
          <Icon name="stop" size={16} /> {listening === "preparing" ? "Preparing…" : "Stop"}
        </Button>
      )}
      {error ? <InlineNotice kind="error">This step could not be read aloud: {error}</InlineNotice> : null}
      <audio ref={audioRef} className="tw:hidden" />
    </div>
  );
}
