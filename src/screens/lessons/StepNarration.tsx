import { useEffect, useRef, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import { narrateLessonStep } from "../../ipc/lessons";
import { createManagedMediaObjectUrl, revokeManagedMediaObjectUrl } from "../../media/managedPlayback";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";

type Listening = "idle" | "preparing" | "playing";

/**
 * Speaks one step's narration. The first listen asks the lesson voice and
 * keeps its audio; later listens play what was kept. Remount it for another
 * step, so moving on stops the voice.
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
  const audioRef = useRef<HTMLAudioElement>(null);
  const urls = useRef<string[]>([]);
  // Each listen has its own number, so a stopped one never plays late.
  const run = useRef(0);
  const [listening, setListening] = useState<Listening>("idle");
  const [error, setError] = useState<string | null>(null);

  const release = () => {
    urls.current.forEach((url) => revokeManagedMediaObjectUrl(url));
    urls.current = [];
  };

  useEffect(() => {
    const audio = audioRef.current;
    return () => {
      run.current += 1;
      audio?.pause();
      release();
    };
  }, []);

  const fail = (id: number, failure: unknown) => {
    if (id !== run.current) return;
    release();
    setError(getErrorMessage(failure));
    setListening("idle");
  };

  const playFrom = (index: number, id: number) => {
    const audio = audioRef.current;
    if (!audio || id !== run.current) return;
    const url = urls.current[index];
    if (url === undefined) {
      release();
      setListening("idle");
      return;
    }
    audio.src = url;
    audio.onended = () => playFrom(index + 1, id);
    audio.play().catch((failure: unknown) => fail(id, failure));
  };

  const listen = () => {
    const id = ++run.current;
    setError(null);
    setListening("preparing");
    narrateLessonStep(lessonId, revisionNumber, stepKey)
      .then((narration) =>
        Promise.all(narration.segments.map((segment) => createManagedMediaObjectUrl(segment.assetId, "audio"))),
      )
      .then(
        (made) => {
          if (id !== run.current) {
            made.forEach((url) => revokeManagedMediaObjectUrl(url));
            return;
          }
          urls.current = made;
          setListening("playing");
          playFrom(0, id);
        },
        (failure: unknown) => fail(id, failure),
      );
  };

  const stop = () => {
    run.current += 1;
    const audio = audioRef.current;
    if (audio) {
      audio.onended = null;
      audio.pause();
      audio.removeAttribute("src");
    }
    release();
    setListening("idle");
  };

  return (
    <div className="tw:flex tw:flex-col tw:items-start tw:gap-2">
      {listening === "idle" ? (
        <Button size="small" className="tw:rounded-full" onClick={listen}>
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
