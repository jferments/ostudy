import {
  addPluginListener,
  invoke,
  type PluginListener,
} from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const PLATFORM_PLUGIN = "ostudy-platform";

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
  sequence?: number;
  phase: LifecyclePhase;
  observedAtUtcMillis: number;
}

export interface AndroidInsetsSnapshot {
  top: number;
  right: number;
  bottom: number;
  left: number;
  imeBottom: number;
  imeVisible: boolean;
}

export interface AndroidNetworkSnapshot {
  online: boolean;
  metered: boolean;
  validatedInternet: boolean;
  transport: "wifi" | "cellular" | "ethernet" | "vpn" | "other" | "none";
}

export interface IncomingShareSummary {
  token: string;
  displayName: string;
  reportedMediaType: string | null;
  byteSize: number;
}

export async function getNativePlatformSnapshot(): Promise<NativePlatformSnapshot> {
  return invoke<NativePlatformSnapshot>("get_platform_capabilities");
}

export async function listenToLifecycle(
  listener: (snapshot: LifecycleSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<LifecycleSnapshot>("ostudy-lifecycle", (event) => listener(event.payload));
}

export function setPlatformBackInterception(enabled: boolean): Promise<void> {
  return invoke<void>("set_platform_back_interception", { enabled });
}

export async function listenToAndroidBack(listener: () => void): Promise<UnlistenFn> {
  return pluginUnlisten(await addPluginListener<Record<string, never>>(PLATFORM_PLUGIN, "back", listener));
}

export async function listenToAndroidLifecycle(
  listener: (snapshot: LifecycleSnapshot) => void,
): Promise<UnlistenFn> {
  return pluginUnlisten(
    await addPluginListener<LifecycleSnapshot>(PLATFORM_PLUGIN, "lifecycle", listener),
  );
}

export async function listenToAndroidInsets(
  listener: (snapshot: AndroidInsetsSnapshot) => void,
): Promise<UnlistenFn> {
  return pluginUnlisten(
    await addPluginListener<AndroidInsetsSnapshot>(PLATFORM_PLUGIN, "insets", listener),
  );
}

export async function listenToAndroidNetwork(
  listener: (snapshot: AndroidNetworkSnapshot) => void,
): Promise<UnlistenFn> {
  return pluginUnlisten(
    await addPluginListener<AndroidNetworkSnapshot>(PLATFORM_PLUGIN, "network", listener),
  );
}

export async function listenToIncomingShare(
  listener: (summary: IncomingShareSummary) => void,
): Promise<UnlistenFn> {
  return pluginUnlisten(
    await addPluginListener<IncomingShareSummary>(PLATFORM_PLUGIN, "incoming-share", listener),
  );
}

function pluginUnlisten(listener: PluginListener): UnlistenFn {
  return () => {
    void listener.unregister();
  };
}
