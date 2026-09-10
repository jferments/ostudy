import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiIpc from "../ipc/aiProfiles";
import * as taskIpc from "../ipc/aiTaskDefaults";
import * as embeddingIpc from "../ipc/embeddings";
import * as settingsIpc from "../ipc/settings";
import { makeSettings } from "../test/factories";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn(), publishExport: vi.fn() }));
vi.mock("../ipc/backup", () => ({ prepareLibraryBackup: vi.fn(), prepareLibraryRestore: vi.fn() }));
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
vi.mock("../ipc/aiTaskDefaults", () => ({ setAiTaskDefault: vi.fn() }));
vi.mock("../ipc/embeddings", () => ({
  getEmbeddingOptions: vi.fn(),
  listEmbeddingModels: vi.fn(),
  saveEmbeddingOptions: vi.fn(),
  testEmbeddingProfile: vi.fn(),
}));
vi.mock("../ipc/common", async (importOriginal) => {
  const original = await importOriginal<typeof import("../ipc/common")>();
  return {
    ...original,
    getErrorMessage: (error: unknown) =>
      error instanceof Error ? error.message : "Unexpected error",
  };
});

const profileId = "a".repeat(32);
const secondId = "b".repeat(32);
const emptyDefaults: aiIpc.AiTaskDefaults = {
  flashcardGenerationProfileId: null,
  flashcardGenerationModelId: null,
  flashcardGenerationReasoningEffort: null,
  tutorProfileId: null,
  tutorModelId: null,
  tutorReasoningEffort: null,
  embeddingsProfileId: null,
  embeddingsModelId: null,
  embeddingsReasoningEffort: null,
};

function catalog(): aiIpc.AiProviderCatalogEntry[] {
  return [
    {
      providerKind: "openrouter",
      displayName: "OpenRouter",
      description: "Use OpenRouter's account-visible model catalog.",
      implementationStatus: "available",
      defaultBaseUrl: "https://openrouter.ai/api/v1/",
      baseUrlEditable: false,
      authenticationRequired: true,
      modelIdRequired: true,
      modelDiscoverySupported: true,
      chatTemplatePolicy: "provider_managed",
      defaultTimeoutMs: 60_000,
    },
  ];
}

function profile(
  overrides: Partial<aiIpc.AiProviderProfileDetail> = {},
): aiIpc.AiProviderProfileDetail {
  return {
    id: profileId,
    displayName: "OpenRouter",
    providerKind: "openrouter",
    providerLabel: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1/",
    baseUrlEditable: false,
    modelId: "",
    reasoningEffort: "automatic",
    authMode: "bearer",
    enabled: true,
    allowInsecureHttp: false,
    requireZeroDataRetention: false,
    requestTimeoutMs: 60_000,
    credentialEpoch: 1,
    networkScope: "cloud",
    credentialStatus: {
      status: "saved_securely",
      authenticationRequired: true,
      securePersistenceAvailable: true,
      sessionOnlyAvailable: true,
    },
    configurationStatus: "ready",
    revision: 2,
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}

function summary(detail: aiIpc.AiProviderProfileDetail): aiIpc.AiProviderProfileSummary {
  return {
    id: detail.id,
    displayName: detail.displayName,
    providerKind: detail.providerKind,
    providerLabel: detail.providerLabel,
    modelId: detail.modelId,
    reasoningEffort: detail.reasoningEffort,
    enabled: detail.enabled,
    networkScope: detail.networkScope,
    credentialStatus: detail.credentialStatus,
    configurationStatus: detail.configurationStatus,
    revision: detail.revision,
    updatedAt: detail.updatedAt,
  };
}

function renderSettings(showToast = vi.fn()) {
  render(
    <OverlayProvider>
      <SettingsScreen
        settings={makeSettings()}
        onPreviewSettings={vi.fn()}
        onSavedSettings={vi.fn()}
        onDirtyChange={vi.fn()}
        showToast={showToast}
      />
    </OverlayProvider>,
  );
  return { showToast };
}

async function openAiTab(): Promise<void> {
  await waitFor(() => expect(settingsIpc.getAppInfo).toHaveBeenCalledOnce());
  fireEvent.click(screen.getByRole("tab", { name: "AI/LLM" }));
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(settingsIpc.getAppInfo).mockResolvedValue({
    appVersion: "0.1.0",
    dataDirectory: "/tmp/ostudy/data",
    databasePath: "/tmp/ostudy/data/library.sqlite3",
    imageDirectory: "/tmp/ostudy/data/assets/images",
    databaseSchemaVersion: 13,
  });
  vi.mocked(settingsIpc.saveSettings).mockImplementation(async (value) => value);
  vi.mocked(aiIpc.getAiProviderCatalog).mockResolvedValue(catalog());
  vi.mocked(aiIpc.getAiTaskDefaults).mockResolvedValue(emptyDefaults);
  vi.mocked(aiIpc.getAiProviderProbeResult).mockResolvedValue(null);
  vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
  vi.mocked(aiIpc.listAiProviderModels).mockResolvedValue({
    profileId,
    profileRevision: 2,
    providerKind: "openrouter",
    models: [
      { id: "deepseek/deepseek-v4-flash", displayName: "DeepSeek V4 Flash", ownedBy: "deepseek" },
      { id: "anthropic/claude-sonnet", displayName: "Claude Sonnet", ownedBy: "anthropic" },
    ],
    retrievedAt: 2,
  });
  vi.mocked(embeddingIpc.listEmbeddingModels).mockResolvedValue([
    { id: "qwen/qwen3-embedding", name: "Qwen3 Embedding" },
  ]);
  vi.mocked(embeddingIpc.getEmbeddingOptions).mockResolvedValue({
    profileId,
    modelId: "qwen/qwen3-embedding",
    dimensions: null,
    inputPolicy: "automatic",
    revision: 1,
  });
  vi.mocked(taskIpc.setAiTaskDefault).mockResolvedValue(emptyDefaults);
});

