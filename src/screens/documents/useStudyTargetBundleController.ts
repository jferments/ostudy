import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";

import { createOperationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import {
  beginStudyTargetBundle,
  cancelStudyTargetBundle,
  composeStudyTargetBundle,
  getStudyTargetBundle,
  type StudyTargetArtifactKind,
  type StudyTargetBundleStatus,
  type StudyTargetOutputPolicy,
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

/**
 * A bundle being made. Like a course being written, a run lives outside any one
 * page: it keeps going and keeps its progress when the page that started it
 * closes, and a New study set can start one from no page at all.
 */
interface BundleRun {
  busy: boolean;
  canceling: boolean;
  error: string | null;
  /** The latest status the run has seen, and how many it has seen. */
  status: StudyTargetBundleStatus | null;
  statusSequence: number;
}

const IDLE: BundleRun = { busy: false, canceling: false, error: null, status: null, statusSequence: 0 };
const runs = new Map<string, BundleRun>();
/** The operation a run that ended without a durable answer retries under. */
const receipts = new Map<string, string>();
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function current(key: string): BundleRun {
  return runs.get(key) ?? IDLE;
}

function publish(key: string, change: Partial<BundleRun>): void {
  const before = current(key);
  const next = { ...before, ...change };
  if (change.status) next.statusSequence = before.statusSequence + 1;
  runs.set(key, next);
  for (const listener of listeners) listener();
}

/** Forgets every run; for tests that reuse one target. */
export function resetStudyTargetBundleRuns(): void {
  runs.clear();
  receipts.clear();
  for (const listener of listeners) listener();
}

/** The bundle being made for the target now, with the latest status it has seen; null when none is. */
export function useStudyTargetBundleRun(target: StudyTargetRef): StudyTargetBundleStatus | "starting" | null {
  const key = targetKey(target);
  const run = useSyncExternalStore(subscribe, () => current(key));
  if (!run.busy) return null;
  return run.status ?? "starting";
}

async function reconcileCommitted(key: string, changed: (() => Promise<unknown>) | undefined, message: string): Promise<void> {
  if (!changed) return;
  try {
    await changed();
  } catch (failure: unknown) {
    publish(key, { error: `${message}, but refreshing the library failed: ${getErrorMessage(failure)}` });
  }
}

/**
 * Makes both outputs unless `outputPolicy` asks for one; a retry keeps the receipt's
 * identity. `known` is the target's latest status, which says which outputs to recreate.
 */
export async function runStudyTargetBundle(
  target: StudyTargetRef,
  outputPolicy: StudyTargetOutputPolicy = { flashcards: true, studyGuide: true },
  known: StudyTargetBundleStatus | null = null,
  changed?: () => Promise<unknown>,
): Promise<boolean> {
  const key = targetKey(target);
  if (current(key).busy) return false;
  const operationId = receipts.get(key) ?? createOperationId();
  receipts.set(key, operationId);
  publish(key, { busy: true, canceling: false, error: null, status: null });
  let polling = false;
  const timer = window.setInterval(() => {
    if (polling) return;
    polling = true;
    getStudyTargetBundle({ target }).then(
      (next) => { if (next && current(key).busy) publish(key, { status: next }); },
      // The composing command owns the terminal error. A transient status read
      // must not replace it or stop the durable operation.
      () => undefined,
    ).finally(() => { polling = false; });
  }, STATUS_POLL_INTERVAL_MS);
  try {
    const begun = await beginStudyTargetBundle({
      operationId,
      target,
      outputPolicy,
      recreateArtifacts: suppressedArtifacts(known).filter((kind) =>
        kind === "flashcards" ? outputPolicy.flashcards : outputPolicy.studyGuide),
    });
    publish(key, { status: begun });
    const result = await composeStudyTargetBundle({ operationId });
    receipts.delete(key);
    const failure = durableFailure(result);
    publish(key, { status: result, error: failure && !current(key).canceling ? failure : null });
    await reconcileCommitted(key, changed, "Study materials were saved");
    return true;
  } catch (failure: unknown) {
    const durable = await getStudyTargetBundle({ target }).catch(() => null);
    if (durable) publish(key, { status: durable });
    if (
      durable?.latestOperationId === operationId
      && durable.latestBatchState
      && TERMINAL_BATCH_STATES.has(durable.latestBatchState)
    ) {
      receipts.delete(key);
      publish(key, { error: durable.latestBatchState === "canceled" ? null : durableFailure(durable) });
      await reconcileCommitted(key, changed, "Study materials were saved");
      return true;
    }
    const code = getErrorCode(failure);
    if (current(key).canceling && (
      code === "ai_request_canceled"
      || code === "study_target_generation_canceled"
      || code === "study_target_generation_not_running"
    )) {
      publish(key, { error: null });
      return true;
    }
    publish(key, { error: getErrorMessage(failure) });
    return false;
  } finally {
    window.clearInterval(timer);
    publish(key, { busy: false, canceling: false });
  }
}

/** Stops the target's running bundle, if it has one. */
export async function stopStudyTargetBundle(target: StudyTargetRef, latestOperationId: string | null = null): Promise<void> {
  const key = targetKey(target);
  const run = current(key);
  if (!run.busy || run.canceling) return;
  const operationId = receipts.get(key) ?? run.status?.latestOperationId ?? latestOperationId;
  if (!operationId) return;
  publish(key, { canceling: true, error: null });
  try {
    await cancelStudyTargetBundle(operationId);
  } catch (failure: unknown) {
    if (getErrorCode(failure) !== "study_target_generation_not_running") {
      publish(key, { canceling: false, error: getErrorMessage(failure) });
    }
  }
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
  const key = target ? targetKey(target) : "";
  const run = useSyncExternalStore(subscribe, () => (key ? current(key) : IDLE));
  const [status, setStatus] = useState<StudyTargetBundleStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [readError, setReadError] = useState<string | null>(null);
  const readSequence = useRef(0);
  const targetRef = useRef(target);
  targetRef.current = target;
  // Only statuses a run sees after this page opened replace what the page read.
  const seenSequence = useRef(run.statusSequence);

  const load = useCallback(async () => {
    const sequence = ++readSequence.current;
    const currentTarget = targetRef.current;
    if (!currentTarget) {
      setStatus(null);
      setReadError(null);
      setLoading(false);
      return null;
    }
    setLoading(true);
    try {
      const next = await getStudyTargetBundle({ target: currentTarget });
      if (readSequence.current !== sequence) return next;
      setStatus(next);
      setReadError(null);
      const run = current(targetKey(currentTarget));
      if (run.error && !run.busy) publish(targetKey(currentTarget), { error: null });
      return next;
    } catch (failure: unknown) {
      if (readSequence.current === sequence) setReadError(getErrorMessage(failure));
      return null;
    } finally {
      if (readSequence.current === sequence) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    return () => { readSequence.current += 1; };
  }, [load, refreshKey, target?.id, target?.kind]);

  useEffect(() => {
    if (run.statusSequence === seenSequence.current) return;
    seenSequence.current = run.statusSequence;
    if (run.status) {
      // A run's status is newer than any read still in flight.
      readSequence.current += 1;
      setStatus(run.status);
    }
  }, [run.status, run.statusSequence]);

  return {
    status,
    loading,
    busy: run.busy,
    canceling: run.canceling,
    error: run.error ?? readError,
    load,
    run: (outputPolicy?: StudyTargetOutputPolicy) =>
      (target ? runStudyTargetBundle(target, outputPolicy, status, changed) : Promise.resolve(false)),
    stop: () => (target ? stopStudyTargetBundle(target, status?.latestOperationId ?? null) : Promise.resolve()),
  };
}
