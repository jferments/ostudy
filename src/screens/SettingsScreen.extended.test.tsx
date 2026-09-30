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

const seededOpenRouterDefaults: aiIpc.AiTaskDefaults = {
  flashcardGenerationProfileId: profileId,
  flashcardGenerationModelId: "deepseek/deepseek-v4.1-flash",
  flashcardGenerationReasoningEffort: "none",
  tutorProfileId: profileId,
  tutorModelId: "deepseek/deepseek-v4.1-flash",
  tutorReasoningEffort: "low",
  embeddingsProfileId: profileId,
  embeddingsModelId: "qwen/qwen3-embedding-4b",
  embeddingsReasoningEffort: "none",
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

async function readyAddProviderButton(): Promise<HTMLButtonElement> {
  const button = (await screen.findByRole("button", { name: "Add provider" })) as HTMLButtonElement;
  await waitFor(() => expect(button.disabled).toBe(false));
  return button;
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
      { id: "deepseek/deepseek-v4.1-flash", displayName: "DeepSeek V4.1 Flash", ownedBy: "deepseek" },
      { id: "anthropic/claude-sonnet", displayName: "Claude Sonnet", ownedBy: "anthropic" },
    ],
    retrievedAt: 2,
  });
  vi.mocked(embeddingIpc.listEmbeddingModels).mockResolvedValue([
    { id: "qwen/qwen3-embedding-4b", name: "Qwen3 Embedding 4B" },
  ]);
  vi.mocked(embeddingIpc.getEmbeddingOptions).mockResolvedValue({
    profileId,
    modelId: "qwen/qwen3-embedding-4b",
    dimensions: null,
    inputPolicy: "automatic",
    revision: 1,
  });
  vi.mocked(taskIpc.setAiTaskDefault).mockResolvedValue(emptyDefaults);
});

describe("SettingsScreen AI provider/model behavior", () => {
  it("connects the first OpenRouter key and reflects native-seeded task defaults without UI rewrite calls", async () => {
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
    vi.mocked(aiIpc.getAiTaskDefaults)
      .mockResolvedValueOnce(emptyDefaults)
      .mockResolvedValue(seededOpenRouterDefaults);
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.setAiProviderCredential).mockResolvedValue(connected);
    vi.mocked(aiIpc.listAiProviderProfiles)
      .mockResolvedValueOnce([])
      .mockResolvedValue([summary(connected)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(connected);

    renderSettings();
    await openAiTab();
    fireEvent.click(await readyAddProviderButton());
    const provider = (await screen.findByRole("combobox", {
      name: "Provider",
    })) as HTMLSelectElement;
    expect(provider.value).toBe("openrouter");
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
      expect(aiIpc.getAiTaskDefaults).toHaveBeenCalledTimes(2);
      expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(profileId, 2);
    });
    expect((apiKey as HTMLInputElement).value).toBe("");
    expect(screen.queryByText("sentinel-secret")).toBeNull();

    await waitFor(() => {
      expect(
        (screen.getByRole("combobox", {
          name: "Flashcard and study-guide generation provider",
        }) as HTMLSelectElement).value,
      ).toBe(profileId);
      expect(
        (screen.getByRole("combobox", {
          name: "Flashcard and study-guide generation model",
        }) as HTMLSelectElement).value,
      ).toBe("deepseek/deepseek-v4.1-flash");
      expect(
        (screen.getByRole("combobox", {
          name: "Flashcard and study-guide generation reasoning",
        }) as HTMLSelectElement).value,
      ).toBe("none");
      expect(
        (screen.getByRole("combobox", {
          name: "Interactive tutor model",
        }) as HTMLSelectElement).value,
      ).toBe("deepseek/deepseek-v4.1-flash");
      expect(
        (screen.getByRole("combobox", {
          name: "Interactive tutor reasoning",
        }) as HTMLSelectElement).value,
      ).toBe("low");
      expect(
        (screen.getByRole("combobox", {
          name: "Semantic embeddings model",
        }) as HTMLSelectElement).value,
      ).toBe("qwen/qwen3-embedding-4b");
    });
    expect(taskIpc.setAiTaskDefault).not.toHaveBeenCalled();
  });

  it("offers another same-provider connection under a unique default name", async () => {
    const first = profile();
    const second = profile({ id: secondId, displayName: "OpenRouter 2" });
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(first), summary(second)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(first);

    renderSettings();
    await openAiTab();
    fireEvent.click(await readyAddProviderButton());
    fireEvent.click(await screen.findByText("Advanced settings"));
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
    await screen.findByRole("button", { name: "Refresh model catalog" });
    expect(screen.queryByRole("combobox", { name: "Model" })).toBeNull();
    expect(screen.queryByRole("combobox", { name: "Reasoning effort" })).toBeNull();
    expect(screen.getByRole("button", { name: "Refresh model catalog" })).toBeTruthy();
  });
});
