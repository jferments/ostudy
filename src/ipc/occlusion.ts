import { invoke } from "@tauri-apps/api/core";

import type { Asset } from "./assets";

export interface PreparedOcclusionImage {
  asset: Asset;
  width: number;
  height: number;
  pixelFingerprint: string;
}

export interface PreviewOcclusionInput {
  definitionJson: string;
  presentationKey: string;
  assetDraftId: string | null;
  revealed: boolean;
  maximumEdge: number;
}

/** Caller retains the same staged handle across a lost import acknowledgement. */
export function prepareOcclusionImage(stagedFileId: string, draftId: string): Promise<PreparedOcclusionImage> {
  return invoke<PreparedOcclusionImage>("prepare_occlusion_image", { input: { stagedFileId, draftId } });
}

/** Authoring-only preview. Practice resolves reveal state natively, not from this Boolean. */
export function previewOcclusionImage(input: PreviewOcclusionInput): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("preview_occlusion_image", { input });
}
