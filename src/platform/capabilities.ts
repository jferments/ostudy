import {
  getNativePlatformSnapshot,
  type NativePlatformSnapshot,
} from "../ipc/platform";

export interface PlatformCapabilities {
  coarsePointer: boolean;
  hoverAvailable: boolean;
  hardwareBackAvailable: boolean;
  audioAvailable: boolean;
  hapticsAvailable: boolean;
  nativeFilePickerAvailable: boolean;
  native: NativePlatformSnapshot | null;
}

let nativeSnapshot: NativePlatformSnapshot | null = null;
const listeners = new Set<() => void>();

function matches(query: string): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia(query).matches
    : false;
}

export async function initializePlatformCapabilities(): Promise<void> {
  nativeSnapshot = await getNativePlatformSnapshot();
  for (const listener of listeners) listener();
}

export function subscribePlatformCapabilities(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getPlatformCapabilities(): PlatformCapabilities {
  return {
    coarsePointer: matches("(pointer: coarse)"),
    hoverAvailable: matches("(hover: hover)"),
    hardwareBackAvailable: nativeSnapshot?.hardwareBackAvailable ?? false,
    audioAvailable: typeof Audio !== "undefined",
    hapticsAvailable: nativeSnapshot?.services.haptics.state === "available",
    nativeFilePickerAvailable:
      nativeSnapshot?.services.externalFileImport.state === "available",
    native: nativeSnapshot,
  };
}
