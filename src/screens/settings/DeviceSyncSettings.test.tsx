import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as syncIpc from "../../ipc/deviceSync";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { DeviceSyncSettings } from "./DeviceSyncSettings";

import { requiredTestValue as required } from "../../test/factories";
vi.mock("../../ipc/deviceSync", () => ({
  getDeviceSyncState: vi.fn(), updateDeviceSyncName: vi.fn(), startDevicePairing: vi.fn(),
  joinDevicePairing: vi.fn(), cancelDeviceSyncSession: vi.fn(), scanDevicePairingQr: vi.fn(),
  removeDeviceSyncPeer: vi.fn(), resolveDeviceSyncConflict: vi.fn(), listenToDeviceSyncProgress: vi.fn(),
}));
const localId = "1".repeat(32);
const peerId = "2".repeat(32);
const defaultSelection: syncIpc.DeviceSyncSelection = {
  flashcards: true, documents: true, studyGuides: true, notes: true, lessons: true, apiSettings: false, apiCredentials: false,
};
function state(): syncIpc.DeviceSyncState {
  return {
    identity: { deviceId: localId, displayName: "Desktop", publicKeyBase64: "public" },
    fingerprint: "a".repeat(64),
    peers: [{
      id: peerId, displayName: "Android phone", publicKeyBase64: "peer-public", fingerprint: "b".repeat(64),
      endpointHint: "192.168.1.22:44000", pairedAt: 1_700_000_000_000, lastSyncedAt: 1_700_000_100_000, revision: 1,
    }],
    conflicts: [{
      id: "3".repeat(32), peerId, tableName: "notes", rowKey: "row-key",
      leftDeviceId: localId, leftPayloadJson: '{"front_text":{"kind":"text","value":"Local"}}',
      rightDeviceId: peerId, rightPayloadJson: '{"front_text":{"kind":"text","value":"Remote"}}',
      selectedPayloadJson: null, conflictingFields: ["front_text"], createdAt: 1_700_000_200_000,
    }],
    sessions: [], localNetworkAvailable: true, usbHostAvailable: true,
    qrScanAvailable: true, secureCredentialsAvailable: true,
  };
}
function activeHostSession(): syncIpc.DeviceSyncSession {
  return {
    id: "6".repeat(32), peerId: null, direction: "host", state: "waiting", endpointHint: "192.168.1.10:44123",
    createdAt: Date.now(), expiresAt: Date.now() + 300_000, completedAt: null, errorCode: null, errorMessage: null,
  };
}
function renderSync(showToast = vi.fn()) {
  render(<OverlayProvider><DeviceSyncSettings showToast={showToast} /></OverlayProvider>);
  return showToast;
}
async function openSheet(button: "Send to a device" | "Receive") {
  await screen.findByText("Android phone");
  fireEvent.click(screen.getByRole("button", { name: button }));
  return screen.findByRole("dialog", { name: button });
}
const toggle = (sheet: HTMLElement, name: string) => within(sheet).getByRole("switch", { name });
afterEach(cleanup);

