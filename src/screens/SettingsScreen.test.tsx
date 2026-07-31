import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("../ipc", () => ({
  getAppInfo: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  saveSettings: vi.fn(),
}));

const settings: ipc.AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  newCardsPerDay: 20,
  reviewsPerDay: 200,
  strugglingCardThreshold: 8,
  autoSuspendStrugglingCards: false,
  buryRelatedCards: true,
};

describe("SettingsScreen", () => {
  beforeEach(() => {
    vi.mocked(ipc.getAppInfo).mockResolvedValue({
      appVersion: "0.1.0",
      dataDirectory: "/tmp/ostudy/data",
      databasePath: "/tmp/ostudy/data/library.sqlite3",
      imageDirectory: "/tmp/ostudy/data/assets/images",
      databaseSchemaVersion: 4,
    });
    vi.mocked(ipc.saveSettings).mockImplementation(async (input) => input);
  });

  it("previews and saves a theme change through the typed IPC wrapper", async () => {
    const onPreviewSettings = vi.fn();
    const onSavedSettings = vi.fn();
    const onDirtyChange = vi.fn();
    const showToast = vi.fn();

    render(
      <SettingsScreen
        settings={settings}
        onPreviewSettings={onPreviewSettings}
        onSavedSettings={onSavedSettings}
        onDirtyChange={onDirtyChange}
        showToast={showToast}
      />,
    );

    await screen.findByText("/tmp/ostudy/data");
    fireEvent.click(screen.getByRole("radio", { name: "Synthwave" }));

    await waitFor(() => {
      expect(onPreviewSettings).toHaveBeenLastCalledWith(
        expect.objectContaining({ theme: "synthwave" }),
      );
      expect(onDirtyChange).toHaveBeenLastCalledWith(true);
    });

    fireEvent.click(screen.getByRole("button", { name: "Save settings" }));

    await waitFor(() => {
      expect(ipc.saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({ theme: "synthwave" }),
      );
      expect(onSavedSettings).toHaveBeenCalledWith(
        expect.objectContaining({ theme: "synthwave" }),
      );
      expect(showToast).toHaveBeenCalledWith("success", "Settings saved.");
    });
  });
});
