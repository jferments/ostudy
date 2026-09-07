import { useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import {
  cancelDeviceSyncSession,
  getDeviceSyncState,
  joinDevicePairing,
  removeDeviceSyncPeer,
  resolveDeviceSyncConflict,
  scanDevicePairingQr,
  startDevicePairing,
  updateDeviceSyncName,
  type DeviceSyncConflict,
  type DeviceSyncResult,
  type DeviceSyncSelection,
  type DeviceSyncSession,
  type DeviceSyncState,
  type PairingInvitation,
} from "../../ipc/deviceSync";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";
import styles from "./DeviceSyncSettings.module.css";
import settingsStyles from "./Settings.module.css";

const ownedStyleMaps = sharedClassMaps;

interface DeviceSyncSettingsProps {
  showToast: ShowToast;
}

type ScanFeedback = "idle" | "scanning" | "detected" | "receiving" | "received" | "cancelled";
type SyncSelectionKey = keyof DeviceSyncSelection;

const ACTIVE_SESSION_STATES = new Set(["waiting", "connecting", "transferring", "merging"]);
const DEFAULT_SELECTION: DeviceSyncSelection = {
  flashcards: true,
  documents: true,
  studyGuides: true,
  apiSettings: false,
  apiCredentials: false,
};

export function DeviceSyncSettings({ showToast }: DeviceSyncSettingsProps) {
  const [state, setState] = useState<DeviceSyncState | null>(null);
  const [deviceName, setDeviceName] = useState("");
  const [hostOverride, setHostOverride] = useState("");
  const [joinInvitation, setJoinInvitation] = useState("");
  const [selection, setSelection] = useState<DeviceSyncSelection>(DEFAULT_SELECTION);
  const [invitation, setInvitation] = useState<PairingInvitation | null>(null);
  const [usbSessionId, setUsbSessionId] = useState<string | null>(null);
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
    } finally {
      if (!silent) setBusyAction(null);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  const credentialsAvailable = state?.secureCredentialsAvailable === true;
  const activeSessions = useMemo(
    () => state?.sessions.filter((session) => ACTIVE_SESSION_STATES.has(session.state)) ?? [],
    [state],
  );
  const hasUnclaimedSendingOffer = activeSessions.some(
    (session) => session.direction === "host" && session.peerId === null,
  );
  const hasSelectedCategory = selection.flashcards || selection.documents || selection.apiSettings;

  useEffect(() => {
    if (!state || credentialsAvailable) return;
    setSelection((current) =>
      current.apiCredentials ? { ...current, apiCredentials: false } : current,
    );
  }, [credentialsAvailable, state]);

  useEffect(() => {
    const invitationWaiting = invitation !== null && invitation.expiresAt > Date.now();
    if (!invitationWaiting && busyAction !== "receive" && activeSessions.length === 0) return;
    const interval = window.setInterval(() => void refresh(true), 1_500);
    return () => window.clearInterval(interval);
  }, [activeSessions.length, busyAction, invitation]);

  useEffect(() => {
    if (!usbSessionId || !state) return;
    const session = state.sessions.find((candidate) => candidate.id === usbSessionId);
    if (!session || ACTIVE_SESSION_STATES.has(session.state)) return;
    if (session.state === "completed") {
      showToast("success", "USB synchronization completed.");
    } else if (session.state === "canceled") {
      showToast("info", "USB synchronization was canceled.");
    } else {
      showToast(
        "error",
        session.errorMessage ?? "USB synchronization did not complete. Reconnect the phone and try again.",
      );
    }
    setUsbSessionId(null);
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
      setState((current) => (current ? { ...current, identity } : current));
      showToast("success", "Device name saved.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
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
      const created = await startDevicePairing({
        transport: "local-network",
        hostOverride: hostOverride.trim() || null,
        selection,
      });
      setInvitation(created);
      await refresh(true);
      showToast(
        "info",
        hasUnclaimedSendingOffer
          ? "The previous waiting offer was canceled and replaced with a new five-minute network offer."
          : "Network sync offer is ready for five minutes.",
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }

  async function startUsbSync(): Promise<void> {
    if (busyAction) return;
    if (!hasSelectedCategory) {
      setErrorMessage("Select at least one synchronization category before starting USB sync.");
      return;
    }
    setBusyAction("usb");
    setInvitation(null);
    setErrorMessage(null);
    try {
      const created = await startDevicePairing({ transport: "usb", selection });
      setUsbSessionId(created.sessionId);
      await refresh(true);
      showToast("info", "USB sync started. Keep the Android phone connected while ostudy transfers the selected data.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }

  function showReceiveResult(result: DeviceSyncResult): void {
    const credentialCopy =
      result.transferredCredentialCount > 0
        ? ` ${result.transferredCredentialCount} saved API credential${
            result.transferredCredentialCount === 1 ? " was" : "s were"
          } transferred.`
        : "";
    const credentialWarning = result.credentialWarning ? ` ${result.credentialWarning}` : "";
    showToast(
      result.conflictCount > 0 || result.credentialWarning ? "info" : "success",
      `Received from ${result.peer.displayName}: ${result.changedRowCount} rows changed${
        result.conflictCount > 0 ? ` and ${result.conflictCount} conflicts need review` : ""
      }.${credentialCopy}${credentialWarning}`,
    );
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
    } finally {
      setBusyAction(null);
    }
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
      showToast(
        "info",
        `${session.direction === "host" ? "Sending" : "Receiving"} request canceled.`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setCancelingSessionId(null);
    }
  }

  async function copyInvitation(): Promise<void> {
    if (!invitation) return;
    try {
      await navigator.clipboard.writeText(invitation.invitation);
      showToast("success", "Synchronization invitation copied.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  async function removePeer(peerId: string, name: string): Promise<void> {
    const accepted = await confirm({
      title: `Remove ${name}?`,
      description:
        "This device will no longer be trusted. Create or accept a new synchronization offer before transferring data again.",
      confirmLabel: "Remove device",
      destructive: true,
    });
    if (!accepted) return;
    setBusyAction(`remove:${peerId}`);
    try {
      await removeDeviceSyncPeer(peerId);
      await refresh(true);
      showToast("success", `${name} was removed.`);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }

  async function resolveConflict(
    conflict: DeviceSyncConflict,
    resolution: "keep_left" | "keep_right" | "delete",
  ): Promise<void> {
    if (resolution === "delete") {
      const accepted = await confirm({
        title: "Delete this synchronized record?",
        description:
          "The record will be removed from this library. A future offer from this device can carry that decision to another device.",
        confirmLabel: "Delete record",
        destructive: true,
      });
      if (!accepted) return;
    }
    setBusyAction(`conflict:${conflict.id}`);
    setErrorMessage(null);
    try {
      await resolveDeviceSyncConflict({ conflictId: conflict.id, resolution });
      await refresh(true);
      showToast("success", "Synchronization conflict resolved.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }

  const qrSource = invitation?.qrSvg
    ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(invitation.qrSvg)}`
    : null;

  return (
    <section className={ownedClassNames(ownedStyleMaps, `${settingsStyles.fullWidthCard} ${styles.syncCard}`)}>
      <div className={settingsStyles.sectionHeading}>
        <div>
          <h2>Device synchronization</h2>
          <p>
            Send selected Flashcards, Documents and their LLM processing data, Study Guides, and
            optional API configuration over your private local network or directly through a USB
            cable. Appearance and other app settings always stay local to each device.
          </p>
        </div>
        <button
          type="button"
          className={ownedClassNames(ownedStyleMaps, "text-button")}
          disabled={busyAction !== null}
          onClick={() => void refresh()}
        >
          Refresh
        </button>
      </div>

      {errorMessage ? <div className={ownedClassNames(ownedStyleMaps, "inline-alert error-alert")}>{errorMessage}</div> : null}
      {!state && busyAction === "refresh" ? (
        <div className={ownedClassNames(ownedStyleMaps, "inline-progress")}>Opening the device synchronization store…</div>
      ) : null}

      {state ? (
        <>
          <div className={styles.identityGrid}>
            <label className={settingsStyles.stackedField}>
              This device's name
              <input
                type="text"
                maxLength={120}
                value={deviceName}
                onChange={(event) => setDeviceName(event.currentTarget.value)}
              />
            </label>
            <div className={styles.identityMeta}>
              <span>Identity fingerprint</span>
              <code>{formatFingerprint(state.fingerprint)}</code>
            </div>
            <button
              type="button"
              className={ownedClassNames(ownedStyleMaps, "secondary-button")}
              disabled={busyAction !== null || deviceName.trim() === state.identity.displayName}
              onClick={() => void saveDeviceName()}
            >
              {busyAction === "name" ? "Saving…" : "Save device name"}
            </button>
          </div>

          <div className={styles.pairingGrid}>
            <article className={styles.pairingPanel}>
              <h3>Send to another device</h3>
              <p>
                Choose exactly which categories this one-way transfer may contain. The receiving
                device never uploads its own library in response.
              </p>

              <fieldset className={styles.scopeFieldset}>
                <legend>What to synchronize</legend>
                <label className={styles.scopeOption}>
                  <input
                    type="checkbox"
                    checked={selection.flashcards}
                    disabled={busyAction !== null}
                    onChange={(event) => changeSelection("flashcards", event.currentTarget.checked)}
                  />
                  <span>
                    <strong>Flashcards</strong>
                    <small>Decks, folders, cards, templates, tags, schedules, review history, and managed card images.</small>
                  </span>
                </label>
                <label className={styles.scopeOption}>
                  <input
                    type="checkbox"
                    checked={selection.documents}
                    disabled={busyAction !== null}
                    onChange={(event) => changeSelection("documents", event.currentTarget.checked)}
                  />
                  <span>
                    <strong>Documents</strong>
                    <small>Source files, canonical content, OCR/semantic data, outlines, LLM processing jobs, checkpoints, and learning-plan data.</small>
                  </span>
                </label>
                <label className={styles.scopeOption}>
                  <input
                    type="checkbox"
                    checked={selection.studyGuides}
                    disabled={busyAction !== null}
                    onChange={(event) => changeSelection("studyGuides", event.currentTarget.checked)}
                  />
                  <span>
                    <strong>Study Guides</strong>
                    <small>Generated guide revisions, sections, learner overlays, and guide checkpoints. Requires Documents.</small>
                  </span>
                </label>
                <label className={styles.scopeOption}>
                  <input
                    type="checkbox"
                    checked={selection.apiSettings}
                    disabled={busyAction !== null}
                    onChange={(event) => changeSelection("apiSettings", event.currentTarget.checked)}
                  />
                  <span>
                    <strong>API provider settings</strong>
                    <small>Provider profiles and task defaults. Theme, layout, fonts, and other application preferences always stay local.</small>
                  </span>
                </label>
                <label className={styles.scopeOption}>
                  <input
                    type="checkbox"
                    checked={selection.apiCredentials}
                    disabled={busyAction !== null || !credentialsAvailable}
                    onChange={(event) => changeSelection("apiCredentials", event.currentTarget.checked)}
                  />
                  <span>
                    <strong>Saved API keys</strong>
                    <small>Securely stored provider credentials. Requires API provider settings and usable secure storage on both devices.</small>
                  </span>
                </label>
              </fieldset>
              {!credentialsAvailable ? (
                <p className={settingsStyles.securityNote}>
                  This device's secure secret store is unavailable, so saved API keys cannot be
                  added to the offer. Provider settings can still be synchronized without keys.
                </p>
              ) : null}

              {state.usbHostAvailable ? (
                <>
                  <p className={settingsStyles.securityNote}>
                    <strong>USB cable:</strong> plug an Android phone into this computer with a
                    data-capable cable, then click USB Sync. ostudy discovers the phone and uses a
                    direct encrypted USB accessory channel—no tethering, IP address, QR code,
                    developer mode, or USB debugging setup is required. Android may ask once which
                    app should handle the newly attached ostudy accessory.
                  </p>
                  <button
                    type="button"
                    className={ownedClassNames(ownedStyleMaps, "primary-button")}
                    disabled={busyAction !== null || !hasSelectedCategory}
                    onClick={() => void startUsbSync()}
                  >
                    {busyAction === "usb" ? "Connecting phone…" : "USB Sync"}
                  </button>
                </>
              ) : null}

              <label className={settingsStyles.stackedField}>
                Advanced LAN address override
                <input
                  type="text"
                  inputMode="decimal"
                  placeholder="Usually detected automatically"
                  value={hostOverride}
                  onChange={(event) => setHostOverride(event.currentTarget.value)}
                />
                <span className={settingsStyles.fieldHelp}>
                  Only needed if automatic private LAN-address detection fails. This field is never
                  used for USB synchronization.
                </span>
              </label>
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                disabled={busyAction !== null || !state.localNetworkAvailable || !hasSelectedCategory}
                onClick={() => void hostPairing()}
              >
                {busyAction === "host"
                  ? "Opening listener…"
                  : hasUnclaimedSendingOffer
                    ? "Replace with network offer"
                    : "Create network sync offer"}
              </button>

              {invitation && qrSource ? (
                <div className={styles.invitationPanel}>
                  <div className={styles.qrFrame}>
                    <img src={qrSource} alt="QR code containing the short-lived ostudy synchronization offer" />
                  </div>
                  <div className={styles.invitationDetails}>
                    <strong>Expires {formatDateTime(invitation.expiresAt)}</strong>
                    <span>Local network · {invitation.endpoint}</span>
                    <code>{formatFingerprint(invitation.fingerprint)}</code>
                    <p className={styles.scanHint}>
                      Includes: {describeSelection(invitation.selection)}.
                    </p>
                    {invitation.selection.apiCredentials && !invitation.credentialsAvailable ? (
                      <p className={styles.scanHint}>
                        Saved API keys were selected, but this device has no securely stored credential bytes available for this offer.
                      </p>
                    ) : null}
                    <p className={styles.scanHint}>
                      Keep the entire white border visible. Scanning this code accepts the offer and
                      starts receiving immediately.
                    </p>
                  </div>
                  <button type="button" className={ownedClassNames(ownedStyleMaps, "secondary-button")} onClick={() => void copyInvitation()}>
                    Copy invitation text
                  </button>
                </div>
              ) : null}
            </article>

            <article className={styles.pairingPanel}>
              <h3>Receive from another device</h3>
              <p>
                Network offers can be scanned on Android or pasted below. Direct USB transfers are
                accepted automatically by the Android app when the desktop starts USB Sync; there is
                no QR or network setup on the cable path. Accepting a transfer never uploads this
                device's library back.
              </p>
              {state.qrScanAvailable ? (
                <>
                  <p className={styles.scanHint}>
                    Center the complete QR code inside the camera frame and hold the phone steady.
                    After detection, ostudy accepts the network offer and begins receiving automatically.
                  </p>
                  <button
                    type="button"
                    className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                    disabled={busyAction !== null}
                    onClick={() => void scanPairingCode()}
                  >
                    {busyAction === "scan"
                      ? "Scanning…"
                      : busyAction === "receive"
                        ? "Receiving…"
                        : "Scan and receive"}
                  </button>
                  {scanFeedback !== "idle" ? (
                    <div
                      className={styles.scanStatus}
                      data-state={scanFeedback}
                      role="status"
                      aria-live="polite"
                    >
                      <span className={styles.scanStatusDot} aria-hidden="true" />
                      {scanFeedback === "scanning"
                        ? "Camera open — looking for an ostudy sync offer…"
                        : scanFeedback === "detected"
                          ? "Sync offer detected, but the connection did not complete. Review the error above, then retry while this offer is still valid."
                          : scanFeedback === "receiving"
                            ? "Sync offer detected — receiving from the sending device…"
                            : scanFeedback === "received"
                              ? "Sync offer accepted — synchronization complete."
                              : "Scan cancelled. No synchronization was started."}
                    </div>
                  ) : null}
                </>
              ) : null}
              <label className={settingsStyles.stackedField}>
                Synchronization invitation
                <textarea
                  rows={5}
                  maxLength={8_192}
                  placeholder="ostudy-sync://pair/…"
                  value={joinInvitation}
                  onChange={(event) => {
                    setJoinInvitation(event.currentTarget.value);
                    if (["detected", "received"].includes(scanFeedback)) setScanFeedback("idle");
                  }}
                />
              </label>
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "primary-button")}
                disabled={busyAction !== null || !joinInvitation.trim() || !state.localNetworkAvailable}
                onClick={() => void joinPairing()}
              >
                {busyAction === "receive"
                  ? "Receiving…"
                  : scanFeedback === "detected" && joinInvitation.trim()
                    ? "Retry connection"
                    : "Accept sync offer"}
              </button>
            </article>
          </div>

          {activeSessions.length > 0 ? (
            <div className={styles.sessionStrip} aria-label="Active synchronization sessions">
              {activeSessions.map((session) => (
                <div className={styles.sessionPill} key={session.id}>
                  <span>
                    <strong>{session.direction === "host" ? "Sending" : "Receiving"}</strong>
                    {humanizeState(session.state)} · {sessionPeerLabel(session, state)}
                  </span>
                  <button
                    type="button"
                    className={ownedClassNames(ownedStyleMaps, "text-button")}
                    disabled={cancelingSessionId !== null}
                    aria-label={`Cancel ${session.direction === "host" ? "sending" : "receiving"} request`}
                    onClick={() => void cancelSession(session)}
                  >
                    {cancelingSessionId === session.id ? "Canceling…" : "Cancel"}
                  </button>
                </div>
              ))}
            </div>
          ) : null}

          <div className={styles.listSection}>
            <div className={styles.listHeading}>
              <h3>Trusted devices</h3>
              <span>{state.peers.length}</span>
            </div>
            {state.peers.length === 0 ? (
              <p className={styles.emptyCopy}>No devices have been paired yet.</p>
            ) : (
              <div className={styles.peerList}>
                {state.peers.map((peer) => (
                  <article key={peer.id}>
                    <div>
                      <strong>{peer.displayName}</strong>
                      <span>{formatFingerprint(peer.fingerprint)}</span>
                      <small>
                        {peer.lastSyncedAt
                          ? `Last synchronized ${formatDateTime(peer.lastSyncedAt)}`
                          : `Paired ${formatDateTime(peer.pairedAt)}`}
                      </small>
                    </div>
                    <button
                      type="button"
                      className={ownedClassNames(ownedStyleMaps, "text-button danger-text")}
                      disabled={busyAction !== null}
                      onClick={() => void removePeer(peer.id, peer.displayName)}
                    >
                      {busyAction === `remove:${peer.id}` ? "Removing…" : "Remove"}
                    </button>
                  </article>
                ))}
              </div>
            )}
          </div>

          <div className={styles.listSection}>
            <div className={styles.listHeading}>
              <h3>Conflicts requiring a decision</h3>
              <span>{state.conflicts.length}</span>
            </div>
            {state.conflicts.length === 0 ? (
              <p className={styles.emptyCopy}>No unresolved synchronization conflicts.</p>
            ) : (
              <div className={styles.conflictList}>
                {state.conflicts.map((conflict) => (
                  <article key={conflict.id}>
                    <div className={styles.conflictHeading}>
                      <div>
                        <strong>{humanizeTable(conflict.tableName)}</strong>
                        <span>{conflict.conflictingFields.join(", ") || "Whole record"}</span>
                      </div>
                      <small>{formatDateTime(conflict.createdAt)}</small>
                    </div>
                    <div className={styles.conflictVersions}>
                      <ConflictVersion
                        label={conflict.leftDeviceId === state.identity.deviceId ? "This device" : "Left device"}
                        payload={conflict.leftPayloadJson}
                      />
                      <ConflictVersion
                        label={conflict.rightDeviceId === state.identity.deviceId ? "This device" : "Paired device"}
                        payload={conflict.rightPayloadJson}
                      />
                    </div>
                    <div className={settingsStyles.actionRow}>
                      <button
                        type="button"
                        className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                        disabled={busyAction !== null}
                        onClick={() => void resolveConflict(conflict, "keep_left")}
                      >
                        Keep left
                      </button>
                      <button
                        type="button"
                        className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                        disabled={busyAction !== null}
                        onClick={() => void resolveConflict(conflict, "keep_right")}
                      >
                        Keep right
                      </button>
                      <button
                        type="button"
                        className={ownedClassNames(ownedStyleMaps, "text-button danger-text")}
                        disabled={busyAction !== null}
                        onClick={() => void resolveConflict(conflict, "delete")}
                      >
                        Delete record
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </div>
        </>
      ) : null}
    </section>
  );
}

function ConflictVersion({ label, payload }: { label: string; payload: string | null }) {
  return (
    <div>
      <strong>{label}</strong>
      {payload ? <pre>{prettyPayload(payload)}</pre> : <p>Deleted on this device</p>}
    </div>
  );
}

function prettyPayload(payload: string): string {
  try {
    return JSON.stringify(JSON.parse(payload), null, 2);
  } catch {
    return payload;
  }
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

function formatFingerprint(value: string): string {
  return value.match(/.{1,4}/g)?.slice(0, 8).join(" ") ?? value;
}

function formatDateTime(value: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function humanizeState(value: string): string {
  return value.replaceAll("_", " ").replace(/^./, (character) => character.toUpperCase());
}

function sessionPeerLabel(session: DeviceSyncSession, state: DeviceSyncState): string {
  if (!session.peerId) return session.endpointHint === "usb" ? "USB cable" : session.endpointHint;
  return state.peers.find((peer) => peer.id === session.peerId)?.displayName ?? session.endpointHint;
}

function humanizeTable(value: string): string {
  return value
    .replaceAll("_", " ")
    .replace(/\b\w/g, (character) => character.toUpperCase());
}
