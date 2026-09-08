import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import { DefaultAiModelSettings } from "./DefaultAiModelSettings";

const profileId = "a".repeat(32);

function controller(): AiProfilesController {
  return {
    catalog: [],
    profiles: [
      {
        id: profileId,
        displayName: "Local nursing model",
        providerKind: "llama_cpp",
        providerLabel: "llama.cpp / Unsloth GGUF",
        modelId: "Qwen-medical",
        reasoningEffort: "high",
        enabled: true,
        networkScope: "local",
        credentialStatus: {
          status: "not_required",
          authenticationRequired: false,
          securePersistenceAvailable: true,
          sessionOnlyAvailable: true,
        },
        configurationStatus: "ready",
        revision: 1,
        updatedAt: 1,
      },
    ],
    defaults: { flashcardGenerationProfileId: profileId },
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

describe("DefaultAiModelSettings", () => {
  it("uses the requested Default AI model wording and shows the model identity", () => {
    render(<DefaultAiModelSettings controller={controller()} />);

    expect(screen.getByRole("heading", { name: "Default AI model" })).toBeTruthy();
    expect(
      screen.getAllByRole("option", { name: "Local nursing model — Qwen-medical" }),
    ).toHaveLength(3);
    expect(screen.getByText(/profile's request timeout/u)).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Document reasoning effort" })).toBeNull();
    expect(screen.getByText(/Each profile stores its own reasoning preference/u)).toBeTruthy();
  });

  it("updates the generation default without copying profile settings", () => {
    const value = controller();
    render(<DefaultAiModelSettings controller={value} />);

    fireEvent.change(
      screen.getByRole("combobox", { name: "Flashcard and study-guide generation" }),
      { target: { value: "" } },
    );
    expect(value.setFlashcardDefault).toHaveBeenCalledWith(null);
    expect(screen.getByText(/does not copy settings between models/u)).toBeTruthy();
  });
  it("selects tutor and embedding defaults without replacing generation", () => {
    const value = controller();
    render(<DefaultAiModelSettings controller={value} />);
    fireEvent.change(screen.getByRole("combobox", { name: "Interactive tutor" }), { target: { value: profileId } });
    fireEvent.change(screen.getByRole("combobox", { name: "Semantic embeddings" }), { target: { value: profileId } });
    expect(value.setTaskDefault).toHaveBeenNthCalledWith(1, "tutor", profileId);
    expect(value.setTaskDefault).toHaveBeenNthCalledWith(2, "embeddings", profileId);
    expect(value.setFlashcardDefault).not.toHaveBeenCalled();
  });

});
