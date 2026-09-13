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

    expect(screen.queryByRole("alertdialog", { name: /USB sync request/i })).toBeNull();
    await waitFor(() => expect(syncIpc.joinUsbDevicePairing).toHaveBeenCalledOnce());
  });

  it("surfaces a native USB access rejection without refreshing the library", async () => {
    const showToast = vi.fn();
    const refreshDecks = vi.fn();
    vi.mocked(syncIpc.joinUsbDevicePairing).mockRejectedValueOnce(
      new Error("The operating system did not grant USB access to oStudy."),
    );

    render(<UsbSyncCoordinator refreshDecks={refreshDecks} showToast={showToast} />);

    await waitFor(() => expect(syncIpc.joinUsbDevicePairing).toHaveBeenCalledOnce());
    expect(refreshDecks).not.toHaveBeenCalled();
    expect(showToast).toHaveBeenCalledWith(
      "error",
      "The operating system did not grant USB access to oStudy.",
    );
  });

  it("receives automatically after Android system consent and refreshes the library", async () => {
    const showToast = vi.fn();
    const refreshDecks = vi.fn().mockResolvedValue(undefined);

    render(<UsbSyncCoordinator refreshDecks={refreshDecks} showToast={showToast} />);

    await waitFor(() => expect(syncIpc.joinUsbDevicePairing).toHaveBeenCalledOnce());
    await waitFor(() => expect(refreshDecks).toHaveBeenCalledOnce());
    expect(showToast).toHaveBeenCalledWith(
      "success",
      expect.stringContaining("USB sync received from Desktop"),
    );
  });
});
