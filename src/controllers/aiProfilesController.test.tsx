import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiIpc from "../ipc/aiProfiles";
import * as taskIpc from "../ipc/aiTaskDefaults";
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
vi.mock("../ipc/aiTaskDefaults", () => ({ setAiTaskDefault: vi.fn() }));

const profileId = "a".repeat(32);
const secondProfileId = "b".repeat(32);

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

function summary(profile: aiIpc.AiProviderProfileDetail): aiIpc.AiProviderProfileSummary {
  return {
    id: profile.id,
    displayName: profile.displayName,
    providerKind: profile.providerKind,
    providerLabel: profile.providerLabel,
    modelId: profile.modelId,
    reasoningEffort: profile.reasoningEffort,
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
      { id: "qwen/qwen3-embedding", displayName: "Qwen Embedding", ownedBy: "qwen" },
    ],
    retrievedAt: 3,
  };
}

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
    vi.mocked(aiIpc.getAiTaskDefaults).mockResolvedValue(emptyDefaults);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(initial);
    vi.mocked(aiIpc.getAiProviderProbeResult).mockResolvedValue(null);
    vi.mocked(aiIpc.listAiProviderModels).mockImplementation(async (_profileId, revision) =>
      modelResult(revision),
    );
    vi.mocked(taskIpc.setAiTaskDefault).mockImplementation(async (input) => ({
      ...emptyDefaults,
      ...(input.taskKind === "flashcard_generation"
        ? {
            flashcardGenerationProfileId: input.profileId,
            flashcardGenerationModelId: input.modelId,
            flashcardGenerationReasoningEffort: input.reasoningEffort,
          }
        : input.taskKind === "tutor"
          ? {
              tutorProfileId: input.profileId,
              tutorModelId: input.modelId,
              tutorReasoningEffort: input.reasoningEffort,
            }
          : {
              embeddingsProfileId: input.profileId,
              embeddingsModelId: input.modelId,
              embeddingsReasoningEffort: input.reasoningEffort,
            }),
    }));
  });

  it("creates a provider connection, stores the API key once, and discovers its reusable model catalog", async () => {
    const created = openRouterProfile({
      credentialEpoch: 0,
      credentialStatus: {
        status: "missing",
        authenticationRequired: true,
        securePersistenceAvailable: true,
        sessionOnlyAvailable: true,
      },
      configurationStatus: "incomplete",
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
    await act(async () => { await result.current.connectSetup(); });

    expect(aiIpc.createAiProviderProfile).toHaveBeenCalledWith(
      expect.objectContaining({ providerKind: "openrouter", modelId: "", authMode: "bearer" }),
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

  it("resumes a created provider instead of duplicating it after credential storage fails", async () => {
    const created = openRouterProfile({
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
    const connected = openRouterProfile({ revision: 2 });
    vi.mocked(aiIpc.listAiProviderProfiles)
      .mockResolvedValueOnce([])
      .mockResolvedValue([summary(connected)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.setAiProviderCredential)
      .mockRejectedValueOnce(new Error("Secure credential storage unavailable."))
      .mockResolvedValueOnce(connected);

    const { result } = renderHook(() => useAiProfilesController(vi.fn()));
    await waitFor(() => expect(result.current.catalog.length).toBe(1));
    act(() => {
      result.current.beginCreate("openrouter");
      result.current.setCredentialDraft("secret-key");
    });
    await act(async () => { await result.current.connectSetup(); });
    await waitFor(() => expect(result.current.selectedProfile?.revision).toBe(1));

    act(() => result.current.setCredentialDraft("secret-key"));
    await act(async () => { await result.current.connectSetup(); });

    expect(aiIpc.createAiProviderProfile).toHaveBeenCalledTimes(1);
    expect(aiIpc.setAiProviderCredential).toHaveBeenLastCalledWith(
      profileId,
      1,
      "secret-key",
      "secure",
    );
    await waitFor(() => expect(result.current.selectedProfile?.revision).toBe(2));
  });

  it("creates a distinct display name for a second API-key connection to the same provider", async () => {
    const first = openRouterProfile();
    const second = openRouterProfile({ id: secondProfileId, displayName: "OpenRouter 2" });
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(first), summary(second)]);
    const { result } = renderHook(() => useAiProfilesController(vi.fn()));
    await waitFor(() => expect(result.current.profiles).toHaveLength(2));

    act(() => result.current.beginCreate("openrouter"));
    expect(result.current.draft?.displayName).toBe("OpenRouter 3");
  });

  it("assigns different models from one provider connection without copying or replacing credentials", async () => {
    const { result } = renderHook(() => useAiProfilesController(vi.fn()));
    await waitFor(() => expect(result.current.profiles).toHaveLength(1));

    await act(async () => {
      await result.current.setTaskDefault("flashcard_generation", profileId, "openai/gpt-example", "high");
      await result.current.setTaskDefault("tutor", profileId, "anthropic/claude-example", "medium");
      await result.current.setTaskDefault("embeddings", profileId, "qwen/qwen3-embedding", "none");
    });

    expect(taskIpc.setAiTaskDefault).toHaveBeenCalledTimes(3);
    expect(taskIpc.setAiTaskDefault).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        taskKind: "flashcard_generation",
        profileId,
        modelId: "openai/gpt-example",
        reasoningEffort: "high",
      }),
    );
    expect(taskIpc.setAiTaskDefault).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        taskKind: "tutor",
        profileId,
        modelId: "anthropic/claude-example",
        reasoningEffort: "medium",
      }),
    );
    expect(taskIpc.setAiTaskDefault).toHaveBeenNthCalledWith(
      3,
      expect.objectContaining({
        taskKind: "embeddings",
        profileId,
        modelId: "qwen/qwen3-embedding",
        reasoningEffort: "none",
      }),
    );
    expect(aiIpc.setAiProviderCredential).not.toHaveBeenCalled();
  });
});
