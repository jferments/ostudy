import { useCallback, useEffect, useRef, useState } from "react";

import { speakTutorReply, type SpokenPiece } from "../../ipc/tutor";
import {
  createManagedMediaObjectUrl,
  playbackErrorMessage,
  revokeManagedMediaObjectUrl,
} from "../../media/managedPlayback";

export interface ReadAloud {
  audioRef: React.RefObject<HTMLAudioElement | null>;
  /** The reply being read, if any. */
  turnId: string | null;
  /** Why the last reading stopped early, with the reply it was for. */
  error: { turnId: string; message: string } | null;
  /** Reads a reply from its start, or stops it when it is the one being read. */
  toggle: (turnId: string) => void;
  stop: () => void;
}

interface Piece {
  url: string;
  pieces: number;
}

/**
 * Reads finished Tutor replies aloud through one hidden audio element, piece
 * by piece: the next piece is asked for while the current one plays, so a long
 * reply starts as soon as its first piece is spoken. Unmounting stops it.
 */
export function useReadAloud(): ReadAloud {
  const audioRef = useRef<HTMLAudioElement>(null);
  // Each reading has its own number, so a stopped one never plays late.
  const run = useRef(0);
  const urls = useRef<string[]>([]);
  const [turnId, setTurnId] = useState<string | null>(null);
  const [error, setError] = useState<ReadAloud["error"]>(null);

  // Stable, so a caller can stop the reading from an effect's cleanup.
  const halt = useCallback(() => {
    run.current += 1;
    const audio = audioRef.current;
    if (audio) {
      audio.onended = null;
      if (!audio.paused) audio.pause();
      audio.removeAttribute("src");
    }
    urls.current.forEach((url) => revokeManagedMediaObjectUrl(url));
    urls.current = [];
  }, []);

  useEffect(() => halt, [halt]);

  const stop = useCallback(() => {
    halt();
    setTurnId(null);
  }, [halt]);

  const read = (id: string) => {
    halt();
    const reading = run.current;
    const current = () => reading === run.current;
    setTurnId(id);
    setError(null);

    const fetchPiece = (index: number): Promise<Piece> =>
      speakTutorReply(id, index).then((spoken: SpokenPiece) =>
        createManagedMediaObjectUrl(spoken.segment.assetId, "audio").then((url) => {
          if (current()) urls.current.push(url);
          else revokeManagedMediaObjectUrl(url);
          return { url, pieces: spoken.pieces };
        }),
      );

    const fail = (failure: unknown) => {
      if (!current()) return;
      halt();
      setError({ turnId: id, message: playbackErrorMessage(failure) });
      setTurnId(null);
    };

    const play = (index: number, piece: Promise<Piece>) => {
      piece.then((ready) => {
        const audio = audioRef.current;
        if (!current() || !audio) return;
        const next = index + 1 < ready.pieces ? fetchPiece(index + 1) : null;
        // A failed next piece is reported when it is due, not while this one plays.
        next?.catch(() => undefined);
        audio.src = ready.url;
        audio.onended = () => {
          if (next) play(index + 1, next);
          else stop();
        };
        audio.play().catch(fail);
      }, fail);
    };

    play(0, fetchPiece(0));
  };

  const toggle = (id: string) => {
    if (turnId === id) stop();
    else read(id);
  };

  return { audioRef, turnId, error, toggle, stop };
}
