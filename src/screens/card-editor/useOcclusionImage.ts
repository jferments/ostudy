import { useEffect, useRef, useState } from "react";

import type { Asset } from "../../ipc/assets";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import { selectAndStageExternalImport } from "../../ipc/externalFiles";
import { prepareOcclusionImage } from "../../ipc/occlusion";
import { managedAssetUrl } from "../../platform/assets";
import type { Size } from "../../editor/occlusion/geometry";
import { newOcclusionDraft, type OcclusionDraftUpdate, type OcclusionEditorDraft } from "./occlusionDraft";

interface ImportOptions {
  draft: OcclusionEditorDraft;
  assetDraftId: string | null;
  disabled: boolean;
  flushDraft: () => Promise<void>;
  onUpdate: (update: OcclusionDraftUpdate) => void;
  onAssetImported: (asset: Asset) => void;
}

/** Persist operation identity before opening the system picker or normalizing pixels.
 * A lost acknowledgement keeps the same staged handle. Recovery is an explicit retry,
 * not an automatic second picker or a new native mutation identity.
 */
export function useOcclusionImport(options: ImportOptions) {
  const latest = useRef(options);
  latest.current = options;
  const [requestedId, setRequestedId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pending = options.draft.pendingImport;

  useEffect(() => {
    if (!pending || pending.operationId !== requestedId || options.disabled) return;
    let live = true;
    setBusy(true);
    setError(null);
    const run = async () => {
      if (pending.assetDraftId !== latest.current.assetDraftId) {
        throw new Error("This image selection belongs to an earlier editor draft. Cancel it and choose the image again.");
      }
      // The effect runs after the parent has installed pending into its durable payload.
      await latest.current.flushDraft();
      if (!live) return;
      if (!pending.stagedFileId) {
        const selected = await selectAndStageExternalImport("image", pending.operationId);
        if (!live) return;
        latest.current.onUpdate((draft) => draft.pendingImport?.operationId === pending.operationId
          ? { ...draft, pendingImport: selected ? { ...pending, stagedFileId: selected.id } : null }
          : draft);
        if (!selected) setRequestedId(null);
        return;
      }
      const prepared = await prepareOcclusionImage(pending.stagedFileId, pending.assetDraftId);
      if (!live) return;
      if (latest.current.draft.pendingImport?.operationId !== pending.operationId) return;
      latest.current.onAssetImported(prepared.asset);
      latest.current.onUpdate((draft) => draft.pendingImport?.operationId === pending.operationId
        ? {
            ...newOcclusionDraft(), imageAssetId: prepared.asset.id,
            imageSize: { width: prepared.width, height: prepared.height },
          }
        : draft);
      setRequestedId(null);
    };
    void run().catch((cause: unknown) => {
      if (!live) return;
      setError(getErrorMessage(cause));
      setRequestedId(null);
    }).finally(() => {
      if (live) setBusy(false);
    });
    return () => { live = false; };
  }, [requestedId, pending?.operationId, pending?.stagedFileId, options.assetDraftId, options.disabled]);

  function start(): void {
    const current = latest.current;
    if (busy || current.disabled || !current.assetDraftId) return;
    const operationId = createOperationId();
    current.onUpdate((draft) => ({
      ...draft,
      pendingImport: { operationId, assetDraftId: current.assetDraftId!, stagedFileId: null },
    }));
    setRequestedId(operationId);
  }
  function retry(): void {
    if (busy || latest.current.disabled || !latest.current.draft.pendingImport) return;
    setError(null);
    setRequestedId(latest.current.draft.pendingImport.operationId);
  }
  function cancel(): void {
    if (busy || latest.current.disabled) return;
    latest.current.onUpdate((draft) => ({ ...draft, pendingImport: null }));
    setRequestedId(null);
    setError(null);
    // Unreferenced staging belongs to the existing bounded native staging ledger.
    // Canceling a UI retry must not pretend to roll back an acknowledged native import.
  }
  return { start, retry, cancel, busy, error };
}

/** Authoring only: the original managed image is never used by the Practice image route. */
export function useOcclusionSource(assetId: string | null) {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<{ key: string; size: Size | null; error: string | null } | null>(null);
  const key = `${assetId ?? "none"}:${attempt}`;
  const url = assetId ? managedAssetUrl(assetId) : null;
  useEffect(() => {
    if (!url) return;
    let live = true;
    const image = new Image();
    image.onload = () => {
      if (!live) return;
      const width = image.naturalWidth;
      const height = image.naturalHeight;
      const valid = width > 0 && height > 0 && width <= 8192 && height <= 8192 && width * height <= 16_000_000;
      setState({ key, size: valid ? { width, height } : null,
        error: valid ? null : "The managed image has unsupported dimensions. Choose a smaller image." });
    };
    image.onerror = () => {
      if (live) setState({ key, size: null, error: "The managed source image could not be opened. Your questions and regions are unchanged." });
    };
    image.src = url;
    return () => { live = false; image.onload = null; image.onerror = null; image.src = ""; };
  }, [key, url]);
  return {
    url,
    size: state?.key === key ? state.size : null,
    error: state?.key === key ? state.error : null,
    retry: () => setAttempt((value) => value + 1),
  };
}
