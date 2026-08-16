import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  joinDevicePairing,
  resolveDeviceSyncConflict,
  scanDevicePairingQr,
  startDevicePairing,
} from "./deviceSync";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("device synchronization IPC", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("starts a bounded pairing listener with explicit credential consent", async () => {
    await startDevicePairing({
      hostOverride: "192.168.1.42",
      includeCredentials: true,
      expiresInSeconds: 300,
    });

    expect(invoke).toHaveBeenCalledWith("start_device_pairing", {
      input: {
        hostOverride: "192.168.1.42",
        includeCredentials: true,
        expiresInSeconds: 300,
      },
    });
  });

  it("joins using only the invitation and explicit credential consent", async () => {
    await joinDevicePairing({
      invitation: "ostudy-sync://pair/example",
      includeCredentials: false,
    });

    expect(invoke).toHaveBeenCalledWith("join_device_pairing", {
      input: {
        invitation: "ostudy-sync://pair/example",
        includeCredentials: false,
      },
    });
  });

  it("routes QR scanning through the nonblocking native command boundary", async () => {
    await scanDevicePairingQr();
    expect(invoke).toHaveBeenCalledWith("scan_device_pairing_qr_async");
  });

  it("sends an explicit conflict resolution without leaking arbitrary fields", async () => {
    await resolveDeviceSyncConflict({
      conflictId: "a".repeat(32),
      resolution: "keep_left",
    });

    expect(invoke).toHaveBeenCalledWith("resolve_device_sync_conflict", {
      input: {
        conflictId: "a".repeat(32),
        resolution: "keep_left",
        mergedPayloadJson: null,
      },
    });
  });
});
