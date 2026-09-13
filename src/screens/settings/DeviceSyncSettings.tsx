import { useEffect, useMemo, useState } from "react";
import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import {
  cancelDeviceSyncSession, getDeviceSyncState, joinDevicePairing, listenToDeviceSyncProgress,
  removeDeviceSyncPeer, resolveDeviceSyncConflict, scanDevicePairingQr, startDevicePairing,
  updateDeviceSyncName, type DeviceSyncConflict, type DeviceSyncProgress, type DeviceSyncResult,
  type DeviceSyncSelection, type DeviceSyncSession, type DeviceSyncState, type PairingInvitation,
} from "../../ipc/deviceSync";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Dialog, DialogActions } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { Checkbox, Textarea } from "../../ui/primitives/FormControls";
import { FormField } from "../../ui/primitives/FormField";
import { TextField } from "../../ui/primitives/TextField";
import { Panel } from "../../ui/primitives/Panel";
import { Badge } from "../../ui/primitives/Badge";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { Progress } from "../../ui/primitives/Progress";
import styles from "./DeviceSyncSettings.module.css";

interface DeviceSyncSettingsProps { showToast: ShowToast; }
type ScanFeedback = "idle" | "scanning" | "detected" | "receiving" | "received" | "cancelled";
type SyncSelectionKey = keyof DeviceSyncSelection;
const ACTIVE_SESSION_STATES = new Set(["waiting", "connecting", "transferring", "merging"]);
const DEFAULT_SELECTION: DeviceSyncSelection = {
  flashcards: true, documents: true, studyGuides: true, apiSettings: false, apiCredentials: false,
};