describe("DeviceSyncSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(syncIpc.getDeviceSyncState).mockResolvedValue(state());
    vi.mocked(syncIpc.listenToDeviceSyncProgress).mockResolvedValue(vi.fn());
    vi.mocked(syncIpc.startDevicePairing).mockResolvedValue({
      sessionId: "4".repeat(32), invitation: "ostudy-sync://pair/example",
      qrSvg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>', endpoint: "192.168.1.10:44000",
      transport: "local-network", fingerprint: "a".repeat(64), expiresAt: Date.now() + 300_000,
      includeApiSettings: false, credentialsAvailable: false, selection: defaultSelection,
    });
    vi.mocked(syncIpc.scanDevicePairingQr).mockResolvedValue("ostudy-sync://pair/scanned");
    vi.mocked(syncIpc.joinDevicePairing).mockResolvedValue({
      sessionId: "5".repeat(32), peer: required(state().peers[0], "sync peer"), changedRowCount: 12, conflictCount: 1,
      transferredCredentialCount: 0, credentialWarning: null, sentPackageBytes: 0, receivedPackageBytes: 20,
    });
    vi.mocked(syncIpc.cancelDeviceSyncSession).mockResolvedValue(undefined);
    vi.mocked(syncIpc.resolveDeviceSyncConflict).mockResolvedValue(undefined);
  });

  it("lists this device and trusted devices, with sending and receiving behind their own buttons", async () => {
    renderSync();
    const list = await screen.findByRole("list", { name: "Devices" });
    expect(within(list).getByText("Desktop")).toBeTruthy();
    expect(within(list).getByText("Android phone")).toBeTruthy();
    expect(screen.queryByRole("switch")).toBeNull();
    expect(screen.getByRole("button", { name: "Send to a device" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Receive" })).toBeTruthy();
  });

  it("creates a one-way network code with learning data on and AI sharing off by default", async () => {
    renderSync();
    const sheet = await openSheet("Send to a device");
    expect(toggle(sheet, "Flashcards").getAttribute("aria-checked")).toBe("true");
    expect(toggle(sheet, "AI services").getAttribute("aria-checked")).toBe("false");
    fireEvent.click(within(sheet).getByRole("button", { name: "Show a code" }));
    await waitFor(() => expect(syncIpc.startDevicePairing).toHaveBeenCalledWith({
      transport: "local-network", hostOverride: null, selection: defaultSelection,
    }));
    expect(await within(sheet).findByAltText("Sync code for the other device to scan")).toBeTruthy();
    expect(within(sheet).getByText(/192\.168\.1\.10:44000/u)).toBeTruthy();
    expect(within(sheet).getByRole("button", { name: "Copy as text" })).toBeTruthy();
  });

  it("starts direct USB sync with consent waiting and no network controls", async () => {
    const showToast = renderSync();
    const sheet = await openSheet("Send to a device");
    fireEvent.click(within(sheet).getByRole("radio", { name: "USB cable" }));
    expect(within(sheet).queryByLabelText("Network address")).toBeNull();
    fireEvent.click(within(sheet).getByRole("button", { name: "Send over USB" }));
    await waitFor(() => expect(syncIpc.startDevicePairing).toHaveBeenCalledWith({ transport: "usb", selection: defaultSelection }));
    expect(showToast).toHaveBeenCalledWith("info", expect.stringContaining("USB sync request sent"));
    expect(within(sheet).getByText(/Waiting for the phone to accept/iu)).toBeTruthy();
  });

  it("offers only the network when this build has no native USB host adapter", async () => {
    vi.mocked(syncIpc.getDeviceSyncState).mockResolvedValue({ ...state(), usbHostAvailable: false });
    renderSync();
    const sheet = await openSheet("Send to a device");
    expect(within(sheet).queryByRole("radio", { name: "USB cable" })).toBeNull();
    expect(within(sheet).getByRole("button", { name: "Show a code" })).toBeTruthy();
  });

  it("keeps the network address override for network codes", async () => {
    renderSync();
    const sheet = await openSheet("Send to a device");
    fireEvent.click(within(sheet).getByText("More options"));
    fireEvent.change(within(sheet).getByLabelText("Network address"), { target: { value: "192.168.42.17" } });
    fireEvent.click(within(sheet).getByRole("button", { name: "Show a code" }));
    await waitFor(() => expect(syncIpc.startDevicePairing).toHaveBeenCalledWith({
      transport: "local-network", hostOverride: "192.168.42.17", selection: defaultSelection,
    }));
  });

  it("lets the sender include AI services and saved API keys", async () => {
    renderSync();
    const sheet = await openSheet("Send to a device");
    fireEvent.click(toggle(sheet, "AI services"));
    fireEvent.click(toggle(sheet, "Saved API keys"));
    fireEvent.click(within(sheet).getByRole("button", { name: "Show a code" }));
    await waitFor(() => expect(syncIpc.startDevicePairing).toHaveBeenCalledWith({
      transport: "local-network", hostOverride: null, selection: { ...defaultSelection, apiSettings: true, apiCredentials: true },
    }));
  });

  it("keeps study guides dependent on documents", async () => {
    renderSync();
    const sheet = await openSheet("Send to a device");
    fireEvent.click(toggle(sheet, "Documents"));
    expect(toggle(sheet, "Documents").getAttribute("aria-checked")).toBe("false");
    expect(toggle(sheet, "Study guides").getAttribute("aria-checked")).toBe("false");
    fireEvent.click(toggle(sheet, "Study guides"));
    expect(toggle(sheet, "Study guides").getAttribute("aria-checked")).toBe("true");
    expect(toggle(sheet, "Documents").getAttribute("aria-checked")).toBe("true");
  });

  it("sends notes and lessons on their own", async () => {
    renderSync();
    const sheet = await openSheet("Send to a device");
    fireEvent.click(toggle(sheet, "Flashcards"));
    fireEvent.click(toggle(sheet, "Documents"));
    fireEvent.click(toggle(sheet, "Lessons"));
    fireEvent.click(within(sheet).getByRole("button", { name: "Show a code" }));
    await waitFor(() => expect(syncIpc.startDevicePairing).toHaveBeenCalledWith({
      transport: "local-network", hostOverride: null,
      selection: { ...defaultSelection, flashcards: false, documents: false, studyGuides: false, lessons: false },
    }));
  });

  it("keeps saved API keys dependent on AI services", async () => {
    renderSync();
    const sheet = await openSheet("Send to a device");
    fireEvent.click(toggle(sheet, "Saved API keys"));
    expect(toggle(sheet, "Saved API keys").getAttribute("aria-checked")).toBe("true");
    expect(toggle(sheet, "AI services").getAttribute("aria-checked")).toBe("true");
    fireEvent.click(toggle(sheet, "AI services"));
    expect(toggle(sheet, "AI services").getAttribute("aria-checked")).toBe("false");
    expect(toggle(sheet, "Saved API keys").getAttribute("aria-checked")).toBe("false");
  });

  it("requires at least one selected category", async () => {
    renderSync();
    const sheet = await openSheet("Send to a device");
    fireEvent.click(toggle(sheet, "Flashcards"));
    fireEvent.click(toggle(sheet, "Documents"));
    fireEvent.click(toggle(sheet, "Notes"));
    fireEvent.click(toggle(sheet, "Lessons"));
    expect((within(sheet).getByRole("button", { name: "Show a code" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(within(sheet).getByRole("radio", { name: "USB cable" }));
    expect((within(sheet).getByRole("button", { name: "Send over USB" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("disables saved API key transfer when secure storage is unavailable", async () => {
    vi.mocked(syncIpc.getDeviceSyncState).mockResolvedValue({ ...state(), secureCredentialsAvailable: false });
    renderSync();
    const sheet = await openSheet("Send to a device");
    const keys = toggle(sheet, "Saved API keys");
    expect(keys.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(keys);
    expect(keys.getAttribute("aria-checked")).toBe("false");
  });

  it("shows a waiting code as a transfer row the user can cancel", async () => {
    const waiting = activeHostSession();
    vi.mocked(syncIpc.getDeviceSyncState).mockResolvedValueOnce({ ...state(), sessions: [waiting] }).mockResolvedValue(state());
    const showToast = renderSync();
    const transfers = await screen.findByRole("region", { name: "Transfers in progress" });
    expect(within(transfers).getByText(/Sending · Waiting/u)).toBeTruthy();
    fireEvent.click(within(transfers).getByRole("button", { name: "Cancel sending request" }));
    await waitFor(() => expect(syncIpc.cancelDeviceSyncSession).toHaveBeenCalledWith(waiting.id));
    expect(showToast).toHaveBeenCalledWith("info", "Sending request canceled.");
  });

  it("offers a new code in place of a waiting one", async () => {
    vi.mocked(syncIpc.getDeviceSyncState).mockResolvedValue({ ...state(), sessions: [activeHostSession()] });
    renderSync();
    const sheet = await openSheet("Send to a device");
    expect(within(sheet).getByRole("button", { name: "Make a new code" })).toBeTruthy();
  });

  it("labels an unclaimed direct-USB session without exposing a network endpoint", async () => {
    vi.mocked(syncIpc.getDeviceSyncState).mockResolvedValue({ ...state(), sessions: [{ ...activeHostSession(), endpointHint: "usb", state: "connecting" }] });
    renderSync();
    expect(await screen.findByText(/Connecting · USB cable/u)).toBeTruthy();
  });

  it("receives a scanned code immediately without a second action", async () => {
    const showToast = renderSync();
    const sheet = await openSheet("Receive");
    fireEvent.click(within(sheet).getByRole("button", { name: "Scan the code" }));
    await waitFor(() => {
      expect(syncIpc.joinDevicePairing).toHaveBeenCalledTimes(1);
      expect(syncIpc.joinDevicePairing).toHaveBeenCalledWith("ostudy-sync://pair/scanned");
    });
    expect(within(sheet).getByRole("status").textContent).toContain("Sync complete");
    expect(showToast).toHaveBeenCalledWith("info", "Received from Android phone: 12 rows changed and 1 conflicts need review.");
    expect(screen.queryByDisplayValue("ostudy-sync://pair/scanned")).toBeNull();
  });

  it("turns a failed automatic scan into an explicit retry without rescanning", async () => {
    vi.mocked(syncIpc.joinDevicePairing).mockRejectedValueOnce(new Error("The sending device could not be reached at 192.168.1.10:44000: timed out"));
    renderSync();
    const sheet = await openSheet("Receive");
    fireEvent.click(within(sheet).getByRole("button", { name: "Scan the code" }));
    expect(await within(sheet).findByRole("button", { name: "Try again" })).toBeTruthy();
    expect(within(sheet).getByRole("status").textContent).toContain("connection did not complete");
    expect(within(sheet).getByText(/sending device could not be reached/iu)).toBeTruthy();
    expect(syncIpc.joinDevicePairing).toHaveBeenCalledTimes(1);
    fireEvent.click(within(sheet).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(syncIpc.joinDevicePairing).toHaveBeenCalledTimes(2));
  });

  it("reports optional credential-store failure without failing the received library", async () => {
    vi.mocked(syncIpc.joinDevicePairing).mockResolvedValueOnce({
      sessionId: "5".repeat(32), peer: required(state().peers[0], "sync peer"), changedRowCount: 12, conflictCount: 0, transferredCredentialCount: 0,
      credentialWarning: "Selected study data synchronized, but saved API credentials could not be stored securely on this device. Re-enter them in Settings → AI if needed.",
      sentPackageBytes: 0, receivedPackageBytes: 20,
    });
    const showToast = renderSync();
    const sheet = await openSheet("Receive");
    fireEvent.change(within(sheet).getByLabelText("Paste the code as text"), { target: { value: "ostudy-sync://pair/credential-warning" } });
    fireEvent.click(within(sheet).getAllByRole("button", { name: "Receive" }).at(-1)!);
    await waitFor(() => expect(syncIpc.joinDevicePairing).toHaveBeenCalledTimes(1));
    expect(showToast).toHaveBeenCalledWith("info", expect.stringContaining("saved API credentials could not be stored securely"));
  });

  it("receives pasted code text without receiver-side scope controls", async () => {
    renderSync();
    const sheet = await openSheet("Receive");
    expect(within(sheet).queryByRole("switch")).toBeNull();
    fireEvent.change(within(sheet).getByLabelText("Paste the code as text"), { target: { value: "ostudy-sync://pair/pasted" } });
    fireEvent.click(within(sheet).getAllByRole("button", { name: "Receive" }).at(-1)!);
    await waitFor(() => expect(syncIpc.joinDevicePairing).toHaveBeenCalledWith("ostudy-sync://pair/pasted"));
  });

  it("reports a canceled scanner without starting synchronization", async () => {
    vi.mocked(syncIpc.scanDevicePairingQr).mockResolvedValueOnce(null);
    renderSync();
    const sheet = await openSheet("Receive");
    fireEvent.click(within(sheet).getByRole("button", { name: "Scan the code" }));
    expect(await within(sheet).findByText(/Scan canceled/u)).toBeTruthy();
    expect(syncIpc.joinDevicePairing).not.toHaveBeenCalled();
    expect((within(sheet).getByLabelText("Paste the code as text") as HTMLTextAreaElement).value).toBe("");
  });

  it("renames this device from its row", async () => {
    vi.mocked(syncIpc.updateDeviceSyncName).mockResolvedValue({ deviceId: localId, displayName: "Study desktop", publicKeyBase64: "public" });
    renderSync();
    fireEvent.click(await screen.findByRole("button", { name: "Rename" }));
    const dialog = await screen.findByRole("dialog", { name: "Rename this device" });
    fireEvent.change(within(dialog).getByLabelText("Name"), { target: { value: "Study desktop" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(syncIpc.updateDeviceSyncName).toHaveBeenCalledWith("Study desktop"));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Rename this device" })).toBeNull());
  });

  it("keeps concurrent card edits waiting until the learner keeps one side", async () => {
    vi.mocked(syncIpc.getDeviceSyncState).mockResolvedValueOnce(state()).mockResolvedValueOnce({ ...state(), conflicts: [] });
    renderSync();
    expect(await screen.findByText("1 change needs a decision")).toBeTruthy();
    expect(screen.getByText("From Android phone")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Review" }));
    const sheet = await screen.findByRole("dialog", { name: "Changes to decide" });
    expect(within(sheet).getByText(/Notes · front_text/u)).toBeTruthy();
    expect(within(sheet).getByText("This device")).toBeTruthy();
    expect(within(sheet).getByText(/"value": "Local"/u)).toBeTruthy();
    expect(within(sheet).getByText(/"value": "Remote"/u)).toBeTruthy();
    fireEvent.click(required(within(sheet).getAllByRole("button", { name: "Keep this" })[0], "keep local"));
    await waitFor(() => expect(syncIpc.resolveDeviceSyncConflict).toHaveBeenCalledWith({ conflictId: "3".repeat(32), resolution: "keep_left" }));
  });
});
