import { invoke } from "@tauri-apps/api/core";
import { createOperationId } from "./common";

export interface Asset {
  id: string;
  originalFilename: string;
  mimeType: string;
  byteSize: number;
  createdAt: number;
}

export interface AssetDraft {
  id: string;
  purpose: "card" | "template" | "import";
  expiresAt: number;
}

export async function beginAssetDraft(
  purpose: AssetDraft["purpose"],
  ownerId?: string,
): Promise<AssetDraft> {
  return invoke<AssetDraft>("begin_asset_draft", { input: { purpose, ownerId } });
}

export async function cancelAssetDraft(draftId: string): Promise<void> {
  return invoke<void>("cancel_asset_draft", { draftId });
}

export async function syncAssetDraft(draftId: string, assetIds: string[]): Promise<void> {
  return invoke<void>("sync_asset_draft", { input: { draftId, assetIds } });
}

export async function importImage(stagedFileId: string, draftId: string): Promise<Asset> {
  return invoke<Asset>("import_image", { input: { stagedFileId, draftId } });
}

export async function importMedia(
  draftId: string,
  operationId = createOperationId(),
): Promise<Asset | null> {
  return invoke<Asset | null>("import_media", { input: { draftId, operationId } });
}

export async function readMediaAsset(assetId: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("read_media_asset", { input: { assetId } });
}
