import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiIpc from "../ipc/aiProfiles";
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
    requireZeroDataRetention: false,
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

function readyDeepseekProfile(
  overrides: Partial<aiIpc.AiProviderProfileDetail> = {},
): aiIpc.AiProviderProfileDetail {
  return profileDetail("deepseek", {
    credentialStatus: {
      status: "saved_securely",
      authenticationRequired: true,
      securePersistenceAvailable: true,
      sessionOnlyAvailable: true,
    },
    configurationStatus: "ready",
    ...overrides,
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
  await waitFor(() => expect(settingsIpc.getAppInfo).toHaveBeenCalledOnce());
  fireEvent.click(screen.getByRole("tab", { name: "AI/LLM" }));
}

function deepseekModels(revision: number): aiIpc.AiProviderModels {
  return {
    profileId: deepseekId,
    profileRevision: revision,
    providerKind: "deepseek",
    models: [
      { id: "deepseek-v4-flash", displayName: "deepseek-v4-flash", ownedBy: "deepseek" },
      { id: "deepseek-v4-pro", displayName: "deepseek-v4-pro", ownedBy: "deepseek" },
    ],
    retrievedAt: 2,
  };
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
      if (profileId === deepseekId) return deepseekModels(revision);
      return {
        profileId,
        profileRevision: revision,
        providerKind: "llama_cpp",
        models: [{ id: "nursing-qwen", displayName: "nursing-qwen", ownedBy: "local" }],
        retrievedAt: 1,
      };
    });
  });

  it("connects a new DeepSeek profile with one API-key action and then saves the selected model", async () => {
    const created = profileDetail("deepseek", {
      modelId: "",
      configurationStatus: "incomplete",
      revision: 1,
    });
    const connected = readyDeepseekProfile({ modelId: "", revision: 2, credentialEpoch: 1 });
    const saved = readyDeepseekProfile({ modelId: "deepseek-v4-pro", revision: 3 });
    vi.mocked(aiIpc.listAiProviderProfiles)
      .mockResolvedValueOnce([])
      .mockResolvedValue([summary(connected)]);
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.setAiProviderCredential).mockResolvedValue(connected);
    vi.mocked(aiIpc.updateAiProviderProfile).mockResolvedValue(saved);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(connected);

    renderSettings();
    await openAiTab();
    fireEvent.click(screen.getByRole("button", { name: "Add model" }));
    const provider = await screen.findByRole("combobox", { name: "Provider" });
    fireEvent.change(provider, { target: { value: "deepseek" } });

    expect(screen.queryByText("Bearer token")).toBeNull();
    expect(screen.queryByText("Enable profile")).toBeNull();
    const apiKey = await screen.findByLabelText("API key");
    expect(
      (screen.getByRole("button", { name: "Connect and load models" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    fireEvent.change(apiKey, { target: { value: "sentinel-secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Connect and load models" }));

    await waitFor(() => {
      expect(aiIpc.createAiProviderProfile).toHaveBeenCalledWith(
        expect.objectContaining({ providerKind: "deepseek", modelId: "", authMode: "bearer" }),
      );
      expect(aiIpc.setAiProviderCredential).toHaveBeenCalledWith(
        deepseekId,
        1,
        "sentinel-secret",
        "secure",
      );
      expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(deepseekId, 2);
    });

    const model = await screen.findByRole("combobox", { name: "Model" });
    expect(screen.getByRole("option", { name: "deepseek-v4-pro" })).toBeTruthy();
    expect((apiKey as HTMLInputElement).value).toBe("");
    expect(screen.queryByText("sentinel-secret")).toBeNull();

    fireEvent.change(model, { target: { value: "deepseek-v4-pro" } });
    fireEvent.click(screen.getByRole("button", { name: "Save profile" }));
    await waitFor(() => {
      expect(aiIpc.updateAiProviderProfile).toHaveBeenCalledWith(
        deepseekId,
        2,
        expect.objectContaining({ modelId: "deepseek-v4-pro" }),
      );
    });
  });

  it("configures a local llama.cpp server without exposing authentication jargon in the basic flow", async () => {
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
    const created = profileDetail("llama_cpp", { modelId: "", configurationStatus: "incomplete" });
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(created);

    renderSettings();
    await openAiTab();
    fireEvent.click(screen.getByRole("button", { name: "Add model" }));

    const provider = await screen.findByRole("combobox", { name: "Provider" });
    expect((provider as HTMLSelectElement).value).toBe("llama_cpp");
    const serverUrl = screen.getByRole("textbox", { name: "Server URL" });
    expect(serverUrl).toBeTruthy();
    expect(screen.queryByLabelText("API key")).toBeNull();
    expect(screen.queryByText("Bearer token")).toBeNull();
    expect(screen.queryByText("Enable profile")).toBeNull();

    const advancedSummary = screen.getByText("Advanced settings");
    const advanced = advancedSummary.closest("details");
    expect(advanced?.hasAttribute("open")).toBe(false);
    expect(within(advanced as HTMLElement).getByText("Provider inactivity timeout")).toBeTruthy();

    fireEvent.change(serverUrl, { target: { value: "http://[::1]:8080/" } });
    expect(
      within(advanced as HTMLElement).queryByText("Allow unencrypted LAN connection"),
    ).toBeNull();
    fireEvent.change(serverUrl, { target: { value: "http://192.168.1.50:8080/" } });
    expect(
      within(advanced as HTMLElement).getByText("Allow unencrypted LAN connection"),
    ).toBeTruthy();
    fireEvent.change(serverUrl, { target: { value: "http://127.0.0.1:8080/" } });
    expect(
      within(advanced as HTMLElement).queryByText("Allow unencrypted LAN connection"),
    ).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Connect and load models" }));
    await waitFor(() => {
      expect(aiIpc.createAiProviderProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          providerKind: "llama_cpp",
          baseUrl: "http://127.0.0.1:8080/",
          authMode: "none",
        }),
      );
      expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(llamaId, 1);
    });
    expect((await screen.findByRole("combobox", { name: "Model" }) as HTMLSelectElement).value).toBe(
      "nursing-qwen",
    );
  });

  it("keeps optional llama.cpp API-key authentication inside Advanced settings", async () => {
    const detail = profileDetail("llama_cpp");
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    renderSettings();
    await openAiTab();
    await screen.findByDisplayValue("Local nursing model");

    expect(screen.queryByLabelText("API key")).toBeNull();
    fireEvent.click(screen.getByText("Advanced settings"));
    const authentication = screen.getByRole("combobox", { name: "Server authentication" });
    expect((authentication as HTMLSelectElement).value).toBe("none");
    expect(screen.queryByText("Bearer token")).toBeNull();

    fireEvent.change(authentication, { target: { value: "bearer" } });
    expect(screen.getByLabelText("API key")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "Connect and load models" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("never reloads a stored API key and lets advanced metadata save without reconnecting", async () => {
    const detail = readyDeepseekProfile();
    const saved = {
      ...detail,
      displayName: "Course model renamed",
      requestTimeoutMs: 120_000,
      revision: 2,
    };
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    vi.mocked(aiIpc.updateAiProviderProfile).mockResolvedValue(saved);
    renderSettings();
    await openAiTab();
    await screen.findByDisplayValue("DeepSeek course model");
    await waitFor(() => expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(deepseekId, 1));

    const apiKey = screen.getByLabelText("API key");
    expect((apiKey as HTMLInputElement).value).toBe("");
    expect((apiKey as HTMLInputElement).placeholder).toContain("Saved");

    fireEvent.click(screen.getByText("Advanced settings"));
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), {
      target: { value: "Course model renamed" },
    });
    fireEvent.change(screen.getByRole("combobox", { name: "Provider inactivity timeout" }), {
      target: { value: "120000" },
    });

    expect((screen.getByRole("button", { name: "Connected" }) as HTMLButtonElement).disabled).toBe(
      true,
    );
    fireEvent.click(screen.getByRole("button", { name: "Save profile" }));
    await waitFor(() => {
      expect(aiIpc.updateAiProviderProfile).toHaveBeenCalledWith(
        deepseekId,
        1,
        expect.objectContaining({
          displayName: "Course model renamed",
          requestTimeoutMs: 120_000,
          enabled: true,
          authMode: "bearer",
        }),
      );
    });
    expect(aiIpc.setAiProviderCredential).not.toHaveBeenCalled();
  });

  it("shows a compact connection result and keeps phases behind Technical details", async () => {
    const detail = readyDeepseekProfile();
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    vi.mocked(aiIpc.testAiProviderProfile).mockResolvedValue({
      profileId: deepseekId,
      profileRevision: 1,
      credentialEpoch: 1,
      testedModelId: "deepseek-v4-flash",
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
          key: "json_object_output",
          status: "supported",
          source: "probed",
          value: null,
        },
      ],
      testedAt: 3,
      persisted: true,
      stale: false,
    });
    const { showToast } = renderSettings();
    await openAiTab();
    await screen.findByDisplayValue("DeepSeek course model");
    await waitFor(() => expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(deepseekId, 1));

    fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
    expect(await screen.findByText("Connection verified")).toBeTruthy();
    expect(screen.getByText("Technical details")).toBeTruthy();
    fireEvent.click(screen.getByText("Technical details"));
    expect(screen.getByText("Text completion")).toBeTruthy();
    expect(screen.getByText(/json object output/iu)).toBeTruthy();
    expect(showToast).toHaveBeenCalledWith("success", "AI provider profile verified.");
  });

  it("marks an entered API key dirty without persisting it until Connect is pressed", async () => {
    const detail = profileDetail("deepseek");
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    const onDirtyChange = vi.fn();
    renderSettings(onDirtyChange);
    await openAiTab();
    const apiKey = await screen.findByLabelText("API key");
    fireEvent.change(apiKey, { target: { value: "temporary-entry" } });
    await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(true));
    expect(aiIpc.setAiProviderCredential).not.toHaveBeenCalled();
    expect(aiIpc.listAiProviderModels).not.toHaveBeenCalled();
  });
});
