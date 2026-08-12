import { convertFileSrc } from "@tauri-apps/api/core";

export function managedAssetUrl(assetId: string): string {
  return convertFileSrc(assetId, "ostudy-asset");
}
