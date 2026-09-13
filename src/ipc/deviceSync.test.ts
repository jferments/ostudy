import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cancelDeviceSyncSession,
  getPendingUsbSyncRequest,
  joinDevicePairing,
  joinUsbDevicePairing,
  rejectUsbDevicePairing,
  requestUsbAccessoryPermission,
  resolveDeviceSyncConflict,
  scanDevicePairingQr,
  startDevicePairing,
} from "./deviceSync";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("device synchronization IPC", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it("starts a bounded one-way LAN sender offer with explicit granular scope", async () => {
    const selection = {
      flashcards: true,
      documents: true,
      studyGuides: true,
      apiSettings: true,
      apiCredentials: true,
    };
    await startDevicePairing({
      hostOverride: "192.168.1.42",
      selection,
      expiresInSeconds: 300,
    });

    expect(invoke).toHaveBeenCalledWith("start_device_pairing", {
      input: {
        transport: "local-network",
        hostOverride: "192.168.1.42",
        selection,
        expiresInSeconds: 300,
      },
    });
  });

  it("starts direct USB synchronization without a host address", async () => {
    const selection = {
      flashcards: true,
      documents: true,
      studyGuides: true,
      apiSettings: false,
      apiCredentials: false,
    };
    await startDevicePairing({ transport: "usb", selection });

    expect(invoke).toHaveBeenCalledWith("start_device_pairing", {
      input: {
        transport: "usb",
        hostOverride: null,
        selection,
        expiresInSeconds: null,
      },
    });
  });

  it("accepts a LAN offer using only the sender invitation", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ sessionId: "example" });
    await joinDevicePairing("ostudy-sync://pair/example");

    expect(invoke).toHaveBeenCalledWith("join_device_pairing", {
      input: {
        invitation: "ostudy-sync://pair/example",
      },
    });
  });

  it("peeks at an attached USB request without claiming it", async () => {
    await getPendingUsbSyncRequest();
    expect(invoke).toHaveBeenCalledWith("get_pending_usb_sync_request");
  });

  it("requests Android USB permission only through the dedicated native plugin boundary", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ available: true, granted: true });
    await requestUsbAccessoryPermission();
    expect(invoke).toHaveBeenCalledWith("plugin:ostudy-usb|requestUsbAccessoryPermission");
  });

  it("accepts an attached USB accessory through the shared join command", async () => {
    await joinUsbDevicePairing();
    expect(invoke).toHaveBeenCalledWith("join_device_pairing", {
      input: {
        invitation: "ostudy-sync://usb-attached",
      },
    });
  });

  it("dismisses a pre-permission USB request without provoking Android permission", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ dismissed: true, canRespond: false });
    await expect(rejectUsbDevicePairing()).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("plugin:ostudy-usb|dismissUsbAccessory");
  });

  it("sends the typed rejection when Android USB permission is already available", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ dismissed: true, canRespond: true })
      .mockResolvedValueOnce(true);
    await expect(rejectUsbDevicePairing()).resolves.toBe(true);
    expect(invoke).toHaveBeenNthCalledWith(1, "plugin:ostudy-usb|dismissUsbAccessory");
    expect(invoke).toHaveBeenNthCalledWith(2, "reject_usb_device_pairing");
  });

  it("cancels one active synchronization session by id", async () => {
    const sessionId = "b".repeat(32);
    await cancelDeviceSyncSession(sessionId);
    expect(invoke).toHaveBeenCalledWith("cancel_device_sync_session", {
      input: { sessionId },
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
