import { useEffect, useState } from "react";

import { occlusionRasterReads } from "../../editor/occlusion/rasterReadQueue";
import type { Size } from "../../editor/occlusion/geometry";
import { getErrorMessage } from "../../ipc/common";
import { readPracticeOcclusionImage, type QuestionAttemptTarget } from "../../ipc/questionAttempts";

interface RasterState {
  key: string;
  url: string | null;
  size: Size | null;
  error: string | null;
  loadedAt: number | null;
}

/** This controller has no source asset ID, original URL, authored definition or
 * client-authorized reveal flag. The native captured revision owns those choices.
 */
export function usePracticeOcclusionImage(target: QuestionAttemptTarget, revision: number, maximumEdge: number) {
  const [retry, setRetry] = useState(0);
  const [state, setState] = useState<RasterState | null>(null);
  const key = JSON.stringify([target.sessionId, target.cardId, target.instanceId, revision, maximumEdge, retry]);
  useEffect(() => {
    let live = true;
    let objectUrl: string | null = null;
    const image = new Image();
    const fail = (error: string) => {
      if (live) setState({ key, url: null, size: null, error, loadedAt: null });
    };
    image.onload = () => {
      if (!live || !objectUrl) return;
      const width = image.naturalWidth;
      const height = image.naturalHeight;
      if (width < 1 || height < 1 || width > 4096 || height > 4096) {
        fail("The prepared image has invalid dimensions. Your saved question is unchanged.");
        return;
      }
      setState({ key, url: objectUrl, size: { width, height }, error: null, loadedAt: Date.now() });
    };
    image.onerror = () => fail("The prepared study image could not be displayed. Retry the image; no answer or rating has been changed.");
    void occlusionRasterReads.read(() => live, () => readPracticeOcclusionImage({
      target, expectedRevision: revision, maximumEdge,
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
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [key, target.sessionId, target.cardId, target.instanceId, revision, maximumEdge]);

  const current = state?.key === key ? state : null;
  return {
    url: current?.url ?? null,
    size: current?.size ?? null,
    error: current?.error ?? null,
    loadedAt: current?.loadedAt ?? null,
    ready: !!current?.url && !!current.size,
    retry: () => setRetry((value) => value + 1),
  };
}