export function DeviceSyncSettings({ showToast }: DeviceSyncSettingsProps) {
  const [state, setState] = useState<DeviceSyncState | null>(null);
  const [deviceName, setDeviceName] = useState("");
  const [hostOverride, setHostOverride] = useState("");
  const [joinInvitation, setJoinInvitation] = useState("");
  const [selection, setSelection] = useState<DeviceSyncSelection>(DEFAULT_SELECTION);
  const [invitation, setInvitation] = useState<PairingInvitation | null>(null);
  const [usbSessionId, setUsbSessionId] = useState<string | null>(null);
  const [usbProgress, setUsbProgress] = useState<DeviceSyncProgress | null>(null);
  const [usbRejected, setUsbRejected] = useState(false);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [cancelingSessionId, setCancelingSessionId] = useState<string | null>(null);
  const [scanFeedback, setScanFeedback] = useState<ScanFeedback>("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const confirm = useConfirm();

  async function refresh(silent = false): Promise<void> {
    if (!silent) setBusyAction("refresh");
    try {
      const next = await getDeviceSyncState();
      setState(next);
      setDeviceName((current) => current || next.identity.displayName);
      setErrorMessage(null);
    } catch (error: unknown) {
      if (!silent) setErrorMessage(getErrorMessage(error));
    } finally { if (!silent) setBusyAction(null); }
  }
  useEffect(() => { void refresh(); }, []);

  const credentialsAvailable = state?.secureCredentialsAvailable === true;
  const activeSessions = useMemo(
    () => state?.sessions.filter((session) => ACTIVE_SESSION_STATES.has(session.state)) ?? [], [state],
  );
  const hasUnclaimedSendingOffer = activeSessions.some((session) => session.direction === "host" && session.peerId === null);
  const hasSelectedCategory = selection.flashcards || selection.documents || selection.apiSettings;
  useEffect(() => {
    if (!state || credentialsAvailable) return;
    setSelection((current) => current.apiCredentials ? { ...current, apiCredentials: false } : current);
  }, [credentialsAvailable, state]);
  useEffect(() => {
    const invitationWaiting = invitation !== null && invitation.expiresAt > Date.now();
    if (!invitationWaiting && busyAction !== "receive" && activeSessions.length === 0) return;
    const interval = window.setInterval(() => void refresh(true), 1_500);
    return () => window.clearInterval(interval);
  }, [activeSessions.length, busyAction, invitation]);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenToDeviceSyncProgress((next) => {
      if (!disposed && next.direction === "send" && next.sessionId === usbSessionId) setUsbProgress(next);
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [usbSessionId]);
  useEffect(() => {
    if (!usbSessionId || !state) return;
    const session = state.sessions.find((candidate) => candidate.id === usbSessionId);
    if (!session || ACTIVE_SESSION_STATES.has(session.state)) return;
    if (session.state === "completed") showToast("success", "USB synchronization completed.");
    else if (session.state === "canceled") showToast("info", "USB synchronization was canceled.");
    else if (session.errorCode === "device_sync_rejected") setUsbRejected(true);
    else showToast("error", session.errorMessage ?? "USB synchronization did not complete. Reconnect the phone and try again.");
    setUsbSessionId(null);
    setUsbProgress(null);
  }, [showToast, state, usbSessionId]);

  function changeSelection(key: SyncSelectionKey, checked: boolean): void {
    setSelection((current) => {
      const next = { ...current, [key]: checked };
      if (key === "documents" && !checked) next.studyGuides = false;
      if (key === "studyGuides" && checked) next.documents = true;
      if (key === "apiSettings" && !checked) next.apiCredentials = false;
      if (key === "apiCredentials" && checked) next.apiSettings = true;
      return next;
    });
    setInvitation(null);
    setErrorMessage(null);
  }
  async function saveDeviceName(): Promise<void> {
    if (busyAction) return;
    setBusyAction("name");
    setErrorMessage(null);
    try {
      const identity = await updateDeviceSyncName(deviceName);
      setState((current) => current ? { ...current, identity } : current);
      showToast("success", "Device name saved.");
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setBusyAction(null); }
  }
  async function hostPairing(): Promise<void> {
    if (busyAction) return;
    if (!hasSelectedCategory) {
      setErrorMessage("Select at least one synchronization category before creating an offer.");
      return;
    }
    setBusyAction("host");
    setErrorMessage(null);
    try {
      const created = await startDevicePairing({ transport: "local-network", hostOverride: hostOverride.trim() || null, selection });
      setInvitation(created);
      await refresh(true);
      showToast("info", hasUnclaimedSendingOffer
        ? "The previous waiting offer was canceled and replaced with a new five-minute network offer."
        : "Network sync offer is ready for five minutes.");
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setBusyAction(null); }
  }
  async function startUsbSync(): Promise<void> {
    if (busyAction) return;
    if (!hasSelectedCategory) {
      setErrorMessage("Select at least one synchronization category before starting USB sync.");
      return;
    }
    setBusyAction("usb");
    setInvitation(null);
    setUsbProgress(null);
    setErrorMessage(null);
    try {
      const created = await startDevicePairing({ transport: "usb", selection });
      setUsbSessionId(created.sessionId);
      await refresh(true);
      showToast("info", "USB sync request sent. Waiting for the Android device to accept it.");
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setBusyAction(null); }
  }
  function showReceiveResult(result: DeviceSyncResult): void {
    const credentialCopy = result.transferredCredentialCount > 0
      ? ` ${result.transferredCredentialCount} saved API credential${result.transferredCredentialCount === 1 ? " was" : "s were"} transferred.` : "";
    const credentialWarning = result.credentialWarning ? ` ${result.credentialWarning}` : "";
    showToast(result.conflictCount > 0 || result.credentialWarning ? "info" : "success",
      `Received from ${result.peer.displayName}: ${result.changedRowCount} rows changed${result.conflictCount > 0 ? ` and ${result.conflictCount} conflicts need review` : ""}.${credentialCopy}${credentialWarning}`);
  }
  async function receiveOffer(invitationText: string, fromScanner: boolean): Promise<void> {
    const trimmed = invitationText.trim();
    if (!trimmed) return;
    setBusyAction("receive");
    setErrorMessage(null);
    if (fromScanner) setScanFeedback("receiving");
    try {
      const result = await joinDevicePairing(trimmed);
      setJoinInvitation("");
      setInvitation(null);
      if (fromScanner) setScanFeedback("received");
      await refresh(true);
      showReceiveResult(result);
    } catch (error: unknown) {
      if (fromScanner) setScanFeedback("detected");
      await refresh(true);
      setErrorMessage(getErrorMessage(error));
    } finally { setBusyAction(null); }
  }
  async function scanPairingCode(): Promise<void> {
    if (busyAction) return;
    setBusyAction("scan");
    setScanFeedback("scanning");
    setErrorMessage(null);
    try {
      const scanned = await scanDevicePairingQr();
      if (!scanned) {
        setScanFeedback("cancelled");
        setBusyAction(null);
        return;
      }
      setJoinInvitation(scanned);
      setScanFeedback("detected");
      showToast("success", "Sync offer detected. Accepting it now…");
      await receiveOffer(scanned, true);
    } catch (error: unknown) {
      setScanFeedback("idle");
      setErrorMessage(getErrorMessage(error));
      setBusyAction(null);
    }
  }
  async function joinPairing(): Promise<void> {
    if (busyAction || !joinInvitation.trim()) return;
    await receiveOffer(joinInvitation, false);
  }
  async function cancelSession(session: DeviceSyncSession): Promise<void> {
    if (cancelingSessionId) return;
    setCancelingSessionId(session.id);
    setErrorMessage(null);
    try {
      await cancelDeviceSyncSession(session.id);
      if (invitation?.sessionId === session.id) setInvitation(null);
      if (usbSessionId === session.id) setUsbSessionId(null);
      await refresh(true);
      showToast("info", `${session.direction === "host" ? "Sending" : "Receiving"} request canceled.`);
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setCancelingSessionId(null); }
  }
  async function copyInvitation(): Promise<void> {
    if (!invitation) return;
    try {
      await navigator.clipboard.writeText(invitation.invitation);
      showToast("success", "Synchronization invitation copied.");
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
  }
  async function removePeer(peerId: string, name: string): Promise<void> {
    const accepted = await confirm({
      title: `Remove ${name}?`,
      description: "This device will no longer be trusted. Create or accept a new synchronization offer before transferring data again.",
      confirmLabel: "Remove device", destructive: true,
    });
    if (!accepted) return;
    setBusyAction(`remove:${peerId}`);
    try {
      await removeDeviceSyncPeer(peerId);
      await refresh(true);
      showToast("success", `${name} was removed.`);
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setBusyAction(null); }
  }
  async function resolveConflict(conflict: DeviceSyncConflict, resolution: "keep_left" | "keep_right" | "delete"): Promise<void> {
    if (resolution === "delete") {
      const accepted = await confirm({
        title: "Delete this synchronized record?",
        description: "The record will be removed from this library. A future offer from this device can carry that decision to another device.",
        confirmLabel: "Delete record", destructive: true,
      });
      if (!accepted) return;
    }
    setBusyAction(`conflict:${conflict.id}`);
    setErrorMessage(null);
    try {
      await resolveDeviceSyncConflict({ conflictId: conflict.id, resolution });
      await refresh(true);
      showToast("success", "Synchronization conflict resolved.");
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setBusyAction(null); }
  }
  const qrSource = invitation?.qrSvg ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(invitation.qrSvg)}` : null;
  const usbProgressText = usbProgress
    ? `Sending ${formatBytes(usbProgress.transferredBytes)} of ${formatBytes(usbProgress.totalBytes)}`
    : "Waiting for the Android device to accept the request…";

  return (
    <Panel title="Device synchronization" description="Send selected Flashcards, Documents and their LLM processing data, Study Guides, and optional API configuration over your private local network or directly through a USB cable. Appearance and other app settings always stay local to each device."
      actions={<Button variant="text" disabled={busyAction !== null} onClick={() => void refresh()}>Refresh</Button>}>
      {usbRejected ? <Dialog title="USB sync rejected" description="The receiving Android device rejected the USB sync request." role="alertdialog" dismissible={false} onClose={() => setUsbRejected(false)} width="small">
        <DialogActions><Button variant="primary" onClick={() => setUsbRejected(false)}>OK</Button></DialogActions>
      </Dialog> : null}
      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      {!state && busyAction === "refresh" ? <InlineProgress>Opening the device synchronization store…</InlineProgress> : null}
      {state ? <>
        <div className="tw:grid tw:items-end tw:gap-4 tw:lg:grid-cols-3">
          <FormField label="This device's name"><TextField maxLength={120} value={deviceName} onChange={(event) => setDeviceName(event.currentTarget.value)} /></FormField>
          <div className="tw:grid tw:min-w-0 tw:gap-2"><span className="tw:text-label tw:font-medium">Identity fingerprint</span>
            <code className="tw:overflow-auto tw:rounded-md tw:bg-muted tw:p-3 tw:text-caption">{formatFingerprint(state.fingerprint)}</code></div>
          <Button disabled={busyAction !== null || deviceName.trim() === state.identity.displayName} onClick={() => void saveDeviceName()}>{busyAction === "name" ? "Saving…" : "Save device name"}</Button>
        </div>
        <div className="tw:grid tw:items-start tw:gap-6 tw:xl:grid-cols-2">
          <Panel as="article" title="Send to another device" description="Choose exactly which categories this one-way transfer may contain. The receiving device never uploads its own library in response.">
            <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-4">
              <legend className="tw:px-2 tw:text-label tw:font-semibold">What to synchronize</legend>
              <SyncChoice title="Flashcards" description="Decks, folders, cards, templates, tags, schedules, review history, and managed card images."
                checked={selection.flashcards} disabled={busyAction !== null} onCheckedChange={(checked) => changeSelection("flashcards", checked)} />
              <SyncChoice title="Documents" description="Source files, canonical content, OCR/semantic data, outlines, LLM processing jobs, checkpoints, and learning-plan data."
                checked={selection.documents} disabled={busyAction !== null} onCheckedChange={(checked) => changeSelection("documents", checked)} />
              <SyncChoice title="Study Guides" description="Generated guide revisions, sections, learner overlays, and guide checkpoints. Requires Documents."
                checked={selection.studyGuides} disabled={busyAction !== null} onCheckedChange={(checked) => changeSelection("studyGuides", checked)} />
              <SyncChoice title="API provider settings" description="Provider profiles and task defaults. Theme, layout, fonts, and other application preferences always stay local."
                checked={selection.apiSettings} disabled={busyAction !== null} onCheckedChange={(checked) => changeSelection("apiSettings", checked)} />
              <SyncChoice title="Saved API keys" description="Securely stored provider credentials. Requires API provider settings and usable secure storage on both devices."
                checked={selection.apiCredentials} disabled={busyAction !== null || !credentialsAvailable} onCheckedChange={(checked) => changeSelection("apiCredentials", checked)} />
            </fieldset>
            {!credentialsAvailable ? <InlineNotice kind="warning">This device's secure secret store is unavailable, so saved API keys cannot be added to the offer. Provider settings can still be synchronized without keys.</InlineNotice> : null}
            {state.usbHostAvailable ? <>
              <p className="tw:m-0 tw:text-label tw:leading-relaxed tw:text-muted-foreground"><strong>USB cable:</strong> plug an Android phone into this computer with a data-capable cable, then click USB Sync. Android opens ostudy for the accessory, and ostudy asks the receiving user to Accept or Reject before any selected study data is transferred. No tethering, IP address, QR code, developer mode, or USB debugging setup is required.</p>
              <Button variant="primary" disabled={busyAction !== null || !hasSelectedCategory || usbSessionId !== null} onClick={() => void startUsbSync()}>{busyAction === "usb" ? "Connecting phone…" : "USB Sync"}</Button>
              {usbSessionId ? <Progress value={usbProgress?.transferredBytes ?? null} max={Math.max(1, usbProgress?.totalBytes ?? 1)} label="USB synchronization progress" valueText={usbProgressText} text={<span>{usbProgressText}</span>} /> : null}
            </> : null}
            <FormField label="Advanced LAN address override" description="Only needed if automatic private LAN-address detection fails. This field is never used for USB synchronization.">
              <TextField inputMode="decimal" placeholder="Usually detected automatically" value={hostOverride} onChange={(event) => setHostOverride(event.currentTarget.value)} />
            </FormField>
            <Button disabled={busyAction !== null || !state.localNetworkAvailable || !hasSelectedCategory} onClick={() => void hostPairing()}>
              {busyAction === "host" ? "Opening listener…" : hasUnclaimedSendingOffer ? "Replace with network offer" : "Create network sync offer"}
            </Button>
            {invitation && qrSource ? <div className="tw:grid tw:justify-items-center tw:gap-4 tw:rounded-xl tw:border tw:border-solid tw:border-primary tw:bg-accent tw:p-4">
              <div className={styles.qrFrame}><img src={qrSource} alt="QR code containing the short-lived ostudy synchronization offer" /></div>
              <div className="tw:grid tw:w-full tw:min-w-0 tw:gap-2 tw:break-words tw:text-label">
                <strong>Expires {formatDateTime(invitation.expiresAt)}</strong><span>Local network · {invitation.endpoint}</span><code>{formatFingerprint(invitation.fingerprint)}</code>
                <p className="tw:m-0 tw:text-caption">Includes: {describeSelection(invitation.selection)}.</p>
                {invitation.selection.apiCredentials && !invitation.credentialsAvailable ? <p className="tw:m-0 tw:text-caption">Saved API keys were selected, but this device has no securely stored credential bytes available for this offer.</p> : null}
                <p className="tw:m-0 tw:text-caption">Keep the entire white border visible. Scanning this code accepts the offer and starts receiving immediately.</p>
              </div>
              <Button className="tw:w-full" onClick={() => void copyInvitation()}>Copy invitation text</Button>
            </div> : null}
          </Panel>
          <Panel as="article" title="Receive from another device" description="Network offers can be scanned on Android or pasted below. Direct USB transfers are presented as an explicit Accept or Reject request on Android. Accepting a transfer never uploads this device's library back.">
            {state.qrScanAvailable ? <>
              <p className="tw:m-0 tw:text-label tw:text-muted-foreground">Center the complete QR code inside the camera frame and hold the phone steady. After detection, ostudy accepts the network offer and begins receiving automatically.</p>
              <Button disabled={busyAction !== null} onClick={() => void scanPairingCode()}>{busyAction === "scan" ? "Scanning…" : busyAction === "receive" ? "Receiving…" : "Scan and receive"}</Button>
              {scanFeedback !== "idle" ? <div role="status" aria-live="polite" data-state={scanFeedback}
                className="tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted tw:p-4 tw:text-label tw:leading-relaxed tw:data-[state=receiving]:border-primary tw:data-[state=receiving]:bg-accent tw:data-[state=received]:border-success tw:data-[state=received]:bg-success-soft tw:data-[state=detected]:border-warning tw:data-[state=detected]:bg-warning-soft">
                {scanFeedback === "scanning" ? "Camera open — looking for an ostudy sync offer…"
                  : scanFeedback === "detected" ? "Sync offer detected, but the connection did not complete. Review the error above, then retry while this offer is still valid."
                    : scanFeedback === "receiving" ? "Sync offer detected — receiving from the sending device…"
                      : scanFeedback === "received" ? "Sync offer accepted — synchronization complete."
                        : "Scan cancelled. No synchronization was started."}
              </div> : null}
            </> : null}
            <FormField label="Synchronization invitation"><Textarea rows={5} maxLength={8_192} placeholder="ostudy-sync://pair/…" value={joinInvitation}
              onChange={(event) => { setJoinInvitation(event.currentTarget.value); if (["detected", "received"].includes(scanFeedback)) setScanFeedback("idle"); }} /></FormField>
            <Button variant="primary" disabled={busyAction !== null || !joinInvitation.trim() || !state.localNetworkAvailable} onClick={() => void joinPairing()}>
              {busyAction === "receive" ? "Receiving…" : scanFeedback === "detected" && joinInvitation.trim() ? "Retry connection" : "Accept sync offer"}
            </Button>
          </Panel>
        </div>
        {activeSessions.length > 0 ? <section aria-label="Active synchronization sessions" className="tw:grid tw:gap-3">
          {activeSessions.map((session) => <div key={session.id} className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-3 tw:rounded-lg tw:bg-accent tw:p-4">
            <span className="tw:flex tw:min-w-0 tw:flex-wrap tw:gap-2 tw:break-words tw:text-label"><strong>{session.direction === "host" ? "Sending" : "Receiving"}</strong>{humanizeState(session.state)} · {sessionPeerLabel(session, state)}</span>
            <Button variant="text" disabled={cancelingSessionId !== null} aria-label={`Cancel ${session.direction === "host" ? "sending" : "receiving"} request`} onClick={() => void cancelSession(session)}>{cancelingSessionId === session.id ? "Canceling…" : "Cancel"}</Button>
          </div>)}
        </section> : null}
        <Panel title="Trusted devices" actions={<Badge>{state.peers.length}</Badge>}>
          {state.peers.length === 0 ? <p className="tw:m-0 tw:text-label tw:text-muted-foreground">No devices have been paired yet.</p> : state.peers.map((peer) =>
            <article key={peer.id} className="tw:flex tw:flex-wrap tw:items-center tw:justify-between tw:gap-4 tw:rounded-lg tw:bg-muted tw:p-4">
              <div className="tw:grid tw:min-w-0 tw:gap-1 tw:break-words"><strong className="tw:font-medium">{peer.displayName}</strong><span className="tw:text-caption tw:text-muted-foreground">{formatFingerprint(peer.fingerprint)}</span><small className="tw:text-muted-foreground">{peer.lastSyncedAt ? `Last synchronized ${formatDateTime(peer.lastSyncedAt)}` : `Paired ${formatDateTime(peer.pairedAt)}`}</small></div>
              <Button variant="dangerGhost" disabled={busyAction !== null} onClick={() => void removePeer(peer.id, peer.displayName)}>{busyAction === `remove:${peer.id}` ? "Removing…" : "Remove"}</Button>
            </article>)}
        </Panel>
        <Panel title="Conflicts requiring a decision" actions={<Badge tone={state.conflicts.length ? "warning" : "neutral"}>{state.conflicts.length}</Badge>}>
          {state.conflicts.length === 0 ? <p className="tw:m-0 tw:text-label tw:text-muted-foreground">No unresolved synchronization conflicts.</p> : state.conflicts.map((conflict) =>
            <article key={conflict.id} className="tw:grid tw:min-w-0 tw:gap-4 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-4">
              <div className="tw:flex tw:flex-wrap tw:items-start tw:justify-between tw:gap-3"><div className="tw:grid tw:min-w-0 tw:gap-1"><strong>{humanizeTable(conflict.tableName)}</strong><span className="tw:text-caption tw:text-muted-foreground">{conflict.conflictingFields.join(", ") || "Whole record"}</span></div><small>{formatDateTime(conflict.createdAt)}</small></div>
              <div className="tw:grid tw:gap-3 tw:lg:grid-cols-2"><ConflictVersion label={conflict.leftDeviceId === state.identity.deviceId ? "This device" : "Left device"} payload={conflict.leftPayloadJson} /><ConflictVersion label={conflict.rightDeviceId === state.identity.deviceId ? "This device" : "Paired device"} payload={conflict.rightPayloadJson} /></div>
              <div className="tw:flex tw:flex-wrap tw:gap-3"><Button disabled={busyAction !== null} onClick={() => void resolveConflict(conflict, "keep_left")}>Keep left</Button><Button disabled={busyAction !== null} onClick={() => void resolveConflict(conflict, "keep_right")}>Keep right</Button><Button variant="dangerGhost" disabled={busyAction !== null} onClick={() => void resolveConflict(conflict, "delete")}>Delete record</Button></div>
            </article>)}
        </Panel>
      </> : null}
    </Panel>
  );
}

function SyncChoice({ title, description, checked, disabled, onCheckedChange }: {
  title: string; description: string; checked: boolean; disabled: boolean; onCheckedChange(checked: boolean): void;
}) {
  return <label className="tw:flex tw:min-h-11 tw:items-start tw:gap-3 tw:rounded-lg tw:p-2 tw:hover:bg-muted">
    <Checkbox checked={checked} disabled={disabled} onCheckedChange={onCheckedChange} />
    <span className="tw:grid tw:min-w-0 tw:gap-1"><strong className="tw:text-label tw:font-medium">{title}</strong><small className="tw:leading-relaxed tw:text-muted-foreground">{description}</small></span>
  </label>;
}
function ConflictVersion({ label, payload }: { label: string; payload: string | null }) {
  return <div className="tw:grid tw:min-w-0 tw:gap-2 tw:rounded-lg tw:bg-muted tw:p-3"><strong>{label}</strong>
    {payload ? <pre className="tw:m-0 tw:max-h-56 tw:overflow-auto tw:whitespace-pre-wrap tw:break-all tw:text-caption">{prettyPayload(payload)}</pre> : <p className="tw:m-0 tw:text-destructive">Deleted on this device</p>}
  </div>;
}
function prettyPayload(payload: string): string {
  try { return JSON.stringify(JSON.parse(payload), null, 2); } catch { return payload; }
}
function describeSelection(selection: DeviceSyncSelection): string {
  const items: string[] = [];
  if (selection.flashcards) items.push("Flashcards and review history");
  if (selection.documents) items.push("Documents and LLM processing data");
  if (selection.studyGuides) items.push("Study Guides");
  if (selection.apiSettings) items.push("API provider settings");
  if (selection.apiCredentials) items.push("saved API keys");
  return items.join(", ");
}
function formatFingerprint(value: string): string { return value.match(/.{1,4}/g)?.slice(0, 8).join(" ") ?? value; }
function formatDateTime(value: number): string {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}
function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let amount = value;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) { amount /= 1024; unit += 1; }
  return `${amount >= 10 || unit === 0 ? amount.toFixed(0) : amount.toFixed(1)} ${units[unit]}`;
}
function humanizeState(value: string): string { return value.replaceAll("_", " ").replace(/^./, (character) => character.toUpperCase()); }
function sessionPeerLabel(session: DeviceSyncSession, state: DeviceSyncState): string {
  if (!session.peerId) return session.endpointHint === "usb" ? "USB cable" : session.endpointHint;
  return state.peers.find((peer) => peer.id === session.peerId)?.displayName ?? session.endpointHint;
}
function humanizeTable(value: string): string { return value.replaceAll("_", " ").replace(/\b\w/g, (character) => character.toUpperCase()); }
