import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as backupIpc from "../ipc/backup";
import * as settingsIpc from "../ipc/settings";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../ipc/backup", () => ({
  exportLibraryBackup: vi.fn(),
  prepareLibraryRestore: vi.fn(),
}));
vi.mock("../ipc/common", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../ipc/settings", () => ({
  getAppInfo: vi.fn(),
  saveSettings: vi.fn(),
}));

const settings: settingsIpc.AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

function renderSettings(showToast = vi.fn()) {
  const onPreviewSettings = vi.fn();
  const onSavedSettings = vi.fn();
  const onDirtyChange = vi.fn();
  render(
    <SettingsScreen
      settings={settings}
      onPreviewSettings={onPreviewSettings}
      onSavedSettings={onSavedSettings}
      onDirtyChange={onDirtyChange}
      showToast={showToast}
    />,
  );
  return { onPreviewSettings, onSavedSettings, onDirtyChange, showToast };
}

describe("SettingsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(openDialog).mockResolvedValue(null);
    vi.mocked(saveDialog).mockResolvedValue(null);
    vi.mocked(settingsIpc.getAppInfo).mockResolvedValue({
      appVersion: "0.1.0",
      dataDirectory: "/tmp/ostudy/data",
      databasePath: "/tmp/ostudy/data/library.sqlite3",
      imageDirectory: "/tmp/ostudy/data/assets/images",
      databaseSchemaVersion: 14,
    });
    vi.mocked(settingsIpc.saveSettings).mockImplementation(async (input) => input);
    vi.mocked(backupIpc.exportLibraryBackup).mockResolvedValue({
      destinationPath: "/tmp/library.ostudybackup",
      deckCount: 3,
      templateCount: 2,
      reviewCount: 41,
      assetCount: 4,
    });
    vi.mocked(backupIpc.prepareLibraryRestore).mockResolvedValue({
      deckCount: 3,
      templateCount: 2,
      reviewCount: 41,
      assetCount: 4,
      restartRequired: true,
    });
  });

  it("previews and saves exact daily new and review limits", async () => {
    const { onPreviewSettings, onSavedSettings, showToast } = renderSettings();

    await screen.findByText("/tmp/ostudy/data");
    fireEvent.change(screen.getByRole("spinbutton", { name: "New cards per day" }), {
      target: { value: "12" },
    });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Reviews per day" }), {
      target: { value: "80" },
    });

    await waitFor(() =>
      expect(onPreviewSettings).toHaveBeenLastCalledWith(
        expect.objectContaining({ dailyNewLimit: 12, dailyReviewLimit: 80 }),
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Save settings" }));
    await waitFor(() => {
      expect(settingsIpc.saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({ dailyNewLimit: 12, dailyReviewLimit: 80 }),
      );
      expect(onSavedSettings).toHaveBeenCalledWith(
        expect.objectContaining({ dailyNewLimit: 12, dailyReviewLimit: 80 }),
      );
      expect(showToast).toHaveBeenCalledWith("success", "Settings saved.");
    });
  });

  it("exports one complete backup with deck, template, and review counts", async () => {
    vi.mocked(saveDialog).mockResolvedValue("/tmp/library.ostudybackup");
    const { showToast } = renderSettings();
    await screen.findByText("/tmp/ostudy/data");

    fireEvent.click(screen.getByRole("button", { name: "Back up library" }));

    await waitFor(() => {
      expect(backupIpc.exportLibraryBackup).toHaveBeenCalledWith(
        "/tmp/library.ostudybackup",
      );
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Backed up 3 decks, 2 templates, and 41 review events.",
      );
    });
  });

  it("prepares a validated restore and requires an application restart", async () => {
    vi.mocked(openDialog).mockResolvedValue("/tmp/library.ostudybackup");
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const { showToast } = renderSettings();
    await screen.findByText("/tmp/ostudy/data");

    fireEvent.click(screen.getByRole("button", { name: "Restore backup" }));

    await waitFor(() => {
      expect(backupIpc.prepareLibraryRestore).toHaveBeenCalledWith(
        "/tmp/library.ostudybackup",
      );
      expect(showToast).toHaveBeenCalledWith(
        "info",
        "Restore prepared for 3 decks. Close and reopen ostudy to apply it.",
      );
    });
    expect(
      screen.getByText(/Restore is prepared\. Close every ostudy window/u),
    ).toBeTruthy();
  });
});