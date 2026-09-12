import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import { listAiProviderModels, testAiProviderProfile } from "../../ipc/aiProfiles";
import {
  getEmbeddingOptions,
  listEmbeddingModels,
  testEmbeddingProfile,
} from "../../ipc/embeddings";
import { DefaultAiModelSettings } from "./DefaultAiModelSettings";

vi.mock("../../ipc/aiProfiles", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/aiProfiles")>();
  return {
    ...actual,
    listAiProviderModels: vi.fn(),
    testAiProviderProfile: vi.fn(),
  };
});
vi.mock("../../ipc/embeddings", () => ({
  getEmbeddingOptions: vi.fn(),
  listEmbeddingModels: vi.fn(),
  saveEmbeddingOptions: vi.fn(),
  testEmbeddingProfile: vi.fn(),
}));

const profileId = "a".repeat(32);

function controller(): AiProfilesController {
  return {
    catalog: [],
    profiles: [
      {
        id: profileId,
        displayName: "OpenRouter",
        providerKind: "openrouter",
        providerLabel: "OpenRouter",
        modelId: "",
        reasoningEffort: "automatic",
        enabled: true,
        networkScope: "cloud",
        credentialStatus: {
          status: "saved_securely",
          authenticationRequired: true,
          securePersistenceAvailable: true,
          sessionOnlyAvailable: true,
        },
        configurationStatus: "ready",
        revision: 2,
        updatedAt: 1,
      },
    ],
    defaults: {
      flashcardGenerationProfileId: profileId,
      flashcardGenerationModelId: "deepseek/deepseek-v4-flash",
      flashcardGenerationReasoningEffort: "high",
      tutorProfileId: profileId,
      tutorModelId: "anthropic/claude-sonnet",
      tutorReasoningEffort: "medium",
      embeddingsProfileId: profileId,
      embeddingsModelId: "qwen/qwen3-embedding",
      embeddingsReasoningEffort: "none",
    },
    selectedProfile: null,
    selectedProfileId: null,
    draft: null,
    credentialDraft: "",
    credentialPersistence: "secure",
    models: [],
    modelsLoaded: false,
    probeResult: null,
    dirty: false,
    loading: false,
    mutating: false,
    refreshingModels: false,
    testing: false,
    errorMessage: null,
    selectProfile: vi.fn(),
    beginCreate: vi.fn(),
    updateDraft: vi.fn(),
    setCredentialDraft: vi.fn(),
    setCredentialPersistence: vi.fn(),
    saveProfile: vi.fn(),
    connectSetup: vi.fn(),
    deleteProfile: vi.fn(),
    saveCredential: vi.fn(),
    clearCredential: vi.fn(),
    refreshModels: vi.fn(),
    testProfile: vi.fn(),
    setFlashcardDefault: vi.fn().mockResolvedValue(undefined),
    setTaskDefault: vi.fn().mockResolvedValue(undefined),
    pendingTaskDefault: null,
    retryTaskDefault: vi.fn().mockResolvedValue(undefined),
    discardDraft: vi.fn(),
    reload: vi.fn().mockResolvedValue(undefined),
  };
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(listAiProviderModels).mockResolvedValue({
    profileId,
    profileRevision: 2,
    providerKind: "openrouter",
    models: [
      { id: "deepseek/deepseek-v4-flash", displayName: "DeepSeek V4 Flash", ownedBy: "deepseek" },
      { id: "anthropic/claude-sonnet", displayName: "Claude Sonnet", ownedBy: "anthropic" },
    ],
    retrievedAt: 1,
  });
  vi.mocked(listEmbeddingModels).mockResolvedValue([
    { id: "qwen/qwen3-embedding", name: "Qwen3 Embedding" },
  ]);
  vi.mocked(getEmbeddingOptions).mockResolvedValue({
    profileId,
    modelId: "qwen/qwen3-embedding",
    dimensions: null,
    inputPolicy: "automatic",
    revision: 1,
  });
  vi.mocked(testAiProviderProfile).mockResolvedValue({
    profileId,
    profileRevision: 2,
    credentialEpoch: 1,
    testedModelId: "anthropic/claude-sonnet",
    adapterRevision: 1,
    resultSchemaVersion: 1,
    overallStatus: "verified",
    phases: [],
    capabilities: [],
    testedAt: 1,
    persisted: true,
    stale: false,
  });
  vi.mocked(testEmbeddingProfile).mockResolvedValue({
    profileId,
    modelId: "qwen/qwen3-embedding",
    profileRevision: 2,
    optionsRevision: 1,
    dimensions: 768,
    spaceId: "b".repeat(64),
    queryAndDocumentTested: true,
  });
});

