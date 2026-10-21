import { useEffect, useRef, useState } from "react";

import { narrateLessonStep, type LessonStepNarration } from "../../ipc/lessons";
import {
  createManagedMediaObjectUrl,
  playbackErrorMessage,
  revokeManagedMediaObjectUrl,
} from "../../media/managedPlayback";

export type Listening = "idle" | "preparing" | "playing";

/**
 * Speaks a lesson's saved text through one hidden audio element: a step's
 * narration, or with `speak` any part the lesson keeps, such as the teacher's
 * lecture. The first play asks the lesson voice and keeps its audio; later
 * plays use what was kept. Unmounting stops the voice.
 */
export function useNarrationPlayer(lessonId: string, revisionNumber: number) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const urls = useRef<string[]>([]);
  // Each play has its own number, so a stopped one never plays late.
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
    setError(playbackErrorMessage(failure));
    setListening("idle");
  };

  const playFrom = (index: number, id: number, onFinished?: () => void) => {
    const audio = audioRef.current;
    if (!audio || id !== run.current) return;
    const url = urls.current[index];
    if (url === undefined) {
      release();
      setListening("idle");
      onFinished?.();
      return;
    }
    audio.src = url;
    audio.onended = () => playFrom(index + 1, id, onFinished);
    audio.play().catch((failure: unknown) => fail(id, failure));
  };

  /**
   * Speaks what `narrate` returns from its start. `onFinished` runs only when
   * the last piece ends, never after Stop or a failure.
   */
  const speak = (narrate: () => Promise<LessonStepNarration>, onFinished?: () => void) => {
    const id = ++run.current;
    const audio = audioRef.current;
    if (audio) {
      audio.onended = null;
      audio.pause();
    }
    release();
    setError(null);
    setListening("preparing");
    narrate()
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
          playFrom(0, id, onFinished);
        },
        (failure: unknown) => fail(id, failure),
      );
  };

  /** Speaks one step's narration from its start. */
  const play = (stepKey: string, onFinished?: () => void) =>
    speak(() => narrateLessonStep(lessonId, revisionNumber, stepKey), onFinished);

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

  return { audioRef, listening, error, play, speak, stop };
}
