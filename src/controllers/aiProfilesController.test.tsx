import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiIpc from "../ipc/aiProfiles";
import { useAiProfilesController } from "./aiProfilesController";

vi.mock("../ipc/aiProfiles", () => ({
  clearAiProviderCredential: vi.fn(),
  createAiProviderProfile: vi.fn(),
  deleteAiProviderProfile: vi.fn(),
  getAiProviderCatalog: vi.fn(),
  getAiProviderProbeResult: vi.fn(),
  getAiProviderProfile: vi.fn(),
  getAiTaskDefaults: vi.fn(),
  listAiProviderModels: vi.fn(),
  listAiProviderProfiles: vi.fn(),
  setAiProviderCredential: vi.fn(),
  setFlashcardGenerationProfile: vi.fn(),
  testAiProviderProfile: vi.fn(),
  updateAiProviderProfile: vi.fn(),
}));

const profileId = "a".repeat(32);

function openRouterProfile(
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
    configurationStatus: "incomplete",
    revision: 2,
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}

function summary(profile: aiIpc.AiProviderProfileDetail): aiIpc.AiProviderProfileSummary {
  return {
    id: profile.id,
    displayName: profile.displayName,
    providerKind: profile.providerKind,
    providerLabel: profile.providerLabel,
    modelId: profile.modelId,
    enabled: profile.enabled,
    networkScope: profile.networkScope,
    credentialStatus: profile.credentialStatus,
    configurationStatus: profile.configurationStatus,
    revision: profile.revision,
    updatedAt: profile.updatedAt,
  };
}

function modelResult(revision: number): aiIpc.AiProviderModels {
  return {
    profileId,
    profileRevision: revision,
    providerKind: "openrouter",
    models: [
      { id: "anthropic/claude-example", displayName: "Claude Example", ownedBy: "anthropic" },
      { id: "openai/gpt-example", displayName: "GPT Example", ownedBy: "openai" },
    ],
    retrievedAt: 3,
  };
}

