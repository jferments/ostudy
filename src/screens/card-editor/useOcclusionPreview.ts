import { useEffect, useState } from "react";

import { getErrorMessage } from "../../ipc/common";
import { previewOcclusionImage } from "../../ipc/occlusion";
import { occlusionDefinition, occlusionDraftReady, type OcclusionEditorDraft } from "./occlusionDraft";

export function useOcclusionPreview(draft: OcclusionEditorDraft, assetDraftId: string | null, enabled: boolean, revealed: boolean) {
  const [retry, setRetry] = useState(0);
  const [state, setState] = useState<{ key: string; url: string | null; error: string | null } | null>(null);
  const ready = enabled && occlusionDraftReady(draft) && !!draft.selectedQuestionId;
  const definitionJson = ready ? JSON.stringify(occlusionDefinition(draft)) : "";
  const key = JSON.stringify([definitionJson, draft.selectedQuestionId, assetDraftId, revealed, retry]);
  useEffect(() => {
    if (!definitionJson || !draft.selectedQuestionId) return;
    let live = true;
    let objectUrl: string | null = null;
    void previewOcclusionImage({
      definitionJson, presentationKey: draft.selectedQuestionId, assetDraftId, revealed, maximumEdge: 1600,
    }).then((bytes) => {
      if (!live) return;
      objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      setState({ key, url: objectUrl, error: null });
    }).catch((cause: unknown) => {
      if (live) setState({ key, url: null, error: getErrorMessage(cause) });
    });
    return () => { live = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  }, [key, definitionJson, draft.selectedQuestionId, assetDraftId, revealed]);
  // Never paint a previous target's image while a new render is pending.
  return {
    ready, url: ready && state?.key === key ? state.url : null,
    error: ready && state?.key === key ? state.error : null,
    retry: () => setRetry((value) => value + 1),
  };
}
