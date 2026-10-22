import { useEffect, useMemo, useState, type ReactNode } from "react";
import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import {
  cancelDeviceSyncSession, getDeviceSyncState, joinDevicePairing, listenToDeviceSyncProgress,
  removeDeviceSyncPeer, resolveDeviceSyncConflict, scanDevicePairingQr, startDevicePairing,
  updateDeviceSyncName, type DeviceSyncConflict, type DeviceSyncProgress, type DeviceSyncResult,
  type DeviceSyncSelection, type DeviceSyncSession, type DeviceSyncState, type PairingInvitation,
} from "../../ipc/deviceSync";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { Icon } from "../../ui/icons/Icon";
import { Dialog, Sheet } from "../../ui/overlays/Dialog";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { Disclosure, Textarea } from "../../ui/primitives/FormControls";
import { FormField } from "../../ui/primitives/FormField";
import { IconButton } from "../../ui/icons/IconButton";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { ListRow, ListRowMedia, ListRows } from "../../ui/primitives/ListRow";
import { Panel } from "../../ui/primitives/Panel";
import { Progress } from "../../ui/primitives/Progress";
import { SegmentedControl } from "../../ui/primitives/SegmentedControl";
import { TextField } from "../../ui/primitives/TextField";
import { SwitchRow } from "./SettingsControls";
import styles from "./DeviceSyncSettings.module.css";

