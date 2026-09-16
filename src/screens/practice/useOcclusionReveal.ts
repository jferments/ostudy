import { useEffect, useRef, useState } from "react";

import { createMutationId, getErrorMessage } from "../../ipc/common";
import {
  getPracticeQuestionAttempt, isImageOcclusionAttempt,
  type ImageOcclusionAttemptView, type QuestionAttemptTarget, type RevealPracticeQuestionInput,
} from "../../ipc/questionAttempts";

interface RevealOptions {
  target: QuestionAttemptTarget;
  attempt: ImageOcclusionAttemptView;
  disabled: boolean;
  imageReady: boolean;
  loadedAt: number | null;
  onReveal: (input: RevealPracticeQuestionInput) => Promise<ImageOcclusionAttemptView>;
  onAttempt: (attempt: ImageOcclusionAttemptView) => void;
  onRevealed: (attempt: ImageOcclusionAttemptView) => void | Promise<void>;
}

/** A reveal retry preserves its identity, expected revision and first elapsed
 * time. Reconciliation is a read, not an excuse to repeat a committed mutation.
 */
export function useOcclusionReveal(options: RevealOptions) {
  const latest = useRef(options);
  latest.current = options;
  const shownAt = useRef<number | null>(null);
  const pending = useRef<RevealPracticeQuestionInput | null>(null);
  const working = useRef(false);
  const mounted = useRef(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  useEffect(() => {
    if (options.imageReady && options.loadedAt !== null && shownAt.current === null) {
      shownAt.current = options.loadedAt;
    }
  }, [options.imageReady, options.loadedAt]);

  async function reconcile(view: ImageOcclusionAttemptView, owner: RevealOptions): Promise<void> {
    try {
      owner.onAttempt(view);
      if (view.progress.phase === "revealed") await owner.onRevealed(view);
    } catch (cause: unknown) {
      if (mounted.current) setError(`The saved answer state could not refresh its feedback or review controls: ${getErrorMessage(cause)}`);
    }
  }

  async function reveal(): Promise<void> {
    const owner = latest.current;
    if (working.current || owner.disabled || !owner.imageReady || owner.attempt.progress.phase !== "answering") return;
    working.current = true;
    setBusy(true);
    setError(null);
    const input = pending.current ?? {
      target: { ...owner.target },
      request: {
        revealId: createMutationId(), expectedRevision: owner.attempt.revision,
        responseTimeMs: Math.max(0, Math.min(86_400_000, Date.now() - (shownAt.current ?? Date.now()))),
      },
    };
    pending.current = input;
    try {
      const view = await owner.onReveal(input);
      if (view.instanceId !== owner.target.instanceId || view.progress.phase !== "revealed"
        || view.revision !== input.request.expectedRevision + 1
        || view.progress.revealId !== input.request.revealId
        || view.progress.responseTimeMs !== input.request.responseTimeMs) {
        throw new Error("The reveal response did not match this captured question. Reload its saved state.");
      }
      pending.current = null;
      await reconcile(view, owner);
    } catch (cause: unknown) {
      if (mounted.current) setError(`${getErrorMessage(cause)} Retry Reveal answer to reuse the same request, or reload the saved answer state.`);
    } finally {
      working.current = false;
      if (mounted.current) setBusy(false);
    }
  }

  async function reload(): Promise<void> {
    const owner = latest.current;
    if (working.current || owner.disabled) return;
    working.current = true;
    setBusy(true);
    setError(null);
    try {
      const saved = await getPracticeQuestionAttempt(owner.target);
      if (!isImageOcclusionAttempt(saved.attempt) || saved.attempt.instanceId !== owner.target.instanceId) {
        throw new Error("The saved state belongs to a different question.");
      }
      // An unchanged answering revision does not prove an ambiguous mutation
      // failed. Keep its retry identity until newer native evidence resolves it.
      if (saved.attempt.progress.phase === "revealed"
        || (pending.current && saved.attempt.revision > pending.current.request.expectedRevision)) {
        pending.current = null;
      }
      if (saved.attempt.revision >= owner.attempt.revision) await reconcile(saved.attempt, owner);
    } catch (cause: unknown) {
      if (mounted.current) setError(getErrorMessage(cause));
    } finally {
      working.current = false;
      if (mounted.current) setBusy(false);
    }
  }

  return { reveal, reload, busy, error };
}
