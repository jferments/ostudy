import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useRequestLane } from "../../controllers/requestLane";
import { createOperationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import {
  cancelLearningTask, getLearningTaskProgress, getSemanticPreparation, prepareSemanticMaterial,
  previewSemanticPreparation, type LearningTaskProgress, type PreparationPreview,
  type PreparationStatus, type StudyScope,
} from "../../ipc/knowledge";

type PreparationInput = Parameters<typeof prepareSemanticMaterial>[0];

/** Mounted with the Tutor owner: closing a panel does not lose the job/receipt. */
export function useMaterialPreparation(scope: StudyScope) {
  const selection = useMemo(() => ({ scope, includeConversations: false }), [scope]);
  const [preview, setPreview] = useState<PreparationPreview | null>(null);
  const [status, setStatus] = useState<PreparationStatus | null>(null);
  const [progress, setProgress] = useState<LearningTaskProgress | null>(null);
  const [pending, setPending] = useState<PreparationInput | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const receipt = useRef<PreparationInput | null>(null);
  const writing = useRef(false);
  const alive = useRef(true);
  const statusLane = useRequestLane();
  const previewLane = useRequestLane();

  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);

  const reload = useCallback(async () => {
    if (selection.scope.kind === "general") return;
    const token = statusLane.begin();
    try {
      const value = await getSemanticPreparation(selection);
      if (!statusLane.isCurrent(token)) return;
      setStatus(value);
      setError(null);
      if (value?.operationId === receipt.current?.operationId && value?.state !== "running") {
        receipt.current = null;
        setPending(null);
      }
    } catch (failure: unknown) {
      if (statusLane.isCurrent(token)) setError(getErrorMessage(failure));
    }
  }, [selection, statusLane]);

  useEffect(() => {
    previewLane.invalidate();
    setPreview(null);
    if (!writing.current) setBusy(false);
    setStatus(null);
    setProgress(null);
    setError(null);
    void reload();
  }, [reload, previewLane]);

  // Poll only admitted/running work. An ambiguous write is retried explicitly,
  // never automatically resubmitted by an effect or by reopening the panel.
  const activeOperation = status?.state === "running" ? status.operationId : busy ? pending?.operationId : null;
  useEffect(() => {
    if (!activeOperation) return;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll(): Promise<void> {
      try {
        const [value, activity] = await Promise.all([
          getSemanticPreparation(selection), getLearningTaskProgress(activeOperation as string),
        ]);
        if (disposed) return;
        setProgress(activity);
        if (value?.operationId === activeOperation) {
          setStatus(value);
          if (value.state !== "running") {
            if (receipt.current?.operationId === activeOperation) {
              receipt.current = null;
              setPending(null);
            }
            return;
          }
        }
      } catch (failure: unknown) {
        if (!disposed) setError(getErrorMessage(failure));
      }
      if (!disposed) timer = setTimeout(() => { void poll(); }, 1000);
    }
    void poll();
    return () => { disposed = true; clearTimeout(timer); };
  }, [activeOperation, selection]);

  async function review(): Promise<void> {
    if (writing.current || receipt.current || status?.state === "running" || scope.kind === "general") return;
    const token = previewLane.begin();
    setBusy(true);
    setError(null);
    try {
      const value = await previewSemanticPreparation(selection);
      if (previewLane.isCurrent(token)) setPreview(value);
    } catch (failure: unknown) {
      if (previewLane.isCurrent(token)) setError(getErrorMessage(failure));
    } finally {
      if (previewLane.isCurrent(token)) setBusy(false);
    }
  }

  async function confirm(): Promise<void> {
    if (writing.current || (!preview && !receipt.current)) return;
    const input = receipt.current ?? {
      operationId: createOperationId(), selection: preview!.selection, consentKey: preview!.consentKey,
    };
    writing.current = true;
    receipt.current = input;
    statusLane.invalidate();
    setPending(input);
    setBusy(true);
    setError(null);
    try {
      const value = await prepareSemanticMaterial(input);
      if (!alive.current) return;
      setStatus(value);
      setPreview(null);
      if (value.state !== "running") { receipt.current = null; setPending(null); }
    } catch (failure: unknown) {
      if (!alive.current) return;
      const code = getErrorCode(failure);
      if (code && (code.startsWith("invalid_") || ["semantic_preparation_changed",
        "ai_task_profile_required", "ai_profile_disabled", "ai_document_model_not_verified",
        "learning_task_running", "learning_task_capacity"].includes(code))) {
        receipt.current = null;
        setPending(null);
        setPreview(null);
      }
      setError(getErrorMessage(failure));
    } finally {
      writing.current = false;
      if (alive.current) setBusy(false);
    }
  }

  async function cancel(): Promise<void> {
    const operationId = receipt.current?.operationId ?? (status?.state === "running" ? status.operationId : null);
    if (!operationId) return;
    try { await cancelLearningTask(operationId); }
    catch (failure: unknown) { if (alive.current) setError(getErrorMessage(failure)); }
  }

  return { selection, preview, status, progress, pending, busy, error,
    locked: busy || pending !== null || status?.state === "running",
    review, confirm, cancel, reload, dismissPreview: () => setPreview(null) };
}

export type MaterialPreparationController = ReturnType<typeof useMaterialPreparation>;
