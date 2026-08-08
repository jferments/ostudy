import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiIpc from "../ipc/aiProfiles";
import * as settingsIpc from "../ipc/settings";
import { makeSettings } from "../test/factories";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("../platform/files", () => ({ openFile: vi.fn(), saveFile: vi.fn() }));
vi.mock("../ipc/backup", () => ({ exportLibraryBackup: vi.fn(), prepareLibraryRestore: vi.fn() }));
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
    vi.mocked(aiIpc.listAiProviderModels).mockResolvedValue({
      profileId: llamaId,
      profileRevision: 1,
      providerKind: "llama_cpp",
      models: [{ id: "nursing-qwen", displayName: "nursing-qwen", ownedBy: "local" }],
      retrievedAt: 1,
    });
  });

  it("creates a llama.cpp profile without treating Unsloth as a separate protocol", async () => {
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
    const created = profileDetail("llama_cpp");
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(created);
    renderSettings();
    await openAiTab();

    fireEvent.click(screen.getByRole("button", { name: "Add llama.cpp" }));
    expect(screen.getByText(/Unsloth-exported GGUF/u)).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), {
      target: { value: "Local nursing model" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "Model ID" }), {
      target: { value: "nursing-qwen" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create profile" }));

    await waitFor(() => {
      expect(aiIpc.createAiProviderProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          providerKind: "llama_cpp",
          baseUrl: "http://127.0.0.1:8080/",
          modelId: "nursing-qwen",
          authMode: "none",
        }),
      );
    });
  });

  it("never reloads a stored DeepSeek credential into the password field", async () => {
    const detail = profileDetail("deepseek", {
      credentialStatus: {
        status: "saved_securely",
        authenticationRequired: true,
        securePersistenceAvailable: true,
        sessionOnlyAvailable: true,
      },
      configurationStatus: "ready",
    });
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

  it("adopts a sole llama.cpp server model ID as an unsaved draft after refresh", async () => {
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
    const { showToast } = renderSettings();
    await openAiTab();
    await screen.findByDisplayValue("Local nursing model");

    fireEvent.click(screen.getByRole("button", { name: "Refresh models" }));
    const modelInput = await screen.findByRole("combobox", { name: "Model ID" });
    await waitFor(() => {
      expect((modelInput as HTMLInputElement).value).toBe(
        "/models/Qwen3.6-27B-MTP-Q4_K_M.gguf",
      );
    });
    expect((screen.getByRole("button", { name: "Save profile" }) as HTMLButtonElement).disabled).toBe(
      false,
    );
    expect(aiIpc.updateAiProviderProfile).not.toHaveBeenCalled();
    expect(showToast).toHaveBeenCalledWith(
      "info",
      expect.stringContaining("server-reported ID"),
    );
  });

  it("shows model discovery and a redacted capability probe result", async () => {
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

    fireEvent.click(screen.getByRole("button", { name: "Refresh models" }));
    await waitFor(() => expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(llamaId, 1));
    expect(await screen.findByText(/1 model\(s\) returned/u)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
    expect(await screen.findByText("Text completion")).toBeTruthy();
    expect(screen.getByText("Server chat template")).toBeTruthy();
    expect(screen.getByText("JSON Schema")).toBeTruthy();
    fireEvent.click(screen.getByText("View diagnostic information"));
    expect(screen.getByText("Adapter revision")).toBeTruthy();
    expect(screen.getByText("http://127.0.0.1:8080/")).toBeTruthy();
    expect(screen.getByText(/never includes the API credential/u)).toBeTruthy();
    expect(screen.queryByText("sentinel-secret")).toBeNull();
    expect(showToast).toHaveBeenCalledWith("success", "AI provider profile verified.");
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
  });
});
