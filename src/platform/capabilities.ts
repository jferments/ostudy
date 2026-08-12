import {
  getNativePlatformSnapshot,
  type AndroidInsetsSnapshot,
  type AndroidNetworkSnapshot,
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
  androidInsets: AndroidInsetsSnapshot | null;
  network: AndroidNetworkSnapshot | null;
}

let nativeSnapshot: NativePlatformSnapshot | null = null;
let androidInsets: AndroidInsetsSnapshot | null = null;
let network: AndroidNetworkSnapshot | null = null;
const listeners = new Set<() => void>();

function matches(query: string): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia(query).matches
    : false;
}

export async function initializePlatformCapabilities(): Promise<void> {
  nativeSnapshot = await getNativePlatformSnapshot();
  publish();
}

export function updateAndroidInsets(snapshot: AndroidInsetsSnapshot): void {
  androidInsets = snapshot;
  publish();
}

export function updateAndroidNetwork(snapshot: AndroidNetworkSnapshot): void {
  network = snapshot;
  publish();
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
    androidInsets,
    network,
  };
}

function publish(): void {
  for (const listener of listeners) listener();
}
