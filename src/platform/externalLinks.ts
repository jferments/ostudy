import { openUrl as openNativeUrl } from "@tauri-apps/plugin-opener";

/** Opens a validated external URL through the platform-owned adapter. */
export async function openExternalUrl(url: string): Promise<void> {
  await openNativeUrl(url);
}

export const openUrl = openExternalUrl;
