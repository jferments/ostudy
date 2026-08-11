import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiIpc from "../ipc/aiProfiles";
import * as backupIpc from "../ipc/backup";
import * as settingsIpc from "../ipc/settings";
import * as platformFiles from "../platform/files";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("../platform/files", () => ({
  openFile: vi.fn(),
  saveFile: vi.fn(),
}));
vi.mock("../ipc/backup", () => ({
  exportLibraryBackup: vi.fn(),
  prepareLibraryRestore: vi.fn(),
}));
vi.mock("../ipc/settings", () => ({
  getAppInfo: vi.fn(),
  saveSettings: vi.fn(),
}));
vi.mock("../ipc/aiProfiles", () => ({
  getAiProviderCatalog: vi.fn(),
  listAiProviderProfiles: vi.fn(),
  getAiProviderProfile: vi.fn(),
  createAiProviderProfile: vi.fn(),
  updateAiProviderProfile: vi.fn(),
  deleteAiProviderProfile: vi.fn(),
  setAiProviderCredential: vi.fn(),
  clearAiProviderCredential: vi.fn(),
  getAiTaskDefaults: vi.fn(),
  setFlashcardGenerationProfile: vi.fn(),
  listAiProviderModels: vi.fn(),
  testAiProviderProfile: vi.fn(),
  getAiProviderProbeResult: vi.fn(),
}));
vi.mock("../ipc/common", async (importOriginal) => {
  const original = await importOriginal<typeof import("../ipc/common")>();
  return { ...original, getErrorMessage: (error: unknown) => error instanceof Error ? error.message : "Unexpected error" };
});

const settings: settingsIpc.AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  animationsEnabled: true,
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
    <OverlayProvider>
      <SettingsScreen
        settings={settings}
        onPreviewSettings={onPreviewSettings}
        onSavedSettings={onSavedSettings}
        onDirtyChange={onDirtyChange}
        showToast={showToast}
      />
    </OverlayProvider>,
  );
  return { onPreviewSettings, onSavedSettings, onDirtyChange, showToast };
}

function providerCatalog(): aiIpc.AiProviderCatalogEntry[] {
  return [
    {
      providerKind: "llama_cpp",
      displayName: "llama.cpp / Unsloth GGUF",
      description: "Connect to an existing llama.cpp server.",
      implementationStatus: "available",
      defaultBaseUrl: "http://127.0.0.1:8080/",
      baseUrlEditable: true,
      authenticationRequired: false,
      modelIdRequired: true,
      modelDiscoverySupported: true,
      chatTemplatePolicy: "server_model",
      defaultTimeoutMs: 60_000,
    },
    {
      providerKind: "deepseek",
      displayName: "DeepSeek",
      description: "Connect to DeepSeek's official cloud API.",
      implementationStatus: "available",
      defaultBaseUrl: "https://api.deepseek.com/",
      baseUrlEditable: false,
      authenticationRequired: true,
      modelIdRequired: true,
      modelDiscoverySupported: true,
      chatTemplatePolicy: "provider_managed",
      defaultTimeoutMs: 60_000,
    },
  ];
}

describe("SettingsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(platformFiles.openFile).mockResolvedValue(null);
    vi.mocked(platformFiles.saveFile).mockResolvedValue(null);
    vi.mocked(settingsIpc.getAppInfo).mockResolvedValue({
      appVersion: "0.1.0",
      dataDirectory: "/tmp/ostudy/data",
      databasePath: "/tmp/ostudy/data/library.sqlite3",
      imageDirectory: "/tmp/ostudy/data/assets/images",
      databaseSchemaVersion: 16,
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
    vi.mocked(aiIpc.getAiProviderCatalog).mockResolvedValue(providerCatalog());
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
    vi.mocked(aiIpc.getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null });
    vi.mocked(aiIpc.getAiProviderProbeResult).mockResolvedValue(null);
  });

  it("renders the four requested accessible settings tabs", async () => {
    renderSettings();
    expect(await screen.findByText("/tmp/ostudy/data")).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Storage/Backup" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Appearance" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "AI/LLM" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Review/Scheduling" })).toBeTruthy();
    expect(screen.getByRole("tabpanel").textContent).toContain("API keys and session-only credentials");
  });

  it("previews and saves appearance preferences without touching AI profile state", async () => {
    const { onPreviewSettings, onSavedSettings, onDirtyChange, showToast } = renderSettings();
    await screen.findByText("/tmp/ostudy/data");
    fireEvent.click(screen.getByRole("tab", { name: "Appearance" }));
    fireEvent.click(screen.getByRole("radio", { name: /Light/u }));

    await waitFor(() => {
      expect(onPreviewSettings).toHaveBeenLastCalledWith(expect.objectContaining({ theme: "light" }));
      expect(onDirtyChange).toHaveBeenLastCalledWith(true);
    });
    fireEvent.click(screen.getByRole("button", { name: "Save preferences" }));
    await waitFor(() => {
      expect(settingsIpc.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ theme: "light" }));
      expect(onSavedSettings).toHaveBeenCalledWith(expect.objectContaining({ theme: "light" }));
      expect(showToast).toHaveBeenCalledWith("success", "Settings saved.");
    });
  });

  it("keeps review limits in the dedicated scheduling tab and leaves shuffle on Practice", async () => {
    const { onPreviewSettings } = renderSettings();
    await screen.findByText("/tmp/ostudy/data");
    fireEvent.click(screen.getByRole("tab", { name: "Review/Scheduling" }));
    fireEvent.change(screen.getByRole("spinbutton", { name: "New cards per day" }), {
      target: { value: "12" },
    });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Reviews per day" }), {
      target: { value: "80" },
    });
    await waitFor(() => {
      expect(onPreviewSettings).toHaveBeenLastCalledWith(
        expect.objectContaining({ dailyNewLimit: 12, dailyReviewLimit: 80 }),
      );
    });
    expect(screen.queryByRole("checkbox", { name: /shuffle/iu })).toBeNull();
    expect(screen.getByText(/shuffling, are chosen on the Practice screen/u)).toBeTruthy();
  });

  it("backs up provider metadata while explicitly excluding credentials", async () => {
    vi.mocked(platformFiles.saveFile).mockResolvedValue("/tmp/library.ostudybackup");
    const { showToast } = renderSettings();
    await screen.findByText("/tmp/ostudy/data");
    fireEvent.click(screen.getByRole("button", { name: "Back up library" }));

    await waitFor(() => {
      expect(backupIpc.exportLibraryBackup).toHaveBeenCalledWith("/tmp/library.ostudybackup");
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Backed up 3 decks, 2 templates, and 41 review events.",
      );
    });
    expect(screen.getByText(/API keys and session-only credentials are never stored/u)).toBeTruthy();
  });

  it("prepares restore only after shared destructive confirmation", async () => {
    vi.mocked(platformFiles.openFile).mockResolvedValue("/tmp/library.ostudybackup");
    const { showToast } = renderSettings();
    await screen.findByText("/tmp/ostudy/data");
    fireEvent.click(screen.getByRole("button", { name: "Restore backup" }));
    expect(await screen.findByRole("alertdialog", { name: "Restore this complete library?" })).toBeTruthy();
    expect(backupIpc.prepareLibraryRestore).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Prepare restore" }));
    await waitFor(() => {
      expect(backupIpc.prepareLibraryRestore).toHaveBeenCalledWith("/tmp/library.ostudybackup");
      expect(showToast).toHaveBeenCalledWith(
        "info",
        "Restore prepared for 3 decks. Close and reopen ostudy to apply it.",
      );
    });
  });
});
