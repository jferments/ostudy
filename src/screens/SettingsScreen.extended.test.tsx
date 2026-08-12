import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiIpc from "../ipc/aiProfiles";
import * as settingsIpc from "../ipc/settings";
import { makeSettings } from "../test/factories";
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
    commit: vi.fn().mockResolvedValue(undefined),
    restart: vi.fn().mockResolvedValue(undefined),
  }),
}));

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
vi.mock("../ipc/common", async (importOriginal) => {
  const original = await importOriginal<typeof import("../ipc/common")>();
  return {
    ...original,
    getErrorMessage: (error: unknown) =>
      error instanceof Error ? error.message : "Unexpected error",
  };
});

const llamaId = "a".repeat(32);
const deepseekId = "b".repeat(32);

function catalog(): aiIpc.AiProviderCatalogEntry[] {
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
    {
      providerKind: "openai",
      displayName: "OpenAI",
      description: "OpenAI support is planned.",
      implementationStatus: "planned",
      defaultBaseUrl: "https://api.openai.com/",
      baseUrlEditable: false,
      authenticationRequired: true,
      modelIdRequired: true,
      modelDiscoverySupported: false,
      chatTemplatePolicy: "provider_managed",
      defaultTimeoutMs: 60_000,
    },
  ];
}

function profileDetail(
  providerKind: "llama_cpp" | "deepseek" = "llama_cpp",
  overrides: Partial<aiIpc.AiProviderProfileDetail> = {},
): aiIpc.AiProviderProfileDetail {
  const deepseek = providerKind === "deepseek";
  return {
    id: deepseek ? deepseekId : llamaId,
    displayName: deepseek ? "DeepSeek course model" : "Local nursing model",
    providerKind,
    providerLabel: deepseek ? "DeepSeek" : "llama.cpp / Unsloth GGUF",
    baseUrl: deepseek ? "https://api.deepseek.com/" : "http://127.0.0.1:8080/",
    baseUrlEditable: !deepseek,
    modelId: deepseek ? "deepseek-v4-flash" : "nursing-qwen",
    authMode: deepseek ? "bearer" : "none",
    enabled: true,
    allowInsecureHttp: false,
    requestTimeoutMs: 60_000,
    credentialEpoch: 0,
    networkScope: deepseek ? "cloud" : "local",
    credentialStatus: {
      status: deepseek ? "missing" : "not_required",
      authenticationRequired: deepseek,
      securePersistenceAvailable: true,
      sessionOnlyAvailable: deepseek,
    },
    configurationStatus: deepseek ? "incomplete" : "ready",
    revision: 1,
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

function readyDeepseekProfile(): aiIpc.AiProviderProfileDetail {
  return profileDetail("deepseek", {
    credentialStatus: {
      status: "saved_securely",
      authenticationRequired: true,
      securePersistenceAvailable: true,
      sessionOnlyAvailable: true,
    },
    configurationStatus: "ready",
  });
}

function summary(detail: aiIpc.AiProviderProfileDetail): aiIpc.AiProviderProfileSummary {
  return {
    id: detail.id,
    displayName: detail.displayName,
    providerKind: detail.providerKind,
    providerLabel: detail.providerLabel,
    modelId: detail.modelId,
    enabled: detail.enabled,
    networkScope: detail.networkScope,
    credentialStatus: detail.credentialStatus,
    configurationStatus: detail.configurationStatus,
    revision: detail.revision,
    updatedAt: detail.updatedAt,
  };
}

function renderSettings(onDirtyChange = vi.fn(), showToast = vi.fn()) {
  render(
    <OverlayProvider>
      <SettingsScreen
        settings={makeSettings()}
        onPreviewSettings={vi.fn()}
        onSavedSettings={vi.fn()}
        onDirtyChange={onDirtyChange}
        showToast={showToast}
      />
    </OverlayProvider>,
  );
  return { onDirtyChange, showToast };
}

async function openAiTab(): Promise<void> {
  await screen.findByText("/tmp/ostudy/data");
  fireEvent.click(screen.getByRole("tab", { name: "AI/LLM" }));
}

describe("SettingsScreen AI/LLM behavior", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(settingsIpc.getAppInfo).mockResolvedValue({
      appVersion: "0.1.0",
      dataDirectory: "/tmp/ostudy/data",
      databasePath: "/tmp/ostudy/data/library.sqlite3",
      imageDirectory: "/tmp/ostudy/data/assets/images",
      databaseSchemaVersion: 16,
    });
    vi.mocked(settingsIpc.saveSettings).mockImplementation(async (value) => value);
    vi.mocked(aiIpc.getAiProviderCatalog).mockResolvedValue(catalog());
    vi.mocked(aiIpc.getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null });
    vi.mocked(aiIpc.getAiProviderProbeResult).mockResolvedValue(null);
    vi.mocked(aiIpc.listAiProviderModels).mockImplementation(async (profileId, revision) => {
      if (profileId === deepseekId) {
        return {
          profileId,
          profileRevision: revision,
          providerKind: "deepseek",
          models: [
            {
              id: "deepseek-v4-flash",
              displayName: "deepseek-v4-flash",
              ownedBy: "deepseek",
            },
            {
              id: "deepseek-v4-pro",
              displayName: "deepseek-v4-pro",
              ownedBy: "deepseek",
            },
          ],
          retrievedAt: 2,
        };
      }
      return {
        profileId,
        profileRevision: revision,
        providerKind: "llama_cpp",
        models: [{ id: "nursing-qwen", displayName: "nursing-qwen", ownedBy: "local" }],
        retrievedAt: 1,
      };
    });
  });

  it("creates a llama.cpp endpoint before automatically discovering its API model", async () => {
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
    const created = profileDetail("llama_cpp", {
      modelId: "",
      configurationStatus: "incomplete",
    });
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(created);
    renderSettings();
    await openAiTab();

    fireEvent.click(screen.getByRole("button", { name: "New profile" }));
    expect(
      (screen.getByRole("combobox", { name: "Provider type" }) as HTMLSelectElement).value,
    ).toBe("llama_cpp");
    expect(screen.getByRole("heading", { name: "1. Provider connection" })).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Model" })).toBeNull();
    expect(screen.getByText(/GET \/v1\/models/u)).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), {
      target: { value: "Local nursing model" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create profile" }));

    await waitFor(() => {
      expect(aiIpc.createAiProviderProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          providerKind: "llama_cpp",
          baseUrl: "http://127.0.0.1:8080/",
          modelId: "",
          authMode: "none",
        }),
      );
      expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(llamaId, 1);
    });
    const modelSelect = await screen.findByRole("combobox", { name: "Model" });
    expect((modelSelect as HTMLSelectElement).value).toBe("nursing-qwen");
    expect(screen.getByRole("button", { name: "Save selected model" })).toBeTruthy();
  });

  it("requires the API credential before DeepSeek model discovery and loads models after save", async () => {
    const detail = profileDetail("deepseek", {
      modelId: "",
      configurationStatus: "incomplete",
    });
    const credentialSaved = {
      ...detail,
      revision: 2,
      credentialEpoch: 1,
      credentialStatus: {
        status: "saved_securely" as const,
        authenticationRequired: true,
        securePersistenceAvailable: true,
        sessionOnlyAvailable: true,
      },
    };
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    vi.mocked(aiIpc.setAiProviderCredential).mockResolvedValue(credentialSaved);
    renderSettings();
    await openAiTab();

    await screen.findByDisplayValue("DeepSeek course model");
    expect(screen.getByRole("heading", { name: "2. API credential" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "3. Available model" })).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Model" })).toBeNull();
    expect(screen.getByText(/Save the API credential above/u)).toBeTruthy();
    expect(aiIpc.listAiProviderModels).not.toHaveBeenCalled();

    const credential = screen.getByLabelText("API credential");
    fireEvent.change(credential, { target: { value: "sentinel-secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Save credential" }));

    await waitFor(() => {
      expect(aiIpc.setAiProviderCredential).toHaveBeenCalledWith(
        deepseekId,
        1,
        "sentinel-secret",
        "secure",
      );
      expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(deepseekId, 2);
    });
    const modelSelect = await screen.findByRole("combobox", { name: "Model" });
    expect(screen.getByRole("option", { name: "deepseek-v4-flash" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "deepseek-v4-pro" })).toBeTruthy();
    expect((modelSelect as HTMLSelectElement).value).toBe("");
    expect(screen.queryByText("sentinel-secret")).toBeNull();
  });

  it("never reloads a stored DeepSeek credential into the password field", async () => {
    const detail = readyDeepseekProfile();
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    vi.mocked(aiIpc.setAiProviderCredential).mockResolvedValue({
      ...detail,
      revision: 2,
      credentialEpoch: 1,
    });
    renderSettings();
    await openAiTab();

    const credential = await screen.findByLabelText("Replacement credential");
    expect((credential as HTMLInputElement).value).toBe("");
    fireEvent.change(credential, { target: { value: "sentinel-secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Replace credential" }));
    await waitFor(() => {
      expect(aiIpc.setAiProviderCredential).toHaveBeenCalledWith(
        deepseekId,
        1,
        "sentinel-secret",
        "secure",
      );
      expect((screen.getByLabelText("Replacement credential") as HTMLInputElement).value).toBe("");
    });
    expect(screen.queryByText("sentinel-secret")).toBeNull();
  });

  it("automatically adopts a sole llama.cpp API model as an unsaved model choice", async () => {
    const detail = profileDetail("llama_cpp", { modelId: "unsloth/Qwen3.6-27B-MTP-GGUF" });
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    vi.mocked(aiIpc.listAiProviderModels).mockResolvedValue({
      profileId: llamaId,
      profileRevision: 1,
      providerKind: "llama_cpp",
      models: [
        {
          id: "/models/Qwen3.6-27B-MTP-Q4_K_M.gguf",
          displayName: "/models/Qwen3.6-27B-MTP-Q4_K_M.gguf",
          ownedBy: "llamacpp",
        },
      ],
      retrievedAt: 1,
    });
    renderSettings();
    await openAiTab();
    await screen.findByDisplayValue("Local nursing model");

    await waitFor(() => expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(llamaId, 1));
    const modelSelect = await screen.findByRole("combobox", { name: "Model" });
    expect((modelSelect as HTMLSelectElement).value).toBe(
      "/models/Qwen3.6-27B-MTP-Q4_K_M.gguf",
    );
    expect(screen.getByRole("button", { name: "Save selected model" })).toBeTruthy();
    expect(aiIpc.updateAiProviderProfile).not.toHaveBeenCalled();
  });

  it("loads DeepSeek model choices from the API and saves the selected model", async () => {
    const detail = readyDeepseekProfile();
    const saved = { ...detail, modelId: "deepseek-v4-pro", revision: 2 };
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    vi.mocked(aiIpc.updateAiProviderProfile).mockResolvedValue(saved);
    renderSettings();
    await openAiTab();
    await screen.findByDisplayValue("DeepSeek course model");

    await waitFor(() => expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(deepseekId, 1));
    const modelSelect = await screen.findByRole("combobox", { name: "Model" });
    expect(screen.getByRole("option", { name: "deepseek-v4-pro" })).toBeTruthy();
    fireEvent.change(modelSelect, { target: { value: "deepseek-v4-pro" } });
    fireEvent.click(screen.getByRole("button", { name: "Save selected model" }));

    await waitFor(() => {
      expect(aiIpc.updateAiProviderProfile).toHaveBeenCalledWith(
        deepseekId,
        1,
        expect.objectContaining({ modelId: "deepseek-v4-pro" }),
      );
    });
  });

  it("shows a redacted plaintext connection transcript with the llama.cpp v1 routes", async () => {
    const detail = profileDetail("llama_cpp");
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    vi.mocked(aiIpc.testAiProviderProfile).mockResolvedValue({
      profileId: llamaId,
      profileRevision: 1,
      credentialEpoch: 0,
      testedModelId: "nursing-qwen",
      adapterRevision: 2,
      resultSchemaVersion: 1,
      overallStatus: "verified",
      phases: [
        {
          phase: "text_completion",
          label: "Text completion",
          status: "passed",
          message: "The minimal probe passed.",
          latencyMs: 12,
          error: null,
        },
      ],
      capabilities: [
        {
          key: "server_chat_template",
          status: "supported",
          source: "declared",
          value: { sha256: "a".repeat(64) },
        },
        {
          key: "json_schema_output",
          status: "supported",
          source: "probed",
          value: null,
        },
      ],
      testedAt: 1,
      persisted: true,
      stale: false,
    });
    const { showToast } = renderSettings();
    await openAiTab();
    await screen.findByDisplayValue("Local nursing model");
    await waitFor(() => expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(llamaId, 1));

    fireEvent.click(screen.getByRole("button", { name: "Test connection" }));

    const diagnostic = await screen.findByLabelText("Connection diagnostic log");
    const transcript = diagnostic.textContent ?? "";
    expect(screen.queryByText("View diagnostic information")).toBeNull();
    expect(transcript).toContain("ostudy AI provider connection test");
    expect(transcript).toContain("model_discovery: GET http://127.0.0.1:8080/v1/models");
    expect(transcript).toContain(
      "server_properties: GET http://127.0.0.1:8080/props?model=nursing-qwen",
    );
    expect(transcript).toContain(
      "text_completion: POST http://127.0.0.1:8080/v1/chat/completions",
    );
    expect(transcript).toContain("secret value redacted");
    expect(transcript).not.toContain("sentinel-secret");
    expect(showToast).toHaveBeenCalledWith("success", "AI provider profile verified.");
  });

  it("presents DeepSeek provider JSON Schema absence as expected neutral metadata", async () => {
    const detail = readyDeepseekProfile();
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    vi.mocked(aiIpc.testAiProviderProfile).mockResolvedValue({
      profileId: deepseekId,
      profileRevision: 1,
      credentialEpoch: 0,
      testedModelId: "deepseek-v4-flash",
      adapterRevision: 2,
      resultSchemaVersion: 1,
      overallStatus: "verified",
      phases: [
        {
          phase: "structured_output",
          label: "Structured output",
          status: "passed",
          message: "Local validation passed.",
          latencyMs: 20,
          error: null,
        },
      ],
      capabilities: [
        {
          key: "json_object_output",
          status: "supported",
          source: "probed",
          value: null,
        },
        {
          key: "json_schema_output",
          status: "unsupported",
          source: "probed",
          value: null,
        },
        {
          key: "local_schema_validation",
          status: "supported",
          source: "probed",
          value: null,
        },
      ],
      testedAt: 3,
      persisted: true,
      stale: false,
    });
    renderSettings();
    await openAiTab();
    await screen.findByDisplayValue("DeepSeek course model");
    await waitFor(() => expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(deepseekId, 1));
    fireEvent.click(screen.getByRole("button", { name: "Test connection" }));

    expect(await screen.findByText("Provider JSON Schema")).toBeTruthy();
    expect(screen.getByText("not offered · provider")).toBeTruthy();
    expect(screen.getByText("Local validation")).toBeTruthy();
    expect(screen.getByText(/expected, not a connection failure/u)).toBeTruthy();
  });

  it("marks credential entry as unsaved settings without persisting the value", async () => {
    const detail = profileDetail("deepseek");
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    const onDirtyChange = vi.fn();
    renderSettings(onDirtyChange);
    await openAiTab();
    const credential = await screen.findByLabelText("API credential");
    fireEvent.change(credential, { target: { value: "temporary-entry" } });
    await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(true));
    expect(aiIpc.setAiProviderCredential).not.toHaveBeenCalled();
    expect(aiIpc.listAiProviderModels).not.toHaveBeenCalled();
  });
});