interface DeviceSyncSettingsProps { showToast: ShowToast; }
type ScanFeedback = "idle" | "scanning" | "detected" | "receiving" | "received" | "cancelled";
type SyncSelectionKey = keyof DeviceSyncSelection;
const ACTIVE_SESSION_STATES = new Set(["waiting", "connecting", "transferring", "merging"]);
const DEFAULT_SELECTION: DeviceSyncSelection = {
  flashcards: true, documents: true, studyGuides: true, notes: true, lessons: true, apiSettings: false, apiCredentials: false,
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
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [cancelingSessionId, setCancelingSessionId] = useState<string | null>(null);
  const [scanFeedback, setScanFeedback] = useState<ScanFeedback>("idle");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [sheet, setSheet] = useState<"send" | "receive" | "conflicts" | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [transport, setTransport] = useState<"network" | "usb">("network");
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
  const hasSelectedCategory = selection.flashcards || selection.documents || selection.notes || selection.lessons || selection.apiSettings;
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
    else if (session.errorCode === "device_sync_rejected") showToast("info", "The receiving Android device rejected the USB sync request.");
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
  async function saveDeviceName(): Promise<boolean> {
    if (busyAction) return false;
    setBusyAction("name");
    setErrorMessage(null);
    try {
      const identity = await updateDeviceSyncName(deviceName.trim());
      setState((current) => current ? { ...current, identity } : current);
      showToast("success", "Device name saved.");
      return true;
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); return false; }
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
    ? `${formatBytes(usbProgress.transferredBytes)} of ${formatBytes(usbProgress.totalBytes)}`
    : "Waiting for the phone to accept…";
  const busy = busyAction !== null;

  return (
    <div className="tw:grid tw:min-w-0 tw:gap-5">
      <Panel className="tw:min-w-0" title="Devices" aria-busy={busyAction === "refresh"}
        actions={<>
          <HelpTrigger topicId="settings.deviceSync" label="Help with device sync" />
          {state ? <>
            <Button variant="secondary" disabled={busy} onClick={() => setSheet("receive")}>Receive</Button>
            <Button variant="primary" disabled={busy} onClick={() => setSheet("send")}>Send to a device</Button>
          </> : null}
        </>}>
        {errorMessage && sheet === null ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
        {!state && busyAction === "refresh" ? <InlineProgress>Opening device sync…</InlineProgress> : null}
        {!state && busyAction === null ? <Button variant="secondary" className="tw:justify-self-start" onClick={() => void refresh()}>Try again</Button> : null}
        {state ? <>
          {activeSessions.length > 0 ? <section aria-label="Transfers in progress" className="tw:grid tw:gap-2">
            {activeSessions.map((session) => {
              const sending = session.direction === "host";
              const usb = session.id === usbSessionId;
              return <div key={session.id} className="tw:flex tw:flex-wrap tw:items-center tw:gap-3 tw:rounded-lg tw:bg-accent tw:px-4 tw:py-3">
                <ListRowMedia icon={session.endpointHint === "usb" ? "usb" : "wifi"} className="tw:bg-background tw:text-primary" />
                <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-1.5">
                  <strong className="tw:break-words tw:font-medium">{sending ? "Sending" : "Receiving"} · {humanizeState(session.state)} · {sessionPeerLabel(session, state)}</strong>
                  {usb ? <Progress value={usbProgress?.transferredBytes ?? null} max={Math.max(1, usbProgress?.totalBytes ?? 1)} label="USB sync progress" valueText={usbProgressText} text={<span>{usbProgressText}</span>} /> : null}
                </div>
                <Button variant="secondary" disabled={cancelingSessionId !== null} aria-label={`Cancel ${sending ? "sending" : "receiving"} request`} onClick={() => void cancelSession(session)}>
                  {cancelingSessionId === session.id ? "Canceling…" : "Cancel"}
                </Button>
              </div>;
            })}
          </section> : null}
          <ListRows aria-label="Devices">
            <ListRow className="tw:items-center" leading={<ListRowMedia icon={state.usbHostAvailable ? "monitor" : "smartphone"} />}
              title={state.identity.displayName}
              meta={<span>This device · <span className="tw:font-mono">{formatFingerprint(state.fingerprint)}</span></span>}
              trailing={<OverflowMenu label="More for this device" items={[{ id: "rename", label: "Rename", disabled: busy, onSelect: () => { setDeviceName(state.identity.displayName); setRenaming(true); } }]} triggerIcon="edit" />} />
            {state.peers.map((peer) => <ListRow key={peer.id} className="tw:items-center" leading={<ListRowMedia icon="smartphone" />}
              title={peer.displayName}
              meta={peer.lastSyncedAt ? `Last synced ${formatDateTime(peer.lastSyncedAt)}` : `Paired ${formatDateTime(peer.pairedAt)}`}
              trailing={<OverflowMenu label={`More for ${peer.displayName}`} items={[{ id: "remove", label: "Remove", destructive: true, disabled: busy, onSelect: () => void removePeer(peer.id, peer.displayName) }]} />} />)}
          </ListRows>
        </> : null}
      </Panel>

      {state && state.conflicts.length > 0 ? <Panel className="tw:min-w-0" aria-label="Changes to decide">
        <ListRows as="div">
          <ListRow as="div" className="tw:items-center" leading={<ListRowMedia icon="warning" className="tw:bg-warning-soft tw:text-warning" />}
            title={`${state.conflicts.length} ${state.conflicts.length === 1 ? "change needs" : "changes need"} a decision`}
            meta={conflictSources(state)}
            trailing={<Button variant="secondary" onClick={() => setSheet("conflicts")}>Review</Button>} />
        </ListRows>
      </Panel> : null}

      {renaming && state ? <Dialog title="Rename this device" width="small" busy={busyAction === "name"} onClose={() => setRenaming(false)}
        onSubmit={() => void saveDeviceName().then((ok) => { if (ok) setRenaming(false); })}
        actions={<>
          <Button variant="secondary" disabled={busyAction === "name"} onClick={() => setRenaming(false)}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={busy || deviceName.trim() === "" || deviceName.trim() === state.identity.displayName}>{busyAction === "name" ? "Saving…" : "Save"}</Button>
        </>}>
        <FormField label="Name"><TextField maxLength={120} value={deviceName} onChange={(event) => setDeviceName(event.currentTarget.value)} /></FormField>
      </Dialog> : null}

      {sheet === "send" && state ? <Sheet title="Send to a device" width="small" onClose={() => setSheet(null)}
        actions={<Button variant="secondary" onClick={() => setSheet(null)}>Done</Button>}>
        <div className="tw:grid tw:min-w-0 tw:gap-5">
          {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
          <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:border-0 tw:p-0">
            <legend className="tw:mb-1 tw:p-0 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">What to send</legend>
            <div className="tw:grid tw:divide-y tw:divide-border/70">
              <SwitchRow label="Flashcards" checked={selection.flashcards} disabled={busy} onChange={(checked) => changeSelection("flashcards", checked)} />
              <SwitchRow label="Documents" checked={selection.documents} disabled={busy} onChange={(checked) => changeSelection("documents", checked)} />
              <SwitchRow label="Study guides" checked={selection.studyGuides} disabled={busy} onChange={(checked) => changeSelection("studyGuides", checked)} />
              <SwitchRow label="Notes" checked={selection.notes} disabled={busy} onChange={(checked) => changeSelection("notes", checked)} />
              <SwitchRow label="Lessons" checked={selection.lessons} disabled={busy} onChange={(checked) => changeSelection("lessons", checked)} />
              <SwitchRow label="AI services" checked={selection.apiSettings} disabled={busy} onChange={(checked) => changeSelection("apiSettings", checked)} />
              <SwitchRow label="Saved API keys" checked={selection.apiCredentials} disabled={busy || !credentialsAvailable} onChange={(checked) => changeSelection("apiCredentials", checked)} />
            </div>
          </fieldset>
          {state.usbHostAvailable ? <SegmentedControl label="How to send" value={transport}
            options={[{ value: "network", label: "Network" }, { value: "usb", label: "USB cable" }]}
            onChange={(next) => { setTransport(next); setErrorMessage(null); }} /> : null}
          {transport === "usb" && state.usbHostAvailable ? <div className="tw:grid tw:gap-3">
            <Button variant="primary" disabled={busy || !hasSelectedCategory || usbSessionId !== null} onClick={() => void startUsbSync()}>
              {busyAction === "usb" ? "Connecting…" : "Send over USB"}
            </Button>
            {usbSessionId ? <Progress value={usbProgress?.transferredBytes ?? null} max={Math.max(1, usbProgress?.totalBytes ?? 1)} label="USB sync progress" valueText={usbProgressText} text={<span>{usbProgressText}</span>} /> : null}
          </div> : <div className="tw:grid tw:gap-3">
            {invitation && qrSource ? <div className="tw:grid tw:justify-items-center tw:gap-3 tw:rounded-lg tw:bg-muted tw:p-4">
              <div className={styles.qrFrame}><img src={qrSource} alt="Sync code for the other device to scan" /></div>
              <span className="tw:text-label tw:text-muted-foreground">Expires {formatTime(invitation.expiresAt)} · <span className="tw:font-mono">{invitation.endpoint}</span></span>
              <Button variant="text" onClick={() => void copyInvitation()}>Copy as text</Button>
            </div> : null}
            <Button variant={invitation ? "secondary" : "primary"} disabled={busy || !state.localNetworkAvailable || !hasSelectedCategory} onClick={() => void hostPairing()}>
              {busyAction === "host" ? "Opening…" : invitation || hasUnclaimedSendingOffer ? "Make a new code" : "Show a code"}
            </Button>
            <Disclosure title="More options">
              <div className="tw:px-4 tw:pb-3">
                <FormField label="Network address"><TextField inputMode="decimal" placeholder="Found automatically" value={hostOverride} onChange={(event) => setHostOverride(event.currentTarget.value)} /></FormField>
              </div>
            </Disclosure>
          </div>}
        </div>
      </Sheet> : null}

      {sheet === "receive" && state ? <Sheet title="Receive" width="small" busy={busyAction === "receive"} onClose={() => setSheet(null)}
        actions={<Button variant="secondary" disabled={busyAction === "receive"} onClick={() => setSheet(null)}>Close</Button>}>
        <div className="tw:grid tw:min-w-0 tw:gap-4">
          {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
          {state.qrScanAvailable ? <>
            <Button variant="primary" disabled={busy} onClick={() => void scanPairingCode()}>
              <Icon name="scan" size={18} />{busyAction === "scan" ? "Scanning…" : busyAction === "receive" ? "Receiving…" : "Scan the code"}
            </Button>
            {scanFeedback !== "idle" ? <div role="status" aria-live="polite" data-state={scanFeedback}
              className="tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted tw:p-4 tw:text-label tw:leading-relaxed tw:data-[state=receiving]:border-primary tw:data-[state=receiving]:bg-accent tw:data-[state=received]:border-success tw:data-[state=received]:bg-success-soft tw:data-[state=detected]:border-warning tw:data-[state=detected]:bg-warning-soft">
              {scanFeedback === "scanning" ? "Looking for a sync code…"
                : scanFeedback === "detected" ? "Found the code, but the connection did not complete. Try again while the code is still valid."
                  : scanFeedback === "receiving" ? "Receiving…"
                    : scanFeedback === "received" ? "Received. Sync complete."
                      : "Scan canceled. Nothing was received."}
            </div> : null}
            <div aria-hidden="true" className="tw:flex tw:items-center tw:gap-3 tw:text-label tw:text-muted-foreground"><span className="tw:h-px tw:flex-1 tw:bg-border" />or<span className="tw:h-px tw:flex-1 tw:bg-border" /></div>
          </> : null}
          <FormField label="Paste the code as text"><Textarea rows={4} maxLength={8_192} placeholder="ostudy-sync://pair/…" value={joinInvitation}
            onChange={(event) => { setJoinInvitation(event.currentTarget.value); if (["detected", "received"].includes(scanFeedback)) setScanFeedback("idle"); }} /></FormField>
          <Button variant={state.qrScanAvailable ? "secondary" : "primary"} disabled={busy || !joinInvitation.trim() || !state.localNetworkAvailable} onClick={() => void joinPairing()}>
            {busyAction === "receive" ? "Receiving…" : scanFeedback === "detected" && joinInvitation.trim() ? "Try again" : "Receive"}
          </Button>
        </div>
      </Sheet> : null}

      {sheet === "conflicts" && state ? <ConflictSheet state={state} disabled={busy}
        onResolve={(conflict, resolution) => void resolveConflict(conflict, resolution)}
        onClose={() => setSheet(null)} /> : null}
    </div>
  );
}

function ConflictSheet({ state, disabled, onResolve, onClose }: {
  state: DeviceSyncState;
  disabled: boolean;
  onResolve: (conflict: DeviceSyncConflict, resolution: "keep_left" | "keep_right" | "delete") => void;
  onClose: () => void;
}) {
  const [index, setIndex] = useState(0);
  const count = state.conflicts.length;
  const current = state.conflicts[Math.min(index, count - 1)];
  useEffect(() => { if (count === 0) onClose(); }, [count, onClose]);
  if (!current) return null;
  const position = Math.min(index, count - 1);
  const sideLabel = (deviceId: string, fallback: string) => deviceId === state.identity.deviceId
    ? "This device"
    : state.peers.find((peer) => peer.id === deviceId)?.displayName ?? fallback;
  return (
    <Sheet title="Changes to decide" width="medium" busy={disabled} onClose={onClose}
      actions={<Button variant="secondary" onClick={onClose}>Close</Button>}>
      <div className="tw:grid tw:min-w-0 tw:gap-4">
        <div className="tw:flex tw:items-center tw:gap-2">
          <span className="tw:min-w-0 tw:flex-1 tw:text-label tw:text-muted-foreground">
            {position + 1} of {count} · {humanizeTable(current.tableName)} · {current.conflictingFields.join(", ") || "Whole record"}
          </span>
          <IconButton icon="chevron-left" label="Previous change" disabled={position === 0} onClick={() => setIndex(position - 1)} />
          <IconButton icon="chevron-right" label="Next change" disabled={position >= count - 1} onClick={() => setIndex(position + 1)} />
        </div>
        <div className="tw:grid tw:gap-3 tw:md:grid-cols-2">
          <ConflictVersion label={sideLabel(current.leftDeviceId, "One device")} payload={current.leftPayloadJson}
            action={<Button variant="secondary" disabled={disabled} onClick={() => onResolve(current, "keep_left")}>Keep this</Button>} />
          <ConflictVersion label={sideLabel(current.rightDeviceId, "Other device")} payload={current.rightPayloadJson}
            action={<Button variant="secondary" disabled={disabled} onClick={() => onResolve(current, "keep_right")}>Keep this</Button>} />
        </div>
        <Button variant="dangerGhost" className="tw:justify-self-start" disabled={disabled} onClick={() => onResolve(current, "delete")}>Delete this record</Button>
      </div>
    </Sheet>
  );
}

function ConflictVersion({ label, payload, action }: { label: string; payload: string | null; action: ReactNode }) {
  return <div className="tw:flex tw:min-w-0 tw:flex-col tw:gap-2 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-3"><strong>{label}</strong>
    {payload ? <pre className="tw:m-0 tw:max-h-72 tw:overflow-auto tw:whitespace-pre-wrap tw:break-all tw:text-caption">{prettyPayload(payload)}</pre> : <p className="tw:m-0 tw:text-destructive">Deleted on this device</p>}
    <div className="tw:mt-auto tw:grid">{action}</div>
  </div>;
}
function prettyPayload(payload: string): string {
  try { return JSON.stringify(JSON.parse(payload), null, 2); } catch { return payload; }
}
function formatFingerprint(value: string): string { return value.match(/.{1,4}/g)?.slice(0, 8).join(" ") ?? value; }
function formatDateTime(value: number): string {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}
function formatTime(value: number): string {
  return new Intl.DateTimeFormat(undefined, { timeStyle: "short" }).format(new Date(value));
}
/** Where the open conflicts came from, by paired device name. */
function conflictSources(state: DeviceSyncState): string {
  const names = [...new Set(state.conflicts.map((conflict) => state.peers.find((peer) => peer.id === conflict.peerId)?.displayName ?? "another device"))];
  return `From ${names.join(", ")}`;
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
