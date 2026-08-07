export interface PlatformCapabilities {
  coarsePointer: boolean;
  hoverAvailable: boolean;
  hardwareBackAvailable: boolean;
  audioAvailable: boolean;
  hapticsAvailable: boolean;
  nativeFilePickerAvailable: boolean;
}

function matches(query: string): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia(query).matches
    : false;
}

export function getPlatformCapabilities(): PlatformCapabilities {
  const userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent;
  const android = /Android/iu.test(userAgent);
  return {
    coarsePointer: matches("(pointer: coarse)"),
    hoverAvailable: matches("(hover: hover)"),
    hardwareBackAvailable: android,
    audioAvailable: typeof Audio !== "undefined",
    hapticsAvailable: typeof navigator !== "undefined" && "vibrate" in navigator,
    nativeFilePickerAvailable: true,
  };
}
