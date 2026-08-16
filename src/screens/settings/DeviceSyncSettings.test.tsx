import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as syncIpc from "../../ipc/deviceSync";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { DeviceSyncSettings } from "./DeviceSyncSettings";

vi.mock("../../ipc/deviceSync", () => ({
  getDeviceSyncState: vi.fn(),
  updateDeviceSyncName: vi.fn(),
  startDevicePairing: vi.fn(),
  joinDevicePairing: vi.fn(),
  scanDevicePairingQr: vi.fn(),
  removeDeviceSyncPeer: vi.fn(),
  resolveDeviceSyncConflict: vi.fn(),
}));

const localId = "1".repeat(32);
const peerId = "2".repeat(32);

function state(): syncIpc.DeviceSyncState {
  return {
    identity: {
      deviceId: localId,
      displayName: "Desktop",
      publicKeyBase64: "public",
    },
    fingerprint: "a".repeat(64),
    peers: [
      {
        id: peerId,
        displayName: "Android phone",
        publicKeyBase64: "peer-public",
        fingerprint: "b".repeat(64),
        endpointHint: "192.168.1.22:44000",
        pairedAt: 1_700_000_000_000,
        lastSyncedAt: 1_700_000_100_000,
        revision: 1,
      },
    ],
    conflicts: [
      {
        id: "3".repeat(32),
        peerId,
        tableName: "notes",
        rowKey: "row-key",
        leftDeviceId: localId,
        leftPayloadJson: '{"front_text":{"kind":"text","value":"Local"}}',
        rightDeviceId: peerId,
        rightPayloadJson: '{"front_text":{"kind":"text","value":"Remote"}}',
        selectedPayloadJson: null,
        conflictingFields: ["front_text"],
        createdAt: 1_700_000_200_000,
      },
    ],
    sessions: [],
    localNetworkAvailable: true,
    qrScanAvailable: true,
    secureCredentialsAvailable: true,
  };
}

function renderSync(showToast = vi.fn()) {
  render(
    <OverlayProvider>
      <DeviceSyncSettings showToast={showToast} />
    </OverlayProvider>,
  );
  return showToast;
}

describe("DeviceSyncSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(syncIpc.getDeviceSyncState).mockResolvedValue(state());
    vi.mocked(syncIpc.startDevicePairing).mockResolvedValue({
      sessionId: "4".repeat(32),
      invitation: "ostudy-sync://pair/example",
      qrSvg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>',
      endpoint: "192.168.1.10:44000",
      fingerprint: "a".repeat(64),
      expiresAt: Date.now() + 300_000,
      credentialsAvailable: false,
    });
    vi.mocked(syncIpc.scanDevicePairingQr).mockResolvedValue("ostudy-sync://pair/scanned");
    vi.mocked(syncIpc.joinDevicePairing).mockResolvedValue({
      sessionId: "5".repeat(32),
      peer: state().peers[0],
      changedRowCount: 12,
      conflictCount: 1,
      transferredCredentialCount: 0,
      sentPackageBytes: 10,
      receivedPackageBytes: 20,
    });
    vi.mocked(syncIpc.resolveDeviceSyncConflict).mockResolvedValue(undefined);
  });

  it("creates a large pairing-code presentation with scanning guidance", async () => {
    renderSync();
    expect(await screen.findByText("Android phone")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Create pairing code" }));

    await waitFor(() => {
      expect(syncIpc.startDevicePairing).toHaveBeenCalledWith({
        hostOverride: null,
        includeCredentials: false,
      });
    });
    expect(await screen.findByAltText(/QR code containing/u)).toBeTruthy();
    expect(screen.getByText("192.168.1.10:44000")).toBeTruthy();
    expect(screen.getByText(/entire white border visible/u)).toBeTruthy();
    expect(screen.getByText(/center the complete QR code inside the camera frame/iu)).toBeTruthy();
  });

  it("shows visual detection feedback after the Android scanner returns a pairing code", async () => {
    const showToast = renderSync();
    await screen.findByText("Android phone");
    fireEvent.click(screen.getByRole("button", { name: "Scan pairing QR code" }));
    expect(await screen.findByDisplayValue("ostudy-sync://pair/scanned")).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Pairing code detected");
    expect(showToast).toHaveBeenCalledWith(
      "success",
      "Pairing code detected. Review the credential option, then connect.",
    );

    fireEvent.click(screen.getByRole("button", { name: "Pair and synchronize" }));
    await waitFor(() => {
      expect(syncIpc.joinDevicePairing).toHaveBeenCalledWith({
        invitation: "ostudy-sync://pair/scanned",
        includeCredentials: false,
      });
      expect(showToast).toHaveBeenCalledWith(
        "info",
        "Synchronized with Android phone: 12 rows changed and 1 conflicts need review.",
      );
    });
  });

  it("reports a cancelled scanner without replacing the invitation", async () => {
    vi.mocked(syncIpc.scanDevicePairingQr).mockResolvedValueOnce(null);
    renderSync();
    await screen.findByText("Android phone");
    fireEvent.click(screen.getByRole("button", { name: "Scan pairing QR code" }));
    expect(await screen.findByText(/Scan cancelled/u)).toBeTruthy();
    const invitation = screen.getByLabelText("Pairing invitation") as HTMLTextAreaElement;
    expect(invitation.value).toBe("");
  });

  it("keeps concurrent card edits visible until an explicit side is selected", async () => {
    vi.mocked(syncIpc.getDeviceSyncState)
      .mockResolvedValueOnce(state())
      .mockResolvedValueOnce({ ...state(), conflicts: [] });
    renderSync();
    expect(await screen.findByText("Notes")).toBeTruthy();
    expect(screen.getByText("front_text")).toBeTruthy();
    expect(screen.getByText(/Local/u)).toBeTruthy();
    expect(screen.getByText(/Remote/u)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Keep left" }));
    await waitFor(() => {
      expect(syncIpc.resolveDeviceSyncConflict).toHaveBeenCalledWith({
        conflictId: "3".repeat(32),
        resolution: "keep_left",
      });
    });
  });
});
