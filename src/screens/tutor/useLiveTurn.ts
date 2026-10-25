import { useEffect, useState } from "react";

import { getLearningTaskProgress } from "../../ipc/knowledge";

/** How often a running turn's progress is read again. */
const POLL_MS = 200;

export interface LiveTurn {
  /** What the turn is doing now, in the learner's words. */
  status: string;
  /** The explanation as far as it has been written; null until it starts or for answers that wait for their checks. */
  draft: string | null;
}

const STATUSES: Record<string, string> = {
  "reading conversation": "Reading the conversation…",
  "resolving study context": "Finding sources…",
  "writing response": "Writing…",
  "checking response": "Checking the answer…",
  "checking hint for spoilers": "Checking the hint…",
  "response saved": "Checking the answer…",
};

/**
 * The live progress of the turn being answered, read while `operationId` is
 * set. One read runs at a time, the next after it settles, and a read that
 * finishes after the turn ends is dropped.
 */
export function useLiveTurn(operationId: string | null): LiveTurn | null {
  const [live, setLive] = useState<(LiveTurn & { operationId: string }) | null>(null);

  useEffect(() => {
    if (operationId === null) return;
    let current = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const read = () => {
      void getLearningTaskProgress(operationId)
        .then((progress) => {
          if (!current || !progress) return;
          setLive({ operationId, status: STATUSES[progress.phase] ?? "Thinking…", draft: progress.draft || null });
        })
        .catch(() => undefined)
        .finally(() => {
          if (current) timer = setTimeout(read, POLL_MS);
        });
    };
    read();
    return () => {
      current = false;
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [operationId]);

  return live && live.operationId === operationId ? live : null;
}
