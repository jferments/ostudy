import { useEffect, useMemo, useRef, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import { narratePodcastTurn, type Podcast, type PodcastTurn } from "../../ipc/podcasts";
import { createManagedMediaObjectUrl, revokeManagedMediaObjectUrl } from "../../media/managedPlayback";

export type PodcastPlayback = "idle" | "preparing" | "playing" | "paused";

/** Playback speeds, in the order the speed button steps through them. */
export const PODCAST_SPEEDS = [1, 1.25, 1.5, 0.75] as const;
/** How far the skip buttons move, in seconds. */
export const PODCAST_SKIP_SECONDS = 15;
/** Turns made ahead of the one playing, so the next one starts at once. */
const MADE_AHEAD = 2;

export interface PlayableTurn extends PodcastTurn {
  chapter: number;
}

/**
 * Plays a podcast turn after turn through one hidden audio element. Each turn
 * is spoken in its host's voice the first time it plays and kept; the next
 * turns are made while one plays. Unmounting stops it and frees the audio.
 */
export function usePodcastPlayer(podcast: Podcast | null) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const turns = useMemo<PlayableTurn[]>(
    () =>
      podcast?.chapters.flatMap((chapter, index) => chapter.turns.map((turn) => ({ ...turn, chapter: index }))) ?? [],
    [podcast],
  );
  const audio = useRef(new Map<string, Promise<string[]>>());
  const urls = useRef<string[]>([]);
  // Each play has its own number, so a stopped one never plays late.
  const run = useRef(0);
  const speed = useRef<number>(PODCAST_SPEEDS[0]);
  const [index, setIndex] = useState(0);
  const [playback, setPlayback] = useState<PodcastPlayback>("idle");
  const [rate, setRate] = useState<number>(PODCAST_SPEEDS[0]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const element = audioRef.current;
    const kept = audio.current;
    const made = urls.current;
    return () => {
      run.current += 1;
      element?.pause();
      kept.clear();
      made.forEach((url) => revokeManagedMediaObjectUrl(url));
    };
  }, []);

  /** The turn's audio, asked for once; a failed request may be asked again. */
  const audioFor = (position: number): Promise<string[]> | null => {
    const turn = turns[position];
    if (!podcast || !turn) return null;
    let found = audio.current.get(turn.key);
    if (!found) {
      found = narratePodcastTurn(podcast.id, turn.key)
        .then((narration) =>
          Promise.all(narration.segments.map((segment) => createManagedMediaObjectUrl(segment.assetId, "audio"))),
        )
        .then((made) => {
          urls.current.push(...made);
          return made;
        });
      found.catch(() => audio.current.delete(turn.key));
      audio.current.set(turn.key, found);
    }
    return found;
  };

  const fail = (id: number, failure: unknown) => {
    if (id !== run.current) return;
    setError(getErrorMessage(failure));
    setPlayback("idle");
  };

  const playPiece = (position: number, pieces: string[], piece: number, id: number) => {
    const element = audioRef.current;
    if (!element || id !== run.current) return;
    const url = pieces[piece];
    if (url === undefined) {
      if (position + 1 < turns.length) playTurn(position + 1);
      else {
        setPlayback("idle");
        setIndex(0);
      }
      return;
    }
    element.src = url;
    element.playbackRate = speed.current;
    element.onended = () => playPiece(position, pieces, piece + 1, id);
    element.play().then(
      () => {
        if (id === run.current) setPlayback("playing");
      },
      (failure: unknown) => fail(id, failure),
    );
  };

  /** Plays from the start of a turn, then on to the end of the podcast. */
  const playTurn = (position: number) => {
    const id = ++run.current;
    const element = audioRef.current;
    if (element) {
      element.onended = null;
      element.pause();
    }
    setIndex(position);
    setError(null);
    setPlayback("preparing");
    const made = audioFor(position);
    if (!made) {
      setPlayback("idle");
      return;
    }
    made.then(
      (pieces) => {
        if (id !== run.current) return;
        playPiece(position, pieces, 0, id);
        for (let ahead = 1; ahead <= MADE_AHEAD; ahead += 1) {
          audioFor(position + ahead)?.catch(() => undefined);
        }
      },
      (failure: unknown) => fail(id, failure),
    );
  };

  const pause = () => {
    audioRef.current?.pause();
    setPlayback("paused");
  };

  const resume = () => {
    const element = audioRef.current;
    if (!element) return;
    const id = run.current;
    element.play().then(
      () => {
        if (id === run.current) setPlayback("playing");
      },
      (failure: unknown) => fail(id, failure),
    );
  };

  const toggle = () => {
    if (playback === "playing") pause();
    else if (playback === "paused") resume();
    else if (playback === "idle") playTurn(index);
  };

  /** Moves within the turn playing, or to the next or previous turn past its ends. */
  const skip = (seconds: number) => {
    const element = audioRef.current;
    if (!element || playback === "idle" || playback === "preparing") {
      playTurn(Math.min(turns.length - 1, Math.max(0, index + Math.sign(seconds))));
      return;
    }
    const target = element.currentTime + seconds;
    if (target < 0) playTurn(Math.max(0, index - 1));
    else if (Number.isFinite(element.duration) && target >= element.duration) {
      if (index + 1 < turns.length) playTurn(index + 1);
    } else element.currentTime = target;
  };

  /** Steps to the next speed. */
  const changeSpeed = () => {
    const speeds: readonly number[] = PODCAST_SPEEDS;
    const next = speeds[(speeds.indexOf(speed.current) + 1) % speeds.length] ?? PODCAST_SPEEDS[0];
    speed.current = next;
    setRate(next);
    if (audioRef.current) audioRef.current.playbackRate = next;
  };

  return { audioRef, turns, index, playback, rate, error, playTurn, toggle, skip, changeSpeed };
}
