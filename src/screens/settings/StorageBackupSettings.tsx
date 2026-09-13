import { Checkbox } from "../../ui/primitives/FormControls";
import { Panel } from "../../ui/primitives/Panel";
import { useEffect, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { prepareLibraryBackup, prepareLibraryRestore } from "../../ipc/backup";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import { getAppInfo, type AppInfo, type AppSettings } from "../../ipc/settings";
import { publishExport, selectExternalImport, shareExport } from "../../platform/files";
import { usePlatformCapabilities } from "../../platform/PlatformProvider";
import { useConfirm } from "../../ui/overlays/overlayContext";
import {
  InfoButton,
  MetadataItem,
  MetadataList,
} from "../../ui/overlays/MetadataDialog";
import { DeviceSyncSettings } from "./DeviceSyncSettings";
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
    <div className="tw:grid tw:min-w-0 tw:gap-6">
      {errorMessage ? (
        <InlineNotice kind="error">{errorMessage}</InlineNotice>
      ) : null}

      <Panel className="tw:min-w-0 tw:self-start">
        <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Data safety</h2>
        <label className="tw:flex tw:min-h-11 tw:items-start tw:justify-between tw:gap-6 tw:text-label tw:[&>span:first-child]:grid tw:[&>span:first-child]:gap-1 tw:[&_small]:text-muted-foreground tw:[&_small]:font-normal tw:[&_small]:leading-relaxed">
          <span>
            <strong>Confirm deletions</strong>
            <small>Ask before destructive content removal.</small>
          </span>
          <Checkbox
            checked={draft.confirmDeletions}
            onCheckedChange={(checked) => update({ confirmDeletions: checked })}
          />
        </label>
        <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-3">
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
        <p className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">
          Complete backups include decks, templates, managed images, schedules, review events,
          sessions, analytics sources, AI provider metadata, task defaults, and ordinary settings.
        </p>
        <p className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">
          API keys and session-only credentials are never stored in SQLite and are never included
          in an ostudy backup.
        </p>
        {restorePrepared ? (
          <InlineNotice kind="info">
            Restore is prepared. Close every ostudy window and reopen the application to apply it.
          </InlineNotice>
        ) : null}
      </Panel>

      <DeviceSyncSettings showToast={showToast} />

      <Panel>
        <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-3 tw:justify-between">
          <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">Storage</h2>
          {appInfo ? (
            <InfoButton
              label="View storage details"
              title="Storage details"
              description="Operational paths and local application version information."
            >
              <MetadataList>
                <MetadataItem label="Application data" value={appInfo.dataDirectory} />
                <MetadataItem label="Database" value={appInfo.databasePath} />
                <MetadataItem label="Images" value={appInfo.imageDirectory} />
                <MetadataItem
                  label="Diagnostics"
                  value={appInfo.diagnosticLogPath ?? "Available after restart"}
                />
                <MetadataItem label="Version" value={appInfo.appVersion} />
                <MetadataItem label="Schema" value={String(appInfo.databaseSchemaVersion)} />
              </MetadataList>
            </InfoButton>
          ) : null}
        </div>
        <p className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">
          <strong>Local library</strong>
          <br />
          The active study library is stored on this device. Backup and device-sync controls are
          available above.
        </p>
        {!appInfo && !errorMessage ? (
          <InlineProgress>Reading storage paths…</InlineProgress>
        ) : null}
      </Panel>
    </div>
  );
}
