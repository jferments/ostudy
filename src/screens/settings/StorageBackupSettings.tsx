import { Panel } from "../../ui/primitives/Panel";
import { useRef, useState } from "react";

import { restoredLibraryMessage } from "../../components/libraryRestoreResult";
import { refreshLibraryView } from "../../platform/libraryView";
import { releaseStagedExternalImport } from "../../ipc/externalFiles";
import type { ShowToast } from "../../components/Toast";
import { prepareLibraryBackup, prepareLibraryRestore } from "../../ipc/backup";
import { createOperationId, getErrorMessage } from "../../ipc/common";
import type { AppSettings } from "../../ipc/settings";
import { publishExport, selectExternalImport, shareExport } from "../../platform/files";
import { usePlatformCapabilities } from "../../platform/PlatformProvider";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { DeviceSyncSettings } from "./DeviceSyncSettings";
import { SwitchRow } from "./SettingsControls";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";

interface StorageBackupSettingsProps {
  draft: AppSettings;
  update: (patch: Partial<AppSettings>) => void;
  showToast: ShowToast;
}

export function StorageBackupSettings({ draft, update, showToast }: StorageBackupSettingsProps) {
  const [isTransferringLibrary, setIsTransferringLibrary] = useState(false);
  const transferActive = useRef(false);
  const [transferLabel, setTransferLabel] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const confirm = useConfirm();
  const capabilities = usePlatformCapabilities();
  const canShare = capabilities.native?.services.systemShare.state === "available";

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

      <Panel className="tw:min-w-0 tw:self-start" title="Backup"
        actions={<HelpTrigger topicId="settings.backup" label="Help with backups" />}>
        <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-3">
          <Button
            type="button"
            variant="primary"
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
        {transferLabel ? <InlineProgress>{transferLabel}</InlineProgress> : null}
        <div className="tw:border-0 tw:border-t tw:border-solid tw:border-border/70">
          <SwitchRow
            label="Confirm deletions"
            checked={draft.confirmDeletions}
            onChange={(confirmDeletions) => update({ confirmDeletions })}
          />
        </div>
      </Panel>

      <DeviceSyncSettings showToast={showToast} />

    </div>
  );
}
