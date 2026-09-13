import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as syncIpc from "../ipc/deviceSync";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
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
  rejectUsbDevicePairing: vi.fn(),
  listenToDeviceSyncProgress: vi.fn().mockResolvedValue(vi.fn()),
}));

afterEach(cleanup);

describe("UsbSyncCoordinator", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(syncIpc.getPendingUsbSyncRequest).mockResolvedValue({
      available: true,
      manufacturer: "oStudy",
      model: "oStudy USB Sync",
      serialNumber: "session",
    });
    vi.mocked(syncIpc.rejectUsbDevicePairing).mockResolvedValue(true);
    vi.mocked(syncIpc.joinUsbDevicePairing).mockResolvedValue({
      sessionId: "a".repeat(32),
      peer: {
        id: "b".repeat(32), displayName: "Desktop", publicKeyBase64: "key", fingerprint: "f".repeat(64),
        endpointHint: "usb", pairedAt: 1, lastSyncedAt: 1, revision: 1,
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

  it("requires an explicit human Accept or Reject decision before USB receive", async () => {
    const showToast = vi.fn();
    render(<OverlayProvider><UsbSyncCoordinator refreshDecks={vi.fn()} showToast={showToast} /></OverlayProvider>);

    const dialog = await screen.findByRole("alertdialog", { name: "USB sync request" });
    expect(dialog.textContent).toContain("Would you like to accept a USB sync request?");
    expect(dialog.textContent).toContain("flashcards, documents, study guides, and API provider information");
    expect(screen.getByRole("button", { name: "Accept" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Reject" })).toBeTruthy();
    expect(syncIpc.joinUsbDevicePairing).not.toHaveBeenCalled();
  });

  it("returns an explicit rejection to the sending device", async () => {
    const showToast = vi.fn();
    render(<OverlayProvider><UsbSyncCoordinator refreshDecks={vi.fn()} showToast={showToast} /></OverlayProvider>);
    await screen.findByRole("alertdialog", { name: "USB sync request" });
    fireEvent.click(screen.getByRole("button", { name: "Reject" }));
    await waitFor(() => expect(syncIpc.rejectUsbDevicePairing).toHaveBeenCalledOnce());
    expect(showToast).toHaveBeenCalledWith("info", "USB sync request rejected.");
  });

  it("accepts the request and refreshes the library after the transfer completes", async () => {
    const showToast = vi.fn();
    const refreshDecks = vi.fn().mockResolvedValue(undefined);
    render(<OverlayProvider><UsbSyncCoordinator refreshDecks={refreshDecks} showToast={showToast} /></OverlayProvider>);
    await screen.findByRole("alertdialog", { name: "USB sync request" });
    fireEvent.click(screen.getByRole("button", { name: "Accept" }));
    await waitFor(() => expect(syncIpc.joinUsbDevicePairing).toHaveBeenCalledOnce());
    await waitFor(() => expect(refreshDecks).toHaveBeenCalledOnce());
    expect(showToast).toHaveBeenCalledWith("success", expect.stringContaining("USB sync received from Desktop"));
  });
});