describe("SettingsScreen AI provider/model behavior", () => {
  it("connects one OpenRouter API key and assigns different models to all three AI tasks", async () => {
    const created = profile({
      credentialEpoch: 0,
      credentialStatus: {
        status: "missing",
        authenticationRequired: true,
        securePersistenceAvailable: true,
        sessionOnlyAvailable: true,
      },
      configurationStatus: "incomplete",
      revision: 1,
    });
    const connected = profile();
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.setAiProviderCredential).mockResolvedValue(connected);
    vi.mocked(aiIpc.listAiProviderProfiles)
      .mockResolvedValueOnce([])
      .mockResolvedValue([summary(connected)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(connected);

    renderSettings();
    await openAiTab();
    fireEvent.click(screen.getByRole("button", { name: "Add provider" }));
    expect((screen.getByRole("combobox", { name: "Provider" }) as HTMLSelectElement).value).toBe(
      "openrouter",
    );
    const apiKey = await screen.findByLabelText("API key");
    fireEvent.change(apiKey, { target: { value: "sentinel-secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() => {
      expect(aiIpc.createAiProviderProfile).toHaveBeenCalledWith(
        expect.objectContaining({ providerKind: "openrouter", modelId: "", authMode: "bearer" }),
      );
      expect(aiIpc.setAiProviderCredential).toHaveBeenCalledWith(
        profileId,
        1,
        "sentinel-secret",
        "secure",
      );
      expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(profileId, 2);
    });
    expect((apiKey as HTMLInputElement).value).toBe("");
    expect(screen.queryByText("sentinel-secret")).toBeNull();

    for (const [label, model] of [
      ["Flashcard and study-guide generation", "deepseek/deepseek-v4-flash"],
      ["Interactive tutor", "anthropic/claude-sonnet"],
    ] as const) {
      const providerSelect = screen.getByRole("combobox", { name: `${label} provider` });
      fireEvent.change(providerSelect, { target: { value: profileId } });
      const modelSelect = screen.getByRole("combobox", { name: `${label} model` });
      await waitFor(() => expect((modelSelect as HTMLSelectElement).disabled).toBe(false));
      fireEvent.change(modelSelect, { target: { value: model } });
    }

    const embeddingProvider = screen.getByRole("combobox", {
      name: "Semantic embeddings provider",
    });
    fireEvent.change(embeddingProvider, { target: { value: profileId } });
    const embeddingModel = screen.getByRole("combobox", { name: "Semantic embeddings model" });
    await waitFor(() => expect((embeddingModel as HTMLSelectElement).disabled).toBe(false));
    fireEvent.change(embeddingModel, { target: { value: "qwen/qwen3-embedding" } });

    await waitFor(() => expect(taskIpc.setAiTaskDefault).toHaveBeenCalledTimes(3));
    const assignments = vi.mocked(taskIpc.setAiTaskDefault).mock.calls.map(([input]) => input);
    expect(assignments).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          taskKind: "flashcard_generation",
          profileId,
          modelId: "deepseek/deepseek-v4-flash",
        }),
        expect.objectContaining({
          taskKind: "tutor",
          profileId,
          modelId: "anthropic/claude-sonnet",
        }),
        expect.objectContaining({
          taskKind: "embeddings",
          profileId,
          modelId: "qwen/qwen3-embedding",
          reasoningEffort: "none",
        }),
      ]),
    );
  });

  it("offers another same-provider connection under a unique default name", async () => {
    const first = profile();
    const second = profile({ id: secondId, displayName: "OpenRouter 2" });
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(first), summary(second)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(first);

    renderSettings();
    await openAiTab();
    fireEvent.click(screen.getByRole("button", { name: "Add provider" }));
    fireEvent.click(screen.getByText("Advanced settings"));
    expect((screen.getByRole("textbox", { name: "Profile name" }) as HTMLInputElement).value).toBe(
      "OpenRouter 3",
    );
  });

  it("keeps model and reasoning controls out of the provider connection editor", async () => {
    const connected = profile();
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(connected)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(connected);

    renderSettings();
    await openAiTab();
    await screen.findByDisplayValue("OpenRouter");
    expect(screen.queryByRole("combobox", { name: "Model" })).toBeNull();
    expect(screen.queryByRole("combobox", { name: "Reasoning effort" })).toBeNull();
    expect(screen.getByRole("button", { name: "Refresh model catalog" })).toBeTruthy();
  });
});
