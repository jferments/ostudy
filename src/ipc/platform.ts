import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type CapabilityState =
  | "available"
  | "permission_required"
  | "denied"
  | "unavailable"
  | "degraded";

export interface ServiceCapability {
  state: CapabilityState;
  reasonCode: string | null;
  canRequestPermission: boolean;
}

export interface NativeServiceCapabilities {
  externalFileImport: ServiceCapability;
  externalFileExport: ServiceCapability;
  secureSecrets: ServiceCapability;
  lifecycleEvents: ServiceCapability;
  localTimeZone: ServiceCapability;
  resourceObservation: ServiceCapability;
  nativePdfRender: ServiceCapability;
  nativePdfTextRescue: ServiceCapability;
  ocr: ServiceCapability;
  camera: ServiceCapability;
  qrScan: ServiceCapability;
  haptics: ServiceCapability;
  systemShare: ServiceCapability;
  localNetwork: ServiceCapability;
  backgroundExecution: ServiceCapability;
}

export interface NativePlatformSnapshot {
  schemaVersion: number;
  runtimeFamily: "desktop" | "mobile";
  operatingSystem: string;
  hardwareBackAvailable: boolean;
  services: NativeServiceCapabilities;
}

export type LifecyclePhase =
  | "starting"
  | "foreground"
  | "background"
  | "suspending"
  | "resuming"
  | "termination_requested"
  | "terminating";

export interface LifecycleSnapshot {
  schemaVersion: number;
  sequence: number;
  phase: LifecyclePhase;
  observedAtUtcMillis: number;
}

export async function getNativePlatformSnapshot(): Promise<NativePlatformSnapshot> {
  return invoke<NativePlatformSnapshot>("get_platform_capabilities");
}

export async function listenToLifecycle(
  listener: (snapshot: LifecycleSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<LifecycleSnapshot>("ostudy-lifecycle", (event) => listener(event.payload));
}
