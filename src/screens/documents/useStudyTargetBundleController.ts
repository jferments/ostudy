import { useCallback, useEffect, useRef, useState } from "react";

import { createOperationId, getErrorMessage } from "../../ipc/common";
import {
  beginStudyTargetBundle,
  composeStudyTargetBundle,
  getStudyTargetBundle,
  type StudyTargetArtifactKind,
  type StudyTargetBundleStatus,
  type StudyTargetRef,
} from "../../ipc/studyMaterials";

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
  const [error, setError] = useState<string | null>(null);
  const readSequence = useRef(0);
  const receipt = useRef<{ key: string; operationId: string } | null>(null);

  const load = useCallback(async () => {
    const sequence = ++readSequence.current;
    if (!target) {
      setStatus(null);
      setError(null);
      setLoading(false);
      return null;
    }
    setLoading(true);
    try {
      const next = await getStudyTargetBundle({ target });
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
      await beginStudyTargetBundle({
        operationId,
        target,
        outputPolicy: { flashcards: true, studyGuide: true },
        recreateArtifacts: suppressedArtifacts(status),
      });
      const result = await composeStudyTargetBundle({ operationId });
      receipt.current = null;
      setStatus(result);
      const failure = durableFailure(result);
      if (failure) setError(failure);
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
        setError(durableFailure(durable));
        await reconcileCommitted("Study materials were saved");
        return true;
      }
      setError(getErrorMessage(failure));
      return false;
    } finally {
      setBusy(false);
    }
  }

  return { status, loading, busy, error, load, run };
}
