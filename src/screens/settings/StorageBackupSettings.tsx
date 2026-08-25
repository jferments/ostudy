import { useEffect, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { prepareLibraryBackup, prepareLibraryRestore } from "../../ipc/backup";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import { getAppInfo, type AppInfo, type AppSettings } from "../../ipc/settings";
import { publishExport, selectExternalImport, shareExport } from "../../platform/files";
import { usePlatformCapabilities } from "../../platform/PlatformProvider";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { DeviceSyncSettings } from "./DeviceSyncSettings";
import { PathRow } from "./SettingsControls";
import styles from "./Settings.module.css";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";

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
  const capabilities = usePlatformCapabilities();
  const canShare = capabilities.native?.services.systemShare.state === "available";

  useEffect(() => {
    void getAppInfo()
      .then(setAppInfo)
      .catch((error: unknown) => setErrorMessage(getErrorMessage(error)));
  }, []);

  async function backupLibrary(): Promise<void> {
    if (isTransferringLibrary) return;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    try {
      const prepared = await prepareLibraryBackup(createOperationId());
      const receipt = await publishExport(prepared.preparedExport.id);
      if (!receipt) return;
      showToast(
        "success",
        `Backed up ${prepared.deckCount} ${prepared.deckCount === 1 ? "deck" : "decks"}, ${prepared.templateCount} ${prepared.templateCount === 1 ? "template" : "templates"}, and ${prepared.reviewCount} review ${prepared.reviewCount === 1 ? "event" : "events"}.`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsTransferringLibrary(false);
    }
  }

  async function shareLibraryBackup(): Promise<void> {
    if (isTransferringLibrary) return;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    try {
      const prepared = await prepareLibraryBackup(createOperationId());
      const launched = await shareExport(prepared.preparedExport.id);
      if (!launched.launched) return;
      showToast(
        "info",
        `Opened system sharing for a backup containing ${prepared.deckCount} ${
          prepared.deckCount === 1 ? "deck" : "decks"
        }.`,
      );
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsTransferringLibrary(false);
    }
  }

  async function restoreLibrary(): Promise<void> {
    if (isTransferringLibrary) return;
    const source = await selectExternalImport("library_backup");
    if (!source) return;
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
      const result = await prepareLibraryRestore(source.id);
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
      {errorMessage ? (
        <InlineNotice kind="error" className={styles.fullWidth}>{errorMessage}</InlineNotice>
      ) : null}

      <section className={styles.settingsCard}>
        <h2>Data safety</h2>
        <label className={styles.switchRow}>
          <span>
            <strong>Confirm deletions</strong>
            <small>Ask before destructive content removal.</small>
          </span>
          <input
            type="checkbox"
            checked={draft.confirmDeletions}
            onChange={(event) => update({ confirmDeletions: event.currentTarget.checked })}
          />
        </label>
        <div className={styles.actionRow}>
          <Button
            type="button"
            variant="secondary"
            disabled={isTransferringLibrary}
            onClick={() => void backupLibrary()}
          >
            {isTransferringLibrary ? "Working…" : "Back up library"}
          </Button>
          {canShare ? (
            <Button
              type="button"
              variant="secondary"
              disabled={isTransferringLibrary}
              onClick={() => void shareLibraryBackup()}
            >
              Share backup
            </Button>
          ) : null}
          <Button
            type="button"
            variant="secondary"
            disabled={isTransferringLibrary}
            onClick={() => void restoreLibrary()}
          >
            Restore backup
          </Button>
        </div>
        <p className={styles.note}>
          Complete backups include decks, templates, managed images, schedules, review events,
          sessions, analytics sources, AI provider metadata, task defaults, and ordinary settings.
        </p>
        <p className={styles.securityNote}>
          API keys and session-only credentials are never stored in SQLite and are never included
          in an ostudy backup.
        </p>
        {restorePrepared ? (
          <InlineNotice kind="info">
            Restore is prepared. Close every ostudy window and reopen the application to apply it.
          </InlineNotice>
        ) : null}
      </section>

      <DeviceSyncSettings showToast={showToast} />

      <section className={styles.fullWidthCard}>
        <h2>Storage</h2>
        <p className={styles.securityNote}>
          <strong>Local library</strong>
          <br />
          The active study library is stored on this device. Backup and device-sync controls are
          available above.
        </p>
        {appInfo ? (
          <dl className={styles.pathList}>
            <PathRow label="Application data" value={appInfo.dataDirectory} />
            <PathRow label="Database" value={appInfo.databasePath} />
            <PathRow label="Images" value={appInfo.imageDirectory} />
            <PathRow
              label="Diagnostics"
              value={appInfo.diagnosticLogPath ?? "Available after restart"}
            />
            <PathRow label="Version" value={appInfo.appVersion} />
            <PathRow label="Schema" value={String(appInfo.databaseSchemaVersion)} />
          </dl>
        ) : errorMessage ? null : (
          <InlineProgress>Reading storage paths…</InlineProgress>
        )}
      </section>
    </div>
  );
}