describe("useAiProfilesController", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    const initial = openRouterProfile();
    vi.mocked(aiIpc.getAiProviderCatalog).mockResolvedValue([
      {
        providerKind: "openrouter",
        displayName: "OpenRouter",
        description: "Use OpenRouter.",
        implementationStatus: "available",
        defaultBaseUrl: "https://openrouter.ai/api/v1/",
        baseUrlEditable: false,
        authenticationRequired: true,
        modelIdRequired: true,
        modelDiscoverySupported: true,
        chatTemplatePolicy: "provider_managed",
        defaultTimeoutMs: 60_000,
      },
    ]);
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(initial)]);
    vi.mocked(aiIpc.getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null });
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(initial);
    vi.mocked(aiIpc.getAiProviderProbeResult).mockResolvedValue(null);
    vi.mocked(aiIpc.listAiProviderModels).mockImplementation(async (_profileId, revision) =>
      modelResult(revision),
    );
  });

  it("creates a profile, stores the entered API key, then discovers models as one setup action", async () => {
    const created = openRouterProfile({
      credentialEpoch: 0,
      credentialStatus: {
        status: "missing",
        authenticationRequired: true,
        securePersistenceAvailable: true,
        sessionOnlyAvailable: true,
      },
      revision: 1,
      updatedAt: 3,
    });
    const connected = openRouterProfile({ revision: 2, updatedAt: 4 });
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.setAiProviderCredential).mockResolvedValue(connected);
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(connected)]);

    const showToast = vi.fn();
    const { result } = renderHook(() => useAiProfilesController(showToast));
    await waitFor(() => expect(result.current.catalog.length).toBe(1));

    act(() => {
      result.current.beginCreate("openrouter");
      result.current.setCredentialDraft("secret-key");
    });

    await act(async () => {
      await result.current.connectSetup();
    });

    expect(aiIpc.createAiProviderProfile).toHaveBeenCalledWith(
      expect.objectContaining({
        providerKind: "openrouter",
        authMode: "bearer",
        enabled: true,
      }),
    );
    expect(aiIpc.setAiProviderCredential).toHaveBeenCalledWith(
      profileId,
      1,
      "secret-key",
      "secure",
    );
    expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(profileId, 2);
    expect(result.current.credentialDraft).toBe("");
    expect(result.current.modelsLoaded).toBe(true);
    expect(showToast).toHaveBeenCalledWith("success", "AI connection saved.");
  });

  it("resumes a created profile instead of duplicating it after credential storage fails", async () => {
    const created = openRouterProfile({
      credentialEpoch: 0,
      credentialStatus: {
        status: "missing",
        authenticationRequired: true,
        securePersistenceAvailable: true,
        sessionOnlyAvailable: true,
      },
      revision: 1,
      updatedAt: 3,
    });
    const connected = openRouterProfile({ revision: 2, updatedAt: 4 });
    vi.mocked(aiIpc.listAiProviderProfiles)
      .mockResolvedValueOnce([])
      .mockResolvedValue([summary(connected)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.setAiProviderCredential)
      .mockRejectedValueOnce(new Error("Secure credential storage unavailable."))
      .mockResolvedValueOnce(connected);

    const showToast = vi.fn();
    const { result } = renderHook(() => useAiProfilesController(showToast));
    await waitFor(() => expect(result.current.catalog.length).toBe(1));

    act(() => {
      result.current.beginCreate("openrouter");
      result.current.setCredentialDraft("secret-key");
    });
    await act(async () => {
      await result.current.connectSetup();
    });

    await waitFor(() => {
      expect(result.current.selectedProfile?.revision).toBe(1);
      expect(result.current.errorMessage).toBe("Secure credential storage unavailable.");
    });
    expect(aiIpc.createAiProviderProfile).toHaveBeenCalledTimes(1);

    act(() => result.current.setCredentialDraft("secret-key"));
    await act(async () => {
      await result.current.connectSetup();
    });

    expect(aiIpc.createAiProviderProfile).toHaveBeenCalledTimes(1);
    expect(aiIpc.updateAiProviderProfile).not.toHaveBeenCalled();
    expect(aiIpc.setAiProviderCredential).toHaveBeenLastCalledWith(
      profileId,
      1,
      "secret-key",
      "secure",
    );
    await waitFor(() => {
      expect(result.current.selectedProfile?.revision).toBe(2);
      expect(result.current.credentialDraft).toBe("");
      expect(result.current.errorMessage).toBeNull();
      expect(result.current.modelsLoaded).toBe(true);
    });
  });

  it("saves an API-selected model and immediately allows the connection probe", async () => {
    const saved = openRouterProfile({
      modelId: "openai/gpt-example",
      configurationStatus: "ready",
      revision: 3,
      updatedAt: 4,
    });
    vi.mocked(aiIpc.updateAiProviderProfile).mockResolvedValue(saved);
    vi.mocked(aiIpc.testAiProviderProfile).mockResolvedValue({
      profileId,
      profileRevision: 3,
      credentialEpoch: 1,
      testedModelId: "openai/gpt-example",
      adapterRevision: 4,
      resultSchemaVersion: 1,
      overallStatus: "verified",
      phases: [],
      capabilities: [],
      testedAt: 5,
      persisted: true,
      stale: false,
    });
    const showToast = vi.fn();
    const { result } = renderHook(() => useAiProfilesController(showToast));

    await waitFor(() => {
      expect(result.current.selectedProfile?.revision).toBe(2);
      expect(result.current.modelsLoaded).toBe(true);
    });

    act(() => result.current.updateDraft({ modelId: "openai/gpt-example" }));
    await waitFor(() => expect(result.current.dirty).toBe(true));

    await act(async () => {
      await result.current.saveProfile();
    });

    expect(aiIpc.updateAiProviderProfile).toHaveBeenCalledWith(
      profileId,
      2,
      expect.objectContaining({ modelId: "openai/gpt-example" }),
    );
    expect(result.current.errorMessage).toBeNull();
    await waitFor(() => {
      expect(result.current.selectedProfile?.revision).toBe(3);
      expect(result.current.dirty).toBe(false);
    });

    await act(async () => {
      await result.current.testProfile();
    });

    expect(aiIpc.testAiProviderProfile).toHaveBeenCalledWith(profileId, 3);
    expect(result.current.probeResult?.overallStatus).toBe("verified");
    expect(result.current.errorMessage).toBeNull();
    expect(showToast).toHaveBeenCalledWith("success", "AI provider profile verified.");
  });
});