import { useEffect, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { exportLibraryBackup, prepareLibraryRestore } from "../../ipc/backup";
import { getErrorMessage } from "../../ipc/common";
import { getAppInfo, type AppInfo, type AppSettings } from "../../ipc/settings";
import { openFile, saveFile } from "../../platform/files";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { PathRow } from "./SettingsControls";
import styles from "./Settings.module.css";

interface StorageBackupSettingsProps {
  draft: AppSettings;
  update: (patch: Partial<AppSettings>) => void;
  showToast: ShowToast;
}

export function StorageBackupSettings({ draft, update, showToast }: StorageBackupSettingsProps) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [isTransferringLibrary, setIsTransferringLibrary] = useState(false);
  const [restorePrepared, setRestorePrepared] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const confirm = useConfirm();

  useEffect(() => {
    void getAppInfo()
      .then(setAppInfo)
      .catch((error: unknown) => setErrorMessage(getErrorMessage(error)));
  }, []);

  async function backupLibrary(): Promise<void> {
    if (isTransferringLibrary) return;
    const destination = await saveFile({
      title: "Back up the complete ostudy library",
      defaultPath: `ostudy-library-${new Date().toISOString().slice(0, 10)}.ostudybackup`,
      filters: [{ name: "ostudy library backup", extensions: ["ostudybackup"] }],
    });
    if (typeof destination !== "string") return;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    try {
      const result = await exportLibraryBackup(destination);
      showToast(
        "success",
        `Backed up ${result.deckCount} ${result.deckCount === 1 ? "deck" : "decks"}, ${result.templateCount} ${result.templateCount === 1 ? "template" : "templates"}, and ${result.reviewCount} review ${result.reviewCount === 1 ? "event" : "events"}.`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsTransferringLibrary(false);
    }
  }

  async function restoreLibrary(): Promise<void> {
    if (isTransferringLibrary) return;
    const source = await openFile({
      title: "Choose an ostudy library backup",
      multiple: false,
      directory: false,
      filters: [{ name: "ostudy library backup", extensions: ["ostudybackup"] }],
    });
    if (typeof source !== "string") return;
    const accepted = await confirm({
      title: "Restore this complete library?",
      description:
        "The current library will be replaced after validation when ostudy next starts. This includes decks, templates, schedules, review history, managed images, analytics sources, AI provider metadata, task defaults, and settings. API credentials are never included.",
      confirmLabel: "Prepare restore",
      destructive: true,
    });
    if (!accepted) return;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    try {
      const result = await prepareLibraryRestore(source);
      setRestorePrepared(result.restartRequired);
      showToast(
        "info",
        `Restore prepared for ${result.deckCount} ${result.deckCount === 1 ? "deck" : "decks"}. Close and reopen ostudy to apply it.`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsTransferringLibrary(false);
    }
  }

  return (
    <div className={styles.panelGrid}>
      {errorMessage ? <div className={`${styles.fullWidth} inline-alert error-alert`}>{errorMessage}</div> : null}
      <section className={styles.settingsCard}>
        <h2>Storage</h2>
        {appInfo ? (
          <dl className={styles.pathList}>
            <PathRow label="Application data" value={appInfo.dataDirectory} />
            <PathRow label="Database" value={appInfo.databasePath} />
            <PathRow label="Images" value={appInfo.imageDirectory} />
            <PathRow label="Version" value={appInfo.appVersion} />
            <PathRow label="Schema" value={String(appInfo.databaseSchemaVersion)} />
          </dl>
        ) : errorMessage ? null : (
          <div className="inline-progress">Reading storage paths…</div>
        )}
      </section>

      <section className={styles.settingsCard}>
        <h2>Data safety</h2>
        <label className={styles.switchRow}>
          <span><strong>Confirm deletions</strong><small>Ask before destructive content removal.</small></span>
          <input
            type="checkbox"
            checked={draft.confirmDeletions}
            onChange={(event) => update({ confirmDeletions: event.currentTarget.checked })}
          />
        </label>
        <div className={styles.actionRow}>
          <button
            type="button"
            className="secondary-button"
            disabled={isTransferringLibrary}
            onClick={() => void backupLibrary()}
          >
            {isTransferringLibrary ? "Working…" : "Back up library"}
          </button>
          <button
            type="button"
            className="secondary-button"
            disabled={isTransferringLibrary}
            onClick={() => void restoreLibrary()}
          >
            Restore backup
          </button>
        </div>
        <p className={styles.note}>
          Complete backups include decks, templates, managed images, schedules, review events, sessions, analytics sources, AI provider metadata, task defaults, and ordinary settings.
        </p>
        <p className={styles.securityNote}>
          API keys and session-only credentials are never stored in SQLite and are never included in an ostudy backup.
        </p>
        {restorePrepared ? (
          <div className="inline-alert info-alert">
            Restore is prepared. Close every ostudy window and reopen the application to apply it.
          </div>
        ) : null}
      </section>
    </div>
  );
}
