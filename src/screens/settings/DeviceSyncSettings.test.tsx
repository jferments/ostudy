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
  cancelDeviceSyncSession: vi.fn(),
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

function activeHostSession(): syncIpc.DeviceSyncSession {
  return {
    id: "6".repeat(32),
    peerId: null,
    direction: "host",
    state: "waiting",
    endpointHint: "192.168.1.10:44123",
    createdAt: Date.now(),
    expiresAt: Date.now() + 300_000,
    completedAt: null,
    errorCode: null,
    errorMessage: null,
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
      includeApiSettings: false,
      credentialsAvailable: false,
    });
    vi.mocked(syncIpc.scanDevicePairingQr).mockResolvedValue("ostudy-sync://pair/scanned");
    vi.mocked(syncIpc.joinDevicePairing).mockResolvedValue({
      sessionId: "5".repeat(32),
      peer: state().peers[0],
      changedRowCount: 12,
      conflictCount: 1,
      transferredCredentialCount: 0,
      credentialWarning: null,
      sentPackageBytes: 0,
      receivedPackageBytes: 20,
    });
    vi.mocked(syncIpc.cancelDeviceSyncSession).mockResolvedValue(undefined);
    vi.mocked(syncIpc.resolveDeviceSyncConflict).mockResolvedValue(undefined);
  });

  it("creates a one-way sender offer with API sharing off by default", async () => {
    renderSync();
    expect(await screen.findByText("Android phone")).toBeTruthy();
    expect(screen.getByText(/Appearance and other app settings always stay local/iu)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Create sync offer" }));

    await waitFor(() => {
      expect(syncIpc.startDevicePairing).toHaveBeenCalledWith({
        hostOverride: null,
        includeApiSettings: false,
      });
    });
    expect(await screen.findByAltText(/QR code containing the short-lived ostudy synchronization offer/u)).toBeTruthy();
    expect(screen.getByText(/192\.168\.1\.10:44000/u)).toBeTruthy();
    expect(screen.getByText(/API provider settings and credentials are not included/iu)).toBeTruthy();
    expect(screen.getByText(/scanning this code accepts the offer/iu)).toBeTruthy();
  });

  it("uses the same authenticated pairing command for USB-network synchronization", async () => {
    renderSync();
    await screen.findByText("Android phone");

    fireEvent.change(screen.getByLabelText(/Connection/iu), {
      target: { value: "usb-network" },
    });
    const usbButton = screen.getByRole("button", { name: "Create USB sync offer" });
    expect((usbButton as HTMLButtonElement).disabled).toBe(true);

    fireEvent.change(screen.getByLabelText(/USB interface address/iu), {
      target: { value: "192.168.42.17" },
    });
    expect((usbButton as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(usbButton);

    await waitFor(() => {
      expect(syncIpc.startDevicePairing).toHaveBeenCalledWith({
        hostOverride: "192.168.42.17",
        includeApiSettings: false,
      });
    });
    expect(screen.getByText(/USB · 192\.168\.1\.10:44000/u)).toBeTruthy();
  });

  it("lets the sender explicitly add API settings to the offer", async () => {
    renderSync();
    await screen.findByText("Android phone");
    const apiToggle = screen.getByRole("checkbox", {
      name: /Include API provider settings and saved credentials/iu,
    });
    fireEvent.click(apiToggle);
    fireEvent.click(screen.getByRole("button", { name: "Create sync offer" }));

    await waitFor(() => {
      expect(syncIpc.startDevicePairing).toHaveBeenCalledWith({
        hostOverride: null,
        includeApiSettings: true,
      });
    });
  });

  it("shows one active waiting offer as replaceable and lets the user cancel it", async () => {
    const waiting = activeHostSession();
    vi.mocked(syncIpc.getDeviceSyncState)
      .mockResolvedValueOnce({ ...state(), sessions: [waiting] })
      .mockResolvedValue(state());
    const showToast = renderSync();

    expect(await screen.findByRole("button", { name: "Replace waiting offer" })).toBeTruthy();
    expect(screen.getByText(/Waiting/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel sending request" }));

    await waitFor(() => {
      expect(syncIpc.cancelDeviceSyncSession).toHaveBeenCalledWith(waiting.id);
    });
    expect(showToast).toHaveBeenCalledWith("info", "Sending request canceled.");
  });

  it("accepts a scanned offer immediately without a second pairing action", async () => {
    const showToast = renderSync();
    await screen.findByText("Android phone");
    fireEvent.click(screen.getByRole("button", { name: "Scan and receive" }));

    await waitFor(() => {
      expect(syncIpc.joinDevicePairing).toHaveBeenCalledTimes(1);
      expect(syncIpc.joinDevicePairing).toHaveBeenCalledWith("ostudy-sync://pair/scanned");
    });
    expect(screen.getByRole("status").textContent).toContain("synchronization complete");
    expect(showToast).toHaveBeenCalledWith(
      "info",
      "Received from Android phone: 12 rows changed and 1 conflicts need review.",
    );
    expect(screen.queryByDisplayValue("ostudy-sync://pair/scanned")).toBeNull();
  });

  it("turns a failed automatic scan into an explicit retry without rescanning", async () => {
    vi.mocked(syncIpc.joinDevicePairing).mockRejectedValueOnce(
      new Error("The sending device could not be reached at 192.168.1.10:44000: timed out"),
    );
    renderSync();
    await screen.findByText("Android phone");
    fireEvent.click(screen.getByRole("button", { name: "Scan and receive" }));

    expect(await screen.findByRole("button", { name: "Retry connection" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("connection did not complete");
    expect(screen.getByText(/sending device could not be reached/iu)).toBeTruthy();
    expect(syncIpc.joinDevicePairing).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Retry connection" }));
    await waitFor(() => expect(syncIpc.joinDevicePairing).toHaveBeenCalledTimes(2));
  });

  it("reports optional credential-store failure without failing the received library", async () => {
    vi.mocked(syncIpc.joinDevicePairing).mockResolvedValueOnce({
      sessionId: "5".repeat(32),
      peer: state().peers[0],
      changedRowCount: 12,
      conflictCount: 0,
      transferredCredentialCount: 0,
      credentialWarning:
        "Study data synchronized, but saved API credentials could not be stored securely on this device. Re-enter them in Settings → AI if needed.",
      sentPackageBytes: 0,
      receivedPackageBytes: 20,
    });
    const showToast = renderSync();
    await screen.findByText("Android phone");
    fireEvent.change(screen.getByLabelText("Synchronization invitation"), {
      target: { value: "ostudy-sync://pair/credential-warning" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Accept sync offer" }));

    await waitFor(() => expect(syncIpc.joinDevicePairing).toHaveBeenCalledTimes(1));
    expect(showToast).toHaveBeenCalledWith(
      "info",
      expect.stringContaining("Study data synchronized, but saved API credentials could not be stored securely"),
    );
  });

  it("accepts pasted invitation text without a receiver-side API toggle", async () => {
    renderSync();
    await screen.findByText("Android phone");
    expect(
      screen.getAllByRole("checkbox", {
        name: /Include API provider settings and saved credentials/iu,
      }),
    ).toHaveLength(1);

    fireEvent.change(screen.getByLabelText("Synchronization invitation"), {
      target: { value: "ostudy-sync://pair/pasted" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Accept sync offer" }));

    await waitFor(() => {
      expect(syncIpc.joinDevicePairing).toHaveBeenCalledWith("ostudy-sync://pair/pasted");
    });
  });

  it("reports a cancelled scanner without starting synchronization", async () => {
    vi.mocked(syncIpc.scanDevicePairingQr).mockResolvedValueOnce(null);
    renderSync();
    await screen.findByText("Android phone");
    fireEvent.click(screen.getByRole("button", { name: "Scan and receive" }));
    expect(await screen.findByText(/Scan cancelled/u)).toBeTruthy();
    expect(syncIpc.joinDevicePairing).not.toHaveBeenCalled();
    const invitation = screen.getByLabelText("Synchronization invitation") as HTMLTextAreaElement;
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
