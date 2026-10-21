import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../test/factories";
import * as aiIpc from "../ipc/aiProfiles";
import * as backupIpc from "../ipc/backup";
import * as settingsIpc from "../ipc/settings";
import * as platformFiles from "../platform/files";
import { dispatchPlatformBack } from "../platform/backNavigation";
import { refreshLibraryView } from "../platform/libraryView";
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
const layout = vi.hoisted(() => ({ mode: "wide" as "wide" | "compact" }));
vi.mock("../ui/layout/LayoutModeProvider", async (importOriginal) => ({
  ...await importOriginal<typeof import("../ui/layout/LayoutModeProvider")>(),
  useLayoutMode: () => ({ layoutMode: layout.mode, inputMode: "fine" }),
}));
vi.mock("../platform/libraryView", () => ({ refreshLibraryView: vi.fn() }));
vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn(), publishExport: vi.fn() }));
vi.mock("../ipc/backup", () => ({
  prepareLibraryBackup: vi.fn(),
  prepareLibraryRestore: vi.fn(),
}));
vi.mock("../ipc/settings", () => ({ getAppInfo: vi.fn(), saveSettings: vi.fn() }));
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
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: false,
  confirmDeletions: true,
  practiceShuffle: true,
  displayQuestionOnAnswer: false,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
  newDayStartHour: 4,
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
    layout.mode = "wide";
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
      databaseSchemaVersion: 13,
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
      operationId: "1".repeat(32),
      cardCount: 189,
      documentCount: 1,
      completed: true,
      restartRequired: false,
    });
    vi.mocked(aiIpc.getAiProviderCatalog).mockResolvedValue(providerCatalog());
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
    vi.mocked(aiIpc.getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null });
    vi.mocked(aiIpc.getAiProviderProbeResult).mockResolvedValue(null);
  });

  it("lists the sections in a pane with Appearance open first", () => {
    renderSettings();
    expect(screen.getByRole("tablist", { name: "Settings sections" }).getAttribute("aria-orientation")).toBe("vertical");
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent)).toEqual([
      "Appearance",
      "Review",
      "Card templates",
      "AI",
      "Backup and sync",
      "Diagnostics",
    ]);
    expect(required(tabs[0], "Appearance tab").getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("heading", { level: 2, name: "Appearance" })).toBeTruthy();
    expect(screen.getByRole("tabpanel").textContent).toContain("Interface font");
    expect(screen.queryByRole("region", { name: "Unsaved changes" })).toBeNull();
  });

  it("keeps Backup and sync focused on backup and device synchronization without a storage help tile", async () => {
    renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "Backup and sync" }));
    const panel = screen.getByRole("tabpanel");
    expect(within(panel).getByRole("heading", { name: "Backup" })).toBeTruthy();
    expect(within(panel).getByRole("heading", { name: "Devices" })).toBeTruthy();
    expect(within(panel).queryByRole("heading", { name: "Storage" })).toBeNull();
    expect(within(panel).queryByRole("button", { name: "View storage details" })).toBeNull();
  });

  it("previews and saves appearance preferences without touching AI provider state", async () => {
    const { onPreviewSettings, onSavedSettings, onDirtyChange, showToast } = renderSettings();
    fireEvent.click(screen.getByRole("radio", { name: /Light/u }));
    await waitFor(() => {
      expect(onPreviewSettings).toHaveBeenLastCalledWith(expect.objectContaining({ theme: "light" }));
      expect(onDirtyChange).toHaveBeenLastCalledWith(true);
    });
    expect(screen.getByRole("region", { name: "Unsaved changes" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(settingsIpc.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ theme: "light" }));
      expect(onSavedSettings).toHaveBeenCalledWith(expect.objectContaining({ theme: "light" }));
      expect(showToast).toHaveBeenCalledWith("success", "Settings saved.");
    });
  });

  it("restores Plex typography while leaving non-appearance settings unchanged", async () => {
    const { onPreviewSettings } = renderSettings();
    fireEvent.click(screen.getByRole("button", { name: "Restore defaults" }));
    await waitFor(() => {
      expect(onPreviewSettings).toHaveBeenLastCalledWith({
        ...settings,
        theme: "dark",
        uiFont: "plex",
        cardFont: "plex",
        uiScale: 100,
        cardBaseSize: 18,
        motionPreference: "system",
        materialPreference: "automatic",
        decorativeArtwork: true,
        celebrationsEnabled: true,
        tutorialsEnabled: false,
      });
    });
  });

  it("keeps review limits in the dedicated scheduling tab and leaves shuffle on Practice", async () => {
    const { onPreviewSettings } = renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "Review" }));
    const schedulingPanel = screen.getByRole("tabpanel", { name: "Review" });
    const restoreDefaults = screen.getByRole("button", { name: "Restore defaults" });
    fireEvent.change(screen.getByRole("spinbutton", { name: "New cards per day" }), {
      target: { value: "12" },
    });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Reviews per day" }), {
      target: { value: "80" },
    });
    const newDayStart = screen.getByRole("combobox", { name: "New day starts at" }) as HTMLSelectElement;
    expect(schedulingPanel.contains(newDayStart)).toBe(true);
    expect(newDayStart.selectedOptions[0]?.textContent).toBe("4 a.m.");
    fireEvent.change(newDayStart, { target: { value: "0" } });
    await waitFor(() => {
      expect(onPreviewSettings).toHaveBeenLastCalledWith(
        expect.objectContaining({ dailyNewLimit: 12, dailyReviewLimit: 80, newDayStartHour: 0 }),
      );
    });
    expect(newDayStart.selectedOptions[0]?.textContent).toBe("12 a.m.");
    fireEvent.click(restoreDefaults);
    await waitFor(() => {
      expect(onPreviewSettings).toHaveBeenLastCalledWith(
        expect.objectContaining({
          desiredRetention: 0.9,
          dailyNewLimit: 20,
          dailyReviewLimit: 200,
          newDayStartHour: 4,
        }),
      );
    });
    expect(screen.queryByRole("checkbox", { name: /shuffle/iu })).toBeNull();
    expect(screen.queryByRole("switch", { name: /shuffle/iu })).toBeNull();
  });

  it("saves the hour a new study day starts", async () => {
    const { onSavedSettings } = renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "Review" }));
    fireEvent.change(screen.getByRole("combobox", { name: "New day starts at" }), {
      target: { value: "6" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
      expect(settingsIpc.saveSettings).toHaveBeenCalledWith({ ...settings, newDayStartHour: 6 });
      expect(onSavedSettings).toHaveBeenCalledWith(expect.objectContaining({ newDayStartHour: 6 }));
    });
  });

  it("separates services from the default model for each task", async () => {
    renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "AI" }));
    expect(await screen.findByRole("heading", { name: "Default for each task" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Services" })).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "Cards and guides service" })).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "Cards and guides model" })).toBeTruthy();
    expect(screen.getByRole("combobox", { name: "Tutor service" })).toBeTruthy();
    // No service can search yet, so Search stays off and offers OpenRouter.
    expect(screen.queryByRole("combobox", { name: "Search service" })).toBeNull();
    expect(screen.getAllByRole("button", { name: "Add OpenRouter" }).length).toBeGreaterThan(0);
    expect(screen.queryByRole("heading", { name: "Question generation" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Add a service" }));
    const sheet = await screen.findByRole("dialog", { name: "Add a service" });
    const llama = within(sheet).getByRole("radio", { name: /^llama\.cpp/u });
    expect(llama.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(within(sheet).getByRole("radio", { name: /^DeepSeek/u }));
    expect(await within(sheet).findByLabelText("API key")).toBeTruthy();
    expect(within(sheet).queryByRole("combobox", { name: "Model" })).toBeNull();
    expect(within(sheet).queryByRole("combobox", { name: "Reasoning effort" })).toBeNull();
  });

  it("backs up provider metadata while explicitly excluding credentials", async () => {
    const { showToast } = renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "Backup and sync" }));
    fireEvent.click(await screen.findByRole("button", { name: "Back up library" }));
    await waitFor(() => {
      expect(backupIpc.prepareLibraryBackup).toHaveBeenCalled();
      expect(platformFiles.publishExport).toHaveBeenCalledWith("2".repeat(32));
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Backed up 3 decks, 2 templates, and 41 review events.",
      );
    });
    expect(screen.getByRole("button", { name: "Help with backups" })).toBeTruthy();
  });

  it("restores immediately only after shared destructive confirmation", async () => {
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
    fireEvent.click(screen.getByRole("tab", { name: "Backup and sync" }));
    fireEvent.click(await screen.findByRole("button", { name: "Restore backup" }));
    expect(
      await screen.findByRole("alertdialog", { name: "Restore this complete library?" }),
    ).toBeTruthy();
    expect(backupIpc.prepareLibraryRestore).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Restore library" }));
    await waitFor(() => {
      expect(backupIpc.prepareLibraryRestore).toHaveBeenCalledWith("1".repeat(32));
      expect(refreshLibraryView).toHaveBeenCalledOnce();
      expect(screen.queryByText(/Close and reopen ostudy/u)).toBeNull();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Restored 3 decks, 189 cards, and 1 document. Your library is ready.",
      );
    });
  });
  it("discards unsaved preferences back to the saved ones", async () => {
    const { onPreviewSettings, onDirtyChange } = renderSettings();
    fireEvent.click(screen.getByRole("radio", { name: /Light/u }));
    await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(true));
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    await waitFor(() => {
      expect(onPreviewSettings).toHaveBeenLastCalledWith(settings);
      expect(onDirtyChange).toHaveBeenLastCalledWith(false);
    });
    expect(screen.queryByRole("region", { name: "Unsaved changes" })).toBeNull();
    expect(settingsIpc.saveSettings).not.toHaveBeenCalled();
  });

  it("opens on a list of sections on a phone and steps back to it", () => {
    layout.mode = "compact";
    renderSettings();
    expect(screen.getByRole("heading", { level: 1, name: "Settings" })).toBeTruthy();
    expect(screen.queryByRole("tablist")).toBeNull();
    const sections = within(screen.getByRole("navigation", { name: "Settings sections" })).getAllByRole("button");
    expect(sections.map((button) => button.textContent)).toEqual([
      "Appearance",
      "Review",
      "Card templates",
      "AI",
      "Backup and sync",
      "Diagnostics",
    ]);

    fireEvent.click(screen.getByRole("button", { name: "Review" }));
    expect(screen.getByRole("heading", { level: 1, name: "Review" })).toBeTruthy();
    expect(screen.getByRole("spinbutton", { name: "New cards per day" })).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Settings sections" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Back to Settings" }));
    expect(screen.getByRole("heading", { level: 1, name: "Settings" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Appearance" }));
    expect(screen.getByRole("heading", { level: 1, name: "Appearance" })).toBeTruthy();
    act(() => { expect(dispatchPlatformBack()).toBe(true); });
    expect(screen.getByRole("navigation", { name: "Settings sections" })).toBeTruthy();
    expect(dispatchPlatformBack()).toBe(false);
  });
});
