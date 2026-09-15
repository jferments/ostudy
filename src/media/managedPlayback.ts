import { readMediaAsset } from "../ipc/assets";

const ASSET_ID_PATTERN = /^[0-9a-f]{32}$/u;

export async function createManagedMediaObjectUrl(
  assetId: string,
  mimeType: string,
): Promise<string> {
  if (!ASSET_ID_PATTERN.test(assetId)) {
    throw new Error("The managed media asset identifier is invalid.");
  }
  if (!(mimeType.startsWith("audio/") || mimeType.startsWith("video/"))) {
    throw new Error("The managed media type is invalid.");
  }

  const bytes = await readMediaAsset(assetId);
  if (!(bytes instanceof ArrayBuffer) || bytes.byteLength === 0) {
    throw new Error("The managed media asset returned no bytes.");
  }

  return URL.createObjectURL(new Blob([bytes], { type: mimeType }));
}

export function revokeManagedMediaObjectUrl(url: string | null): void {
  if (url !== null) URL.revokeObjectURL(url);
}