describe("DefaultAiModelSettings", () => {
  it("uses one provider connection with independent task model assignments", async () => {
    render(<DefaultAiModelSettings controller={controller()} />);

    expect(screen.getByRole("heading", { name: "Default AI models" })).toBeTruthy();
    await waitFor(() => {
      expect(
        (screen.getByRole("combobox", {
          name: "Flashcard and study-guide generation model",
        }) as HTMLSelectElement).value,
      ).toBe("deepseek/deepseek-v4-flash");
      expect(
        (screen.getByRole("combobox", { name: "Interactive tutor model" }) as HTMLSelectElement)
          .value,
      ).toBe("anthropic/claude-sonnet");
      expect(
        (screen.getByRole("combobox", { name: "Semantic embeddings model" }) as HTMLSelectElement)
          .value,
      ).toBe("qwen/qwen3-embedding");
    });
    for (const label of [
      "Flashcard and study-guide generation provider",
      "Interactive tutor provider",
      "Semantic embeddings provider",
    ]) {
      expect((screen.getByRole("combobox", { name: label }) as HTMLSelectElement).value).toBe(
        profileId,
      );
    }
  });

  it("writes provider, model, and reasoning as one task assignment", async () => {
    const value = controller();
    render(<DefaultAiModelSettings controller={value} />);
    const model = await screen.findByRole("combobox", { name: "Interactive tutor model" });
    await waitFor(() => expect((model as HTMLSelectElement).disabled).toBe(false));

    fireEvent.change(model, { target: { value: "deepseek/deepseek-v4-flash" } });
    await waitFor(() => {
      expect(value.setTaskDefault).toHaveBeenCalledWith(
        "tutor",
        profileId,
        "deepseek/deepseek-v4-flash",
        "medium",
      );
    });
    fireEvent.change(screen.getByRole("combobox", { name: "Interactive tutor reasoning" }), {
      target: { value: "high" },
    });
    expect(value.setTaskDefault).toHaveBeenLastCalledWith(
      "tutor",
      profileId,
      "deepseek/deepseek-v4-flash",
      "high",
    );
  });

  it("renders a returned failed probe as an actionable error with phase detail", async () => {
    vi.mocked(testAiProviderProfile).mockResolvedValue({
      profileId,
      profileRevision: 2,
      credentialEpoch: 1,
      testedModelId: "deepseek/deepseek-v4-flash",
      adapterRevision: 9,
      resultSchemaVersion: 1,
      overallStatus: "failed",
      phases: [
        {
          phase: "text_completion",
          label: "Text completion",
          status: "failed",
          message: "The selected model did not complete the minimal text probe.",
          latencyMs: 10,
          error: {
            code: "ai_text_probe_invalid",
            message: "The model responded, but it did not follow the minimal text probe.",
            retryable: false,
            retryAfterSeconds: null,
          },
        },
      ],
      capabilities: [],
      testedAt: 2,
      persisted: true,
      stale: false,
    });

    render(<DefaultAiModelSettings controller={controller()} />);
    const testButton = screen.getAllByRole("button", { name: "Test model" })[0];
    fireEvent.click(testButton);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Model verification failed.");
    expect(alert.textContent).toContain("Text completion");
    expect(alert.textContent).toContain("did not follow the minimal text probe");
  });

  it("reports a failed document schema contract on the generation model row", async () => {
    vi.mocked(testAiProviderProfile).mockResolvedValue({
      profileId,
      profileRevision: 2,
      credentialEpoch: 1,
      testedModelId: "deepseek/deepseek-v4-flash",
      adapterRevision: 9,
      resultSchemaVersion: 1,
      overallStatus: "partial",
      phases: [
        {
          phase: "document_generation_schema",
          label: "Document generation schema",
          status: "failed",
          message: "The representative document-generation contract did not validate.",
          latencyMs: 10,
          error: {
            code: "ai_document_schema_probe_invalid",
            message: "The provider returned document content that did not satisfy the schema.",
            retryable: false,
            retryAfterSeconds: null,
          },
        },
      ],
      capabilities: [
        {
          key: "document_generation_schema",
          status: "unsupported",
          source: "probed",
          value: null,
        },
      ],
      testedAt: 2,
      persisted: true,
      stale: false,
    });

    render(<DefaultAiModelSettings controller={controller()} />);
    const testButton = screen.getAllByRole("button", { name: "Test model" })[0];
    fireEvent.click(testButton);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("document-generation structured-output contract");
    expect(alert.textContent).toContain("Document generation schema");
  });

  it("contains no obsolete provider/profile implementation guidance", () => {
    render(<DefaultAiModelSettings controller={controller()} />);
    expect(screen.queryByText(/Each profile stores its own reasoning preference/u)).toBeNull();
    expect(screen.queryByText(/Use a separate embedding-model profile/u)).toBeNull();
    expect(screen.queryByText(/request timeout limits initial response/u)).toBeNull();
  });
});
