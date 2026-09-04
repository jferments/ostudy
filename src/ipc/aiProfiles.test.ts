import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  clearAiProviderCredential,
  createAiProviderProfile,
  listAiProviderModels,
  setAiProviderCredential,
  testAiProviderProfile,
  updateAiProviderProfile,
} from "./aiProfiles";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("./common", async (importOriginal) => {
  const original = await importOriginal<typeof import("./common")>();
  return { ...original, createOperationId: () => "f".repeat(32) };
});

const profileId = "a".repeat(32);
const draft = {
  displayName: "Local model",
  providerKind: "llama_cpp" as const,
  baseUrl: "http://127.0.0.1:8080/",
  modelId: "model",
  authMode: "none" as const,
  enabled: true,
  allowInsecureHttp: false,
  requireZeroDataRetention: false,
  requestTimeoutMs: 60_000,
};

describe("AI provider IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("creates profiles through a strict operation-identified native mutation", async () => {
    await createAiProviderProfile(draft);
    expect(invoke).toHaveBeenCalledWith("create_ai_provider_profile", {
      input: {
        ...draft,
        reasoningEffort: "low",
        operationId: "f".repeat(32),
      },
    });
  });

  it("omits immutable provider kind and absent editor drafts from update mutations", async () => {
    await updateAiProviderProfile(profileId, 7, draft);
    expect(invoke).toHaveBeenCalledWith("update_ai_provider_profile", {
      input: {
        operationId: "f".repeat(32),
        profileId,
        revision: 7,
        displayName: draft.displayName,
        baseUrl: draft.baseUrl,
        modelId: draft.modelId,
        reasoningEffort: "low",
        authMode: draft.authMode,
        enabled: draft.enabled,
        allowInsecureHttp: draft.allowInsecureHttp,
        requireZeroDataRetention: draft.requireZeroDataRetention,
        requestTimeoutMs: draft.requestTimeoutMs,
      },
    });
    const call = vi.mocked(invoke).mock.calls[0]?.[1] as { input?: Record<string, unknown> };
    expect(call.input).not.toHaveProperty("providerKind");
    expect(call.input).not.toHaveProperty("editorDraft");
  });

  it("submits credentials only to operation-identified write commands", async () => {
    await setAiProviderCredential(profileId, 3, "sentinel-secret", "session");
    expect(invoke).toHaveBeenCalledWith("set_ai_provider_credential", {
      input: {
        profileId,
        revision: 3,
        credential: "sentinel-secret",
        persistence: "session",
        operationId: "f".repeat(32),
      },
    });
    vi.mocked(invoke).mockClear();
    await clearAiProviderCredential(profileId, 4);
    expect(invoke).toHaveBeenCalledWith("clear_ai_provider_credential", {
      input: {
        profileId,
        revision: 4,
        operationId: "f".repeat(32),
      },
    });
  });

  it("exposes narrow model-discovery and profile-test commands instead of generic prompts", async () => {
    await listAiProviderModels(profileId, 2);
    expect(invoke).toHaveBeenCalledWith("list_ai_provider_models", {
      input: { profileId, revision: 2 },
    });
    vi.mocked(invoke).mockClear();
    await testAiProviderProfile(profileId, 2);
    expect(invoke).toHaveBeenCalledWith("test_ai_provider_profile", {
      input: { profileId, revision: 2 },
    });
  });
});
