import { useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { getErrorMessage } from "../../ipc/common";
import {
  getDeviceSyncState,
  joinDevicePairing,
  removeDeviceSyncPeer,
  resolveDeviceSyncConflict,
  scanDevicePairingQr,
  startDevicePairing,
  updateDeviceSyncName,
  type DeviceSyncConflict,
  type DeviceSyncState,
  type PairingInvitation,
} from "../../ipc/deviceSync";
import { useConfirm } from "../../ui/overlays/overlayContext";
import settingsStyles from "./Settings.module.css";
import styles from "./DeviceSyncSettings.module.css";

interface DeviceSyncSettingsProps {
  showToast: ShowToast;
}

export function DeviceSyncSettings({ showToast }: DeviceSyncSettingsProps) {
  const [state, setState] = useState<DeviceSyncState | null>(null);
  const [deviceName, setDeviceName] = useState("");
  const [hostOverride, setHostOverride] = useState("");
  const [joinInvitation, setJoinInvitation] = useState("");
  const [includeCredentials, setIncludeCredentials] = useState(false);
  const [invitation, setInvitation] = useState<PairingInvitation | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
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

  useEffect(() => {
    if (!invitation || invitation.expiresAt <= Date.now()) return;
    const interval = window.setInterval(() => void refresh(true), 2_500);
    return () => window.clearInterval(interval);
  }, [invitation]);

  const credentialsAllowed = state?.secureCredentialsAvailable === true;
  const activeSessions = useMemo(
    () =>
      state?.sessions.filter((session) =>
        ["waiting", "connecting", "transferring", "merging"].includes(session.state),
      ) ?? [],
    [state],
  );

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
    setBusyAction("host");
    setErrorMessage(null);
    try {
      const created = await startDevicePairing({
        hostOverride: hostOverride.trim() || null,
        includeCredentials: includeCredentials && credentialsAllowed,
      });
      setInvitation(created);
      await refresh(true);
      showToast("info", "Pairing code is ready on this device for five minutes.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }

  async function scanPairingCode(): Promise<void> {
    if (busyAction) return;
    setBusyAction("scan");
    setErrorMessage(null);
    try {
      const scanned = await scanDevicePairingQr();
      if (scanned) {
        setJoinInvitation(scanned);
        showToast("success", "Pairing code scanned. Review the credential option, then connect.");
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }

  async function joinPairing(): Promise<void> {
    if (busyAction || !joinInvitation.trim()) return;
    setBusyAction("join");
    setErrorMessage(null);
    try {
      const result = await joinDevicePairing({
        invitation: joinInvitation.trim(),
        includeCredentials: includeCredentials && credentialsAllowed,
      });
      setJoinInvitation("");
      setInvitation(null);
      await refresh(true);
      showToast(
        result.conflictCount > 0 ? "info" : "success",
        `Synchronized with ${result.peer.displayName}: ${result.changedRowCount} rows changed${
          result.conflictCount > 0 ? ` and ${result.conflictCount} conflicts need review` : ""
        }.`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusyAction(null);
    }
  }

  async function copyInvitation(): Promise<void> {
    if (!invitation) return;
    try {
      await navigator.clipboard.writeText(invitation.invitation);
      showToast("success", "Pairing invitation copied.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    }
  }

  async function removePeer(peerId: string, name: string): Promise<void> {
    const accepted = await confirm({
      title: `Remove ${name}?`,
      description:
        "This device will no longer be trusted. Pair it again before transferring library changes or credentials.",
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
          "The record will be removed from this library. The next synchronization will carry that decision to the paired device.",
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

  const qrSource = invitation
    ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(invitation.qrSvg)}`
    : null;

  return (
    <section className={`${settingsStyles.fullWidthCard} ${styles.syncCard}`}>
      <div className={settingsStyles.sectionHeading}>
        <div>
          <h2>Device synchronization</h2>
          <p>
            Pair trusted devices over your private local network, then merge decks, cards,
            templates, settings, review history, schedules, and managed images.
          </p>
        </div>
        <button
          type="button"
          className="text-button"
          disabled={busyAction !== null}
          onClick={() => void refresh()}
        >
          Refresh
        </button>
      </div>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {!state && busyAction === "refresh" ? (
        <div className="inline-progress">Opening the device synchronization store…</div>
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
              className="secondary-button"
              disabled={busyAction !== null || deviceName.trim() === state.identity.displayName}
              onClick={() => void saveDeviceName()}
            >
              {busyAction === "name" ? "Saving…" : "Save device name"}
            </button>
          </div>

          <div className={styles.pairingGrid}>
            <article className={styles.pairingPanel}>
              <h3>Show a pairing code</h3>
              <p>
                Keep ostudy open on this device, then scan the code from the other device.
                Invitations expire and can be used only for their active listener session.
              </p>
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
                  Enter a private IP only when automatic LAN-address detection fails.
                </span>
              </label>
              <button
                type="button"
                className="primary-button"
                disabled={busyAction !== null || !state.localNetworkAvailable}
                onClick={() => void hostPairing()}
              >
                {busyAction === "host" ? "Opening listener…" : "Create pairing code"}
              </button>

              {invitation && qrSource ? (
                <div className={styles.invitationPanel}>
                  <img src={qrSource} alt="QR code containing the short-lived ostudy pairing invitation" />
                  <div>
                    <strong>Expires {formatDateTime(invitation.expiresAt)}</strong>
                    <span>{invitation.endpoint}</span>
                    <code>{formatFingerprint(invitation.fingerprint)}</code>
                  </div>
                  <button type="button" className="secondary-button" onClick={() => void copyInvitation()}>
                    Copy invitation text
                  </button>
                </div>
              ) : null}
            </article>

            <article className={styles.pairingPanel}>
              <h3>Connect to another device</h3>
              <p>
                Scan the displayed code on Android or paste the invitation text. Both devices
                authenticate the scanned identity before any library bytes are transferred.
              </p>
              {state.qrScanAvailable ? (
                <button
                  type="button"
                  className="secondary-button"
                  disabled={busyAction !== null}
                  onClick={() => void scanPairingCode()}
                >
                  {busyAction === "scan" ? "Opening camera…" : "Scan pairing QR code"}
                </button>
              ) : null}
              <label className={settingsStyles.stackedField}>
                Pairing invitation
                <textarea
                  rows={5}
                  maxLength={8_192}
                  placeholder="ostudy-sync://pair/…"
                  value={joinInvitation}
                  onChange={(event) => setJoinInvitation(event.currentTarget.value)}
                />
              </label>
              <button
                type="button"
                className="primary-button"
                disabled={busyAction !== null || !joinInvitation.trim() || !state.localNetworkAvailable}
                onClick={() => void joinPairing()}
              >
                {busyAction === "join" ? "Synchronizing…" : "Pair and synchronize"}
              </button>
            </article>
          </div>

          <label className={`${settingsStyles.switchRow} ${styles.credentialToggle}`}>
            <span>
              <strong>Transfer saved API credentials during this pairing</strong>
              <small>
                Off by default. When enabled on both devices, credentials travel only through the
                authenticated encrypted channel and are re-encrypted into the receiving secure store.
              </small>
            </span>
            <input
              type="checkbox"
              checked={includeCredentials}
              disabled={!credentialsAllowed || busyAction !== null}
              onChange={(event) => setIncludeCredentials(event.currentTarget.checked)}
            />
          </label>
          {!credentialsAllowed ? (
            <p className={settingsStyles.securityNote}>
              This platform's secure secret store is unavailable, so API credentials cannot be
              transferred. Provider metadata and ordinary settings can still synchronize.
            </p>
          ) : null}

          {activeSessions.length > 0 ? (
            <div className={styles.sessionStrip} aria-label="Active synchronization sessions">
              {activeSessions.map((session) => (
                <span key={session.id}>
                  <strong>{session.direction === "host" ? "Hosting" : "Connecting"}</strong>
                  {humanizeState(session.state)} · {session.endpointHint}
                </span>
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
                      className="text-button danger-text-button"
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
                        className="secondary-button"
                        disabled={busyAction !== null}
                        onClick={() => void resolveConflict(conflict, "keep_left")}
                      >
                        Keep left
                      </button>
                      <button
                        type="button"
                        className="secondary-button"
                        disabled={busyAction !== null}
                        onClick={() => void resolveConflict(conflict, "keep_right")}
                      >
                        Keep right
                      </button>
                      <button
                        type="button"
                        className="text-button danger-text-button"
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

function humanizeTable(value: string): string {
  return value
    .replaceAll("_", " ")
    .replace(/\b\w/g, (character) => character.toUpperCase());
}
