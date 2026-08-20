import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { createOperationId, getErrorMessage } from "../ipc/common";
import {
  commitEditorDraft,
  discardEditorDraft,
  openEditorDraft,
  saveEditorDraft,
  type EditorDraft,
  type EditorDraftCommitRef,
  type EditorDraftKind,
} from "../ipc/drafts";
import { listenToLifecycle } from "../ipc/platform";

export type DurableDraftStatus =
  | "opening"
  | "clean"
  | "locally_dirty"
  | "saving"
  | "saved"
  | "save_failed"
  | "conflicted"
  | "committing";

interface DurableDraftOptions<T extends Record<string, unknown>> {
  kind: EditorDraftKind;
  targetEntityId?: string;
  contextEntityId?: string;
  baseRevision?: number;
  payload: T;
  assetIds?: string[];
  enabled?: boolean;
  onRecovered?: (payload: T) => void;
}

interface DurableDraftController<T extends Record<string, unknown>> {
  draftId: string | null;
  assetDraftId: string | null;
  revision: number | null;
  status: DurableDraftStatus;
  errorMessage: string | null;
  recovered: boolean;
  hasUnsavedLocalChanges: boolean;
  flush: () => Promise<void>;
  discard: () => Promise<void>;
  prepareCanonicalCommit: () => Promise<EditorDraftCommitRef>;
  acceptCanonicalCommit: () => void;
  commit: (targetEntityId?: string) => Promise<void>;
  restart: (payload: T) => Promise<void>;
}

interface RegisteredDiscardHandler {
  token: symbol;
  discard: () => Promise<void>;
}

const PAYLOAD_SCHEMA_VERSION = 1;
const AUTOSAVE_DELAY_MS = 650;
const activeDiscardHandlers = new Map<EditorDraftKind, RegisteredDiscardHandler>();

/**
 * Discard the durable draft owned by the currently mounted editor of this kind.
 * App-level navigation uses this after the user explicitly confirms that
 * unsaved editor changes should be lost. A missing handler means no editor of
 * that kind is currently mounted.
 */
export async function discardActiveDurableDraft(kind: EditorDraftKind): Promise<void> {
  await activeDiscardHandlers.get(kind)?.discard();
}

