import { Checkbox } from "../../ui/primitives/FormControls";
import { Panel } from "../../ui/primitives/Panel";
import { useEffect, useRef, useState } from "react";

import { restoredLibraryMessage } from "../../components/libraryRestoreResult";
import { refreshLibraryView } from "../../platform/libraryView";
import { releaseStagedExternalImport } from "../../ipc/externalFiles";
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
  const transferActive = useRef(false);
  const [transferLabel, setTransferLabel] = useState<string | null>(null);
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
    if (transferActive.current) return;
    transferActive.current = true;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    setTransferLabel("Creating a complete library backup…");
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
      transferActive.current = false;
      setIsTransferringLibrary(false);
      setTransferLabel(null);
    }
  }

  async function shareLibraryBackup(): Promise<void> {
    if (transferActive.current) return;
    transferActive.current = true;
    setIsTransferringLibrary(true);
    setErrorMessage(null);
    setTransferLabel("Creating a complete library backup…");
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
      transferActive.current = false;
      setIsTransferringLibrary(false);
      setTransferLabel(null);
    }
  }

  async function restoreLibrary(): Promise<void> {
    if (transferActive.current) return;
    transferActive.current = true;
    setIsTransferringLibrary(true);
    setTransferLabel("Choose a backup…");
    setErrorMessage(null);
    try {
      const source = await selectExternalImport("library_backup");
      if (!source) return;
      const accepted = await confirm({
        title: "Restore this complete library?",
        description:
          "This replaces the current library after validation, including documents, study guides, decks, schedules, review history, drafts, and settings. Back up anything you need to keep first. The interface refreshes when restoration finishes; oStudy stays running. Saved API keys are not changed, and session-only credentials must be entered again.",
        confirmLabel: "Restore library",
        destructive: true,
      });
      if (!accepted) {
        await releaseStagedExternalImport(source.id);
        return;
      }
      setTransferLabel("Validating and restoring your library…");
      const result = await prepareLibraryRestore(source.id);
      showToast("success", restoredLibraryMessage(result));
      refreshLibraryView();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      transferActive.current = false;
      setIsTransferringLibrary(false);
      setTransferLabel(null);
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
          Complete backups include documents, source files, study guides, collections, Tutor history,
          decks, templates, managed images, drafts, schedules, reviews, provider metadata, and settings.
        </p>
        <p className="tw:m-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">
          API keys and session-only credentials are never stored in SQLite and are never included
          in an ostudy backup.
        </p>
        {transferLabel ? <InlineProgress>{transferLabel}</InlineProgress> : null}
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
