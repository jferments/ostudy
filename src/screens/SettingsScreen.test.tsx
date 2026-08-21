import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiIpc from "../ipc/aiProfiles";
import * as backupIpc from "../ipc/backup";
import * as settingsIpc from "../ipc/settings";
import * as platformFiles from "../platform/files";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => ({
    draftId: "d".repeat(32),
    assetDraftId: "7".repeat(32),
    revision: 1,
    status: "saved",
    errorMessage: null,
    recovered: false,
    hasUnsavedLocalChanges: false,
    flush: vi.fn().mockResolvedValue(undefined),
    discard: vi.fn().mockResolvedValue(undefined),
    prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),
    acceptCanonicalCommit: vi.fn(),
    commit: vi.fn().mockResolvedValue(undefined),
    restart: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("../platform/files", () => ({
  selectExternalImport: vi.fn(),
  publishExport: vi.fn(),
}));
vi.mock("../ipc/backup", () => ({
  prepareLibraryBackup: vi.fn(),
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
  return {
    ...original,
    getErrorMessage: (error: unknown) =>
      error instanceof Error ? error.message : "Unexpected error",
  };
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
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue(null);
    vi.mocked(platformFiles.publishExport).mockResolvedValue({
      preparedExportId: "2".repeat(32),
      displayName: "export",
      destinationKind: "desktop_file",
      completedAt: Date.now(),
    });
    vi.mocked(settingsIpc.getAppInfo).mockResolvedValue({
      appVersion: "0.1.0",
      dataDirectory: "/tmp/ostudy/data",
      databasePath: "/tmp/ostudy/data/library.sqlite3",
      imageDirectory: "/tmp/ostudy/data/assets/images",
      databaseSchemaVersion: 16,
    });
    vi.mocked(settingsIpc.saveSettings).mockImplementation(async (input) => input);
    vi.mocked(backupIpc.prepareLibraryBackup).mockResolvedValue({
      preparedExport: {
        id: "2".repeat(32),
        purpose: "library_backup",
        suggestedFilename: "library.ostudybackup",
        mediaType: "application/vnd.ostudy.backup",
        byteSize: 128,
        sha256: "0".repeat(64),
        expiresAt: Date.now() + 60_000,
      },
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

  it("puts Appearance first and exposes the renamed Backup/Sync tab", () => {
    renderSettings();
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent)).toEqual([
      "Appearance",
      "Backup/Sync",
      "AI/LLM",
      "Review/Scheduling",
      "Diagnostics",
    ]);
    expect(tabs[0].getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tabpanel").textContent).toContain("Typography");
  });

  it("keeps storage information at the bottom of Backup/Sync", async () => {
    renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "Backup/Sync" }));
    expect(await screen.findByText("/tmp/ostudy/data")).toBeTruthy();
    const panel = screen.getByRole("tabpanel");
    const dataSafety = screen.getByRole("heading", { name: "Data safety" });
    const deviceSync = screen.getByRole("heading", { name: "Device synchronization" });
    const storage = screen.getByRole("heading", { name: "Storage" });
    expect(panel.contains(storage)).toBe(true);
    expect(
      dataSafety.compareDocumentPosition(deviceSync) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      deviceSync.compareDocumentPosition(storage) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("previews and saves appearance preferences without touching AI profile state", async () => {
    const { onPreviewSettings, onSavedSettings, onDirtyChange, showToast } = renderSettings();
    fireEvent.click(screen.getByRole("radio", { name: /Light/u }));

    await waitFor(() => {
      expect(onPreviewSettings).toHaveBeenLastCalledWith(expect.objectContaining({ theme: "light" }));
      expect(onDirtyChange).toHaveBeenLastCalledWith(true);
    });
    fireEvent.click(screen.getByRole("button", { name: "Save preferences" }));
    await waitFor(() => {
      expect(settingsIpc.saveSettings).toHaveBeenCalledWith(
        expect.objectContaining({ theme: "light" }),
      );
      expect(onSavedSettings).toHaveBeenCalledWith(expect.objectContaining({ theme: "light" }));
      expect(showToast).toHaveBeenCalledWith("success", "Settings saved.");
    });
  });

  it("keeps review limits in the dedicated scheduling tab and leaves shuffle on Practice", async () => {
    const { onPreviewSettings } = renderSettings();
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

  it("uses one Add model action and selects the provider inside the simple setup form", async () => {
    renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "AI/LLM" }));
    expect(await screen.findByRole("heading", { name: "AI models" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add model" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Add llama\.cpp/u })).toBeNull();
    expect(screen.queryByRole("button", { name: /Add DeepSeek/u })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Planned providers" })).toBeNull();
    expect(screen.queryByRole("option", { name: /planned/u })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Default task profile" })).toBeNull();
    expect(screen.getAllByRole("combobox", { name: "Flashcard generation" })).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "Add model" }));
    const provider = await screen.findByRole("combobox", { name: "Provider" });
    expect((provider as HTMLSelectElement).value).toBe("llama_cpp");
    fireEvent.change(provider, { target: { value: "deepseek" } });
    await waitFor(() => expect((provider as HTMLSelectElement).value).toBe("deepseek"));
    expect(await screen.findByLabelText("API key")).toBeTruthy();
    expect(screen.queryByText("Bearer token")).toBeNull();
    expect(screen.queryByText("Enable profile")).toBeNull();
  });

  it("backs up provider metadata while explicitly excluding credentials", async () => {
    const { showToast } = renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "Backup/Sync" }));
    await screen.findByText("/tmp/ostudy/data");
    fireEvent.click(screen.getByRole("button", { name: "Back up library" }));

    await waitFor(() => {
      expect(backupIpc.prepareLibraryBackup).toHaveBeenCalled();
      expect(platformFiles.publishExport).toHaveBeenCalledWith("2".repeat(32));
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Backed up 3 decks, 2 templates, and 41 review events.",
      );
    });
    expect(screen.getByText(/API keys and session-only credentials are never stored/u)).toBeTruthy();
  });

  it("prepares restore only after shared destructive confirmation", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({
      id: "1".repeat(32),
      purpose: "library_backup",
      detectedKind: "ostudybackup",
      displayName: "library.ostudybackup",
      mediaType: "application/octet-stream",
      byteSize: 128,
      sha256: "0".repeat(64),
      expiresAt: Date.now() + 60_000,
    });
    const { showToast } = renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "Backup/Sync" }));
    await screen.findByText("/tmp/ostudy/data");
    fireEvent.click(screen.getByRole("button", { name: "Restore backup" }));
    expect(
      await screen.findByRole("alertdialog", { name: "Restore this complete library?" }),
    ).toBeTruthy();
    expect(backupIpc.prepareLibraryRestore).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Prepare restore" }));
    await waitFor(() => {
      expect(backupIpc.prepareLibraryRestore).toHaveBeenCalledWith("1".repeat(32));
      expect(showToast).toHaveBeenCalledWith(
        "info",
        "Restore prepared for 3 decks. Close and reopen ostudy to apply it.",
      );
    });
  });
});