export function useDurableDraft<T extends Record<string, unknown>>(
  options: DurableDraftOptions<T>,
): DurableDraftController<T> {
  const enabled = options.enabled ?? true;
  const [draft, setDraft] = useState<EditorDraft<T> | null>(null);
  const [status, setStatus] = useState<DurableDraftStatus>("opening");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [recovered, setRecovered] = useState(false);
  const operationIdRef = useRef(createOperationId());
  const openingGenerationRef = useRef(0);
  const savedPayloadKeyRef = useRef("");
  const pendingPayloadRef = useRef<T | null>(null);
  const pendingAssetIdsRef = useRef<string[]>([]);
  const saveLoopRef = useRef<Promise<void> | null>(null);
  const draftRef = useRef<EditorDraft<T> | null>(null);
  const optionsRef = useRef(options);
  const discardRegistrationTokenRef = useRef(Symbol("ostudy-durable-draft"));
  optionsRef.current = options;
  draftRef.current = draft;

  const payloadKey = useMemo(() => stableJson(options.payload), [options.payload]);
  const assetKey = useMemo(
    () => [...new Set(options.assetIds ?? [])].sort().join(","),
    [options.assetIds],
  );

  const open = useCallback(async (initialPayload: T, operationId: string): Promise<void> => {
    const generation = openingGenerationRef.current + 1;
    openingGenerationRef.current = generation;
    setStatus("opening");
    setErrorMessage(null);
    const loaded = await openEditorDraft<T>({
      operationId,
      draftKind: optionsRef.current.kind,
      targetEntityId: optionsRef.current.targetEntityId,
      contextEntityId: optionsRef.current.contextEntityId,
      baseRevision: optionsRef.current.baseRevision,
      payloadSchemaVersion: PAYLOAD_SCHEMA_VERSION,
      initialPayload,
    });
    if (openingGenerationRef.current !== generation) return;
    const initialKey = stableJson(initialPayload);
    const loadedKey = stableJson(loaded.payload);
    draftRef.current = loaded;
    setDraft(loaded);
    savedPayloadKeyRef.current = loadedKey;
    const wasRecovered = loadedKey !== initialKey || loaded.updatedAt > loaded.createdAt;
    setRecovered(wasRecovered);
    if (wasRecovered) optionsRef.current.onRecovered?.(loaded.payload);
    setStatus(loaded.state === "conflicted" ? "conflicted" : "clean");
  }, []);

  useEffect(() => {
    if (!enabled) {
      setDraft(null);
      setStatus("clean");
      return;
    }
    let active = true;
    void open(options.payload, operationIdRef.current).catch((error: unknown) => {
      if (!active) return;
      setErrorMessage(getErrorMessage(error));
      setStatus("save_failed");
    });
    return () => {
      active = false;
      openingGenerationRef.current += 1;
    };
    // The editor identity, not the changing payload, controls draft opening.
  }, [
    enabled,
    open,
    options.kind,
    options.targetEntityId,
    options.contextEntityId,
    options.baseRevision,
  ]);

  const runSaveLoop = useCallback(async (): Promise<void> => {
    if (saveLoopRef.current) return saveLoopRef.current;
    const loop = (async () => {
      while (pendingPayloadRef.current) {
        const currentDraft = draftRef.current;
        if (!currentDraft || currentDraft.state !== "active") return;
        const payload = pendingPayloadRef.current;
        const assets = pendingAssetIdsRef.current;
        pendingPayloadRef.current = null;
        setStatus("saving");
        setErrorMessage(null);
        try {
          const saved = await saveEditorDraft<T>({
            draftId: currentDraft.id,
            revision: currentDraft.revision,
            payloadSchemaVersion: PAYLOAD_SCHEMA_VERSION,
            payload,
            assetIds: assets,
          });
          draftRef.current = saved;
          setDraft(saved);
          savedPayloadKeyRef.current = stableJson(saved.payload);
          setStatus(pendingPayloadRef.current ? "locally_dirty" : "saved");
        } catch (error: unknown) {
          pendingPayloadRef.current = payload;
          setErrorMessage(getErrorMessage(error));
          setStatus("save_failed");
          throw error;
        }
      }
    })().finally(() => {
      saveLoopRef.current = null;
    });
    saveLoopRef.current = loop;
    return loop;
  }, []);

  const flush = useCallback(async (): Promise<void> => {
    if (!enabled) return;
    if (!draftRef.current) {
      throw new Error("The durable editor draft is still opening.");
    }
    const currentPayload = optionsRef.current.payload;
    const currentKey = stableJson(currentPayload);
    if (
      currentKey !== savedPayloadKeyRef.current ||
      assetKey !== pendingAssetIdsRef.current.join(",")
    ) {
      pendingPayloadRef.current = currentPayload;
      pendingAssetIdsRef.current = [...new Set(optionsRef.current.assetIds ?? [])].sort();
    }
    await runSaveLoop();
  }, [assetKey, enabled, runSaveLoop]);

  useEffect(() => {
    if (!enabled || !draft || draft.state !== "active") return;
    if (
      payloadKey === savedPayloadKeyRef.current &&
      assetKey === pendingAssetIdsRef.current.join(",")
    ) {
      return;
    }
    pendingPayloadRef.current = options.payload;
    pendingAssetIdsRef.current = [...new Set(options.assetIds ?? [])].sort();
    setStatus((current) => (current === "saving" ? current : "locally_dirty"));
    const timer = window.setTimeout(() => {
      void runSaveLoop().catch(() => undefined);
    }, AUTOSAVE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [assetKey, draft, enabled, options.assetIds, options.payload, payloadKey, runSaveLoop]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenToLifecycle((snapshot) => {
      if (
        snapshot.phase === "background" ||
        snapshot.phase === "suspending" ||
        snapshot.phase === "termination_requested"
      ) {
        void flush().catch(() => undefined);
      }
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [flush]);

  const discard = useCallback(async (): Promise<void> => {
    const current = draftRef.current;
    if (!current) return;
    await discardEditorDraft(current.id, current.revision);
    draftRef.current = null;
    setDraft(null);
    pendingPayloadRef.current = null;
    pendingAssetIdsRef.current = [];
    savedPayloadKeyRef.current = "";
    setRecovered(false);
    setErrorMessage(null);
    setStatus("clean");
  }, []);

  useEffect(() => {
    if (!enabled) return;
    const token = discardRegistrationTokenRef.current;
    const kind = options.kind;
    activeDiscardHandlers.set(kind, { token, discard });
    return () => {
      if (activeDiscardHandlers.get(kind)?.token === token) {
        activeDiscardHandlers.delete(kind);
      }
    };
  }, [discard, enabled, options.kind]);

  const prepareCanonicalCommit = useCallback(async (): Promise<EditorDraftCommitRef> => {
    await flush();
    const current = draftRef.current;
    if (!current || current.state !== "active") {
      throw new Error("The durable editor draft is not available for its final save.");
    }
    return { draftId: current.id, revision: current.revision };
  }, [flush]);

  const acceptCanonicalCommit = useCallback((): void => {
    draftRef.current = null;
    setDraft(null);
    pendingPayloadRef.current = null;
    pendingAssetIdsRef.current = [];
    savedPayloadKeyRef.current = "";
    setRecovered(false);
    setErrorMessage(null);
    setStatus("clean");
  }, []);

  const commit = useCallback(
    async (targetEntityId?: string): Promise<void> => {
      const reference = await prepareCanonicalCommit();
      setStatus("committing");
      await commitEditorDraft(reference.draftId, reference.revision, targetEntityId);
      acceptCanonicalCommit();
    },
    [acceptCanonicalCommit, prepareCanonicalCommit],
  );

  const restart = useCallback(
    async (payload: T): Promise<void> => {
      const current = draftRef.current;
      if (current) {
        await commitEditorDraft(current.id, current.revision);
      }
      operationIdRef.current = createOperationId();
      draftRef.current = null;
      setDraft(null);
      pendingPayloadRef.current = null;
      pendingAssetIdsRef.current = [];
      savedPayloadKeyRef.current = "";
      await open(payload, operationIdRef.current);
    },
    [open],
  );

  return {
    draftId: draft?.id ?? null,
    assetDraftId: draft?.assetDraftId ?? null,
    revision: draft?.revision ?? null,
    status,
    errorMessage,
    recovered,
    hasUnsavedLocalChanges:
      status === "locally_dirty" || status === "saving" || status === "save_failed",
    flush,
    discard,
    prepareCanonicalCommit,
    acceptCanonicalCommit,
    commit,
    restart,
  };
}

function stableJson(value: unknown): string {
  return JSON.stringify(value, (_key, nested) => {
    if (nested === null || typeof nested !== "object" || Array.isArray(nested)) {
      return nested;
    }
    return Object.fromEntries(
      Object.entries(nested as Record<string, unknown>).sort(([left], [right]) =>
        left < right ? -1 : left > right ? 1 : 0,
      ),
    );
  });
}
