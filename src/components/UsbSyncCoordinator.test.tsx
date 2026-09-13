import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as syncIpc from "../ipc/deviceSync";
import { UsbSyncCoordinator } from "./UsbSyncCoordinator";

vi.mock("../platform/PlatformProvider", () => ({
  usePlatformCapabilities: () => ({ native: { operatingSystem: "android" } }),
}));
vi.mock("../ipc/platform", () => ({
  listenToAndroidUsbSync: vi.fn().mockResolvedValue(vi.fn()),
}));
vi.mock("../ipc/deviceSync", () => ({
  getPendingUsbSyncRequest: vi.fn(),
  joinUsbDevicePairing: vi.fn(),
  requestUsbAccessoryPermission: vi.fn(),
  listenToDeviceSyncProgress: vi.fn().mockResolvedValue(vi.fn()),
}));

afterEach(cleanup);

describe("UsbSyncCoordinator", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(syncIpc.getPendingUsbSyncRequest)
      .mockResolvedValueOnce({
        available: true,
        manufacturer: "oStudy",
        model: "oStudy USB Sync",
        serialNumber: "session",
      })
      .mockResolvedValue({
        available: false,
        manufacturer: null,
        model: null,
        serialNumber: null,
      });
    vi.mocked(syncIpc.requestUsbAccessoryPermission).mockResolvedValue({
      available: true,
      granted: true,
    });
    vi.mocked(syncIpc.joinUsbDevicePairing).mockResolvedValue({
      sessionId: "a".repeat(32),
      peer: {
        id: "b".repeat(32),
        displayName: "Desktop",
        publicKeyBase64: "key",
        fingerprint: "f".repeat(64),
        endpointHint: "usb",
        pairedAt: 1,
        lastSyncedAt: 1,
        revision: 1,
      },
      changedRowCount: 3,
      conflictCount: 0,
      transferredCredentialCount: 0,
      credentialWarning: null,
      sentPackageBytes: 0,
      receivedPackageBytes: 1024,
    });
    vi.mocked(syncIpc.listenToDeviceSyncProgress).mockResolvedValue(vi.fn());
  });

  it("does not add a second app-owned consent dialog after Android routes the accessory", async () => {
    render(<UsbSyncCoordinator refreshDecks={vi.fn()} showToast={vi.fn()} />);

    await waitFor(() => expect(syncIpc.requestUsbAccessoryPermission).toHaveBeenCalledOnce());
    expect(screen.queryByRole("alertdialog", { name: /USB sync request/i })).toBeNull();
    await waitFor(() => expect(syncIpc.joinUsbDevicePairing).toHaveBeenCalledOnce());
  });

  it("does not start the transfer when Android does not grant USB access", async () => {
    const showToast = vi.fn();
    vi.mocked(syncIpc.requestUsbAccessoryPermission).mockResolvedValue({
      available: true,
      granted: false,
    });

    render(<UsbSyncCoordinator refreshDecks={vi.fn()} showToast={showToast} />);

    await waitFor(() => expect(syncIpc.requestUsbAccessoryPermission).toHaveBeenCalledOnce());
    expect(syncIpc.joinUsbDevicePairing).not.toHaveBeenCalled();
    expect(showToast).toHaveBeenCalledWith("info", "Android did not grant USB access to oStudy.");
  });

  it("receives automatically after Android system consent and refreshes the library", async () => {
    const showToast = vi.fn();
    const refreshDecks = vi.fn().mockResolvedValue(undefined);

    render(<UsbSyncCoordinator refreshDecks={refreshDecks} showToast={showToast} />);

    await waitFor(() => expect(syncIpc.requestUsbAccessoryPermission).toHaveBeenCalledOnce());
    await waitFor(() => expect(syncIpc.joinUsbDevicePairing).toHaveBeenCalledOnce());
    const permissionOrder = vi.mocked(syncIpc.requestUsbAccessoryPermission).mock.invocationCallOrder[0];
    const joinOrder = vi.mocked(syncIpc.joinUsbDevicePairing).mock.invocationCallOrder[0];
    expect(permissionOrder).toBeDefined();
    expect(joinOrder).toBeDefined();
    expect(permissionOrder ?? Number.MAX_SAFE_INTEGER).toBeLessThan(
      joinOrder ?? Number.MAX_SAFE_INTEGER,
    );
    await waitFor(() => expect(refreshDecks).toHaveBeenCalledOnce());
    expect(showToast).toHaveBeenCalledWith(
      "success",
      expect.stringContaining("USB sync received from Desktop"),
    );
  });
});
