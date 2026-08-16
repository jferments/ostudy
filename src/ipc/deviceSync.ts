import { invoke } from "@tauri-apps/api/core";

export interface DeviceSyncIdentity {
  deviceId: string;
  displayName: string;
  publicKeyBase64: string;
}

export interface DeviceSyncPeer {
  id: string;
  displayName: string;
  publicKeyBase64: string;
  fingerprint: string;
  endpointHint: string;
  pairedAt: number;
  lastSyncedAt: number | null;
  revision: number;
}

export interface DeviceSyncConflict {
  id: string;
  peerId: string;
  tableName: string;
  rowKey: string;
  leftDeviceId: string;
  leftPayloadJson: string | null;
  rightDeviceId: string;
  rightPayloadJson: string | null;
  selectedPayloadJson: string | null;
  conflictingFields: string[];
  createdAt: number;
}

export interface DeviceSyncSession {
  id: string;
  peerId: string | null;
  direction: "host" | "join";
  state:
    | "waiting"
    | "connecting"
    | "transferring"
    | "merging"
    | "completed"
    | "failed"
    | "expired"
    | "canceled";
  endpointHint: string;
  createdAt: number;
  expiresAt: number;
  completedAt: number | null;
  errorCode: string | null;
  errorMessage: string | null;
}

export interface DeviceSyncState {
  identity: DeviceSyncIdentity;
  fingerprint: string;
  peers: DeviceSyncPeer[];
  conflicts: DeviceSyncConflict[];
  sessions: DeviceSyncSession[];
  localNetworkAvailable: boolean;
  qrScanAvailable: boolean;
  secureCredentialsAvailable: boolean;
}

export interface PairingInvitation {
  sessionId: string;
  invitation: string;
  qrSvg: string;
  endpoint: string;
  fingerprint: string;
  expiresAt: number;
  includeApiSettings: boolean;
  credentialsAvailable: boolean;
}

export interface DeviceSyncResult {
  sessionId: string;
  peer: DeviceSyncPeer;
  changedRowCount: number;
  conflictCount: number;
  transferredCredentialCount: number;
  sentPackageBytes: number;
  receivedPackageBytes: number;
}

export function getDeviceSyncState(): Promise<DeviceSyncState> {
  return invoke<DeviceSyncState>("get_device_sync_state");
}

export function updateDeviceSyncName(displayName: string): Promise<DeviceSyncIdentity> {
  return invoke<DeviceSyncIdentity>("update_device_sync_name", {
    input: { displayName },
  });
}

export function startDevicePairing(input: {
  hostOverride?: string | null;
  includeApiSettings: boolean;
  expiresInSeconds?: number | null;
}): Promise<PairingInvitation> {
  return invoke<PairingInvitation>("start_device_pairing", {
    input: {
      hostOverride: input.hostOverride ?? null,
      includeApiSettings: input.includeApiSettings,
      expiresInSeconds: input.expiresInSeconds ?? null,
    },
  });
}

export function joinDevicePairing(invitation: string): Promise<DeviceSyncResult> {
  return invoke<DeviceSyncResult>("join_device_pairing", {
    input: { invitation },
  });
}

export function scanDevicePairingQr(): Promise<string | null> {
  return invoke<string | null>("scan_device_pairing_qr_async");
}

export function removeDeviceSyncPeer(peerId: string): Promise<void> {
  return invoke<void>("remove_device_sync_peer", {
    input: { peerId },
  });
}

export function resolveDeviceSyncConflict(input: {
  conflictId: string;
  resolution: "keep_left" | "keep_right" | "merged" | "delete";
  mergedPayloadJson?: string | null;
}): Promise<void> {
  return invoke<void>("resolve_device_sync_conflict", {
    input: {
      conflictId: input.conflictId,
      resolution: input.resolution,
      mergedPayloadJson: input.mergedPayloadJson ?? null,
    },
  });
}
