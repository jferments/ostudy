import { useCallback, useEffect, useRef, useState } from "react";

import { createOperationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import {
  beginStudyTargetBundle,
  cancelStudyTargetBundle,
  composeStudyTargetBundle,
  getStudyTargetBundle,
  type StudyTargetArtifactKind,
  type StudyTargetBundleStatus,
  type StudyTargetRef,
} from "../../ipc/studyMaterials";

const STATUS_POLL_INTERVAL_MS = 400;

const TERMINAL_BATCH_STATES = new Set([
  "complete",
  "partial",
  "failed",
  "canceled",
  "interrupted",
]);

function targetKey(target: StudyTargetRef): string {
  return `${target.kind}:${target.id}`;
}

function suppressedArtifacts(status: StudyTargetBundleStatus | null): StudyTargetArtifactKind[] {
  if (!status) return [];
  const result: StudyTargetArtifactKind[] = [];
  if (status.flashcards.state === "suppressed") result.push("flashcards");
  if (status.studyGuide.state === "suppressed") result.push("study_guide");
  return result;
}

function durableFailure(status: StudyTargetBundleStatus): string | null {
  for (const artifact of [status.flashcards, status.studyGuide]) {
    if (
      (artifact.state === "failed" || artifact.state === "interrupted" || artifact.state === "canceled")
      && artifact.safeErrorMessage
    ) {
      return artifact.safeErrorMessage;
    }
  }
  return null;
}

export function useStudyTargetBundleController({
  target,
  changed,
  refreshKey,
}: {
  target: StudyTargetRef | null;
  changed?: () => Promise<unknown>;
  refreshKey?: string | number | null;
}) {
  const [status, setStatus] = useState<StudyTargetBundleStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [canceling, setCanceling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const cancelingRef = useRef(false);
  const readSequence = useRef(0);
  const receipt = useRef<{ key: string; operationId: string } | null>(null);
  const targetRef = useRef(target);
  targetRef.current = target;

  const load = useCallback(async () => {
    const sequence = ++readSequence.current;
    const currentTarget = targetRef.current;
    if (!currentTarget) {
      setStatus(null);
      setError(null);
      setLoading(false);
      return null;
    }
    setLoading(true);
    try {
      const next = await getStudyTargetBundle({ target: currentTarget });
      if (readSequence.current !== sequence) return next;
      setStatus(next);
      setError(null);
      return next;
    } catch (failure: unknown) {
      if (readSequence.current === sequence) setError(getErrorMessage(failure));
      return null;
    } finally {
      if (readSequence.current === sequence) setLoading(false);
    }
  }, [refreshKey, target?.id, target?.kind]);

  useEffect(() => {
    void load();
    return () => { readSequence.current += 1; };
  }, [load]);

  useEffect(() => {
    const currentTarget = targetRef.current;
    if (!busy || !currentTarget) return;
    let disposed = false;
    let polling = false;
    const poll = async () => {
      if (disposed || polling) return;
      polling = true;
      try {
        const next = await getStudyTargetBundle({ target: currentTarget });
        if (!disposed && next) setStatus(next);
      } catch {
        // The composing command owns the terminal error. A transient status read
        // must not replace it or stop the durable operation.
      } finally {
        polling = false;
      }
    };
    void poll();
    const timer = window.setInterval(() => { void poll(); }, STATUS_POLL_INTERVAL_MS);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [busy, target?.id, target?.kind]);

  async function reconcileCommitted(message: string): Promise<void> {
    if (!changed) return;
    try {
      await changed();
    } catch (failure: unknown) {
      setError(`${message}, but refreshing the library failed: ${getErrorMessage(failure)}`);
    }
  }

  async function run(): Promise<boolean> {
    if (!target || busy) return false;
    const key = targetKey(target);
    const operationId = receipt.current?.key === key
      ? receipt.current.operationId
      : createOperationId();
    receipt.current = { key, operationId };
    setBusy(true);
    setError(null);
    readSequence.current += 1;
    try {
      const begun = await beginStudyTargetBundle({
        operationId,
        target,
        outputPolicy: { flashcards: true, studyGuide: true },
        recreateArtifacts: suppressedArtifacts(status),
      });
      setStatus(begun);
      const result = await composeStudyTargetBundle({ operationId });
      receipt.current = null;
      setStatus(result);
      const failure = durableFailure(result);
      setError(failure && !cancelingRef.current ? failure : null);
      await reconcileCommitted("Study materials were saved");
      return true;
    } catch (failure: unknown) {
      const durable = await getStudyTargetBundle({ target }).catch(() => null);
      if (durable) setStatus(durable);
      if (
        durable?.latestOperationId === operationId
        && durable.latestBatchState
        && TERMINAL_BATCH_STATES.has(durable.latestBatchState)
      ) {
        receipt.current = null;
        setError(durable.latestBatchState === "canceled" ? null : durableFailure(durable));
        await reconcileCommitted("Study materials were saved");
        return true;
      }
      const code = getErrorCode(failure);
      if (cancelingRef.current && (
        code === "ai_request_canceled"
        || code === "study_target_generation_canceled"
        || code === "study_target_generation_not_running"
      )) {
        setError(null);
        return true;
      }
      setError(getErrorMessage(failure));
      return false;
    } finally {
      setBusy(false);
      cancelingRef.current = false;
      setCanceling(false);
    }
  }

  async function stop(): Promise<void> {
    if (!target || !busy || canceling) return;
    const operationId = receipt.current?.operationId ?? status?.latestOperationId;
    if (!operationId) return;
    cancelingRef.current = true;
    setCanceling(true);
    setError(null);
    try {
      await cancelStudyTargetBundle(operationId);
    } catch (failure: unknown) {
      if (getErrorCode(failure) !== "study_target_generation_not_running") {
        cancelingRef.current = false;
        setCanceling(false);
        setError(getErrorMessage(failure));
      }
    }
  }

  return { status, loading, busy, canceling, error, load, run, stop };
}
