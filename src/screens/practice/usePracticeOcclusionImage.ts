import { useEffect, useState } from "react";

import { occlusionRasterReads } from "../../editor/occlusion/rasterReadQueue";
import type { Size } from "../../editor/occlusion/geometry";
import { getErrorMessage } from "../../ipc/common";
import { readPracticeOcclusionImage, type QuestionAttemptTarget } from "../../ipc/questionAttempts";

interface RasterState {
  identity: string;
  url: string;
  size: Size;
  loadedAt: number;
}

/** Preserve an already visible raster while refining its resolution, but never
 * reuse pixels across a captured target or reveal revision. Native state alone
 * selects masking; the hook has no original asset URL or reveal override.
 */
export function usePracticeOcclusionImage(target: QuestionAttemptTarget, revision: number, maximumEdge: number) {
  const [retry, setRetry] = useState(0);
  const [state, setState] = useState<RasterState | null>(null);
  const [failure, setFailure] = useState<{ key: string; message: string } | null>(null);
  const identity = JSON.stringify([target.sessionId, target.cardId, target.instanceId, revision]);
  const key = JSON.stringify([identity, maximumEdge, retry]);

  useEffect(() => {
    setState((current) => current?.identity === identity ? current : null);
  }, [identity]);
  useEffect(() => () => { if (state?.url) URL.revokeObjectURL(state.url); }, [state?.url]);
  useEffect(() => {
    let live = true;
    let objectUrl: string | null = null;
    let adopted = false;
    const image = new Image();
    const fail = (message: string) => { if (live) setFailure({ key, message }); };
    image.onload = () => {
      if (!live || !objectUrl) return;
      const width = image.naturalWidth;
      const height = image.naturalHeight;
      if (width < 1 || height < 1 || width > 4096 || height > 4096) {
        fail("The prepared image has invalid dimensions. Your saved question is unchanged.");
        return;
      }
      const url = objectUrl;
      adopted = true;
      setState((current) => ({
        identity,
        url,
        size: { width, height },
        loadedAt: current?.identity === identity ? current.loadedAt : Date.now(),
      }));
      setFailure(null);
    };
    image.onerror = () => fail("The study image could not be displayed. Retry the image; no answer or rating has been changed.");
    const requestTarget: QuestionAttemptTarget = {
      sessionId: target.sessionId,
      cardId: target.cardId,
      instanceId: target.instanceId,
    };
    void occlusionRasterReads.read(() => live, () => readPracticeOcclusionImage({
      target: requestTarget, expectedRevision: revision, maximumEdge,
    })).then((bytes) => {
      if (!live || !bytes) return;
      objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      image.src = objectUrl;
    }).catch((error: unknown) => fail(getErrorMessage(error)));
    return () => {
      live = false;
      image.onload = null;
      image.onerror = null;
      image.src = "";
      // Visible URLs are released after React replaces their DOM, not when a
      // sharper request starts. Failed/canceled decode URLs have no visible owner.
      if (objectUrl && !adopted) URL.revokeObjectURL(objectUrl);
    };
  }, [key, identity, target.sessionId, target.cardId, target.instanceId, revision, maximumEdge]);

  const current = state?.identity === identity ? state : null;
  return {
    url: current?.url ?? null,
    size: current?.size ?? null,
    error: failure?.key === key ? failure.message : null,
    loadedAt: current?.loadedAt ?? null,
    ready: !!current,
    retry: () => setRetry((value) => value + 1),
  };
}
