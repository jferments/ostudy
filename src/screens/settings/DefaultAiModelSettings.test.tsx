import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AppSettings } from "../../ipc/settings";
import { DefaultAiModelSettings } from "./DefaultAiModelSettings";

const profileId = "a".repeat(32);

const settings: AppSettings = {
  theme: "dark",
  uiFont: "plex",
  cardFont: "plex",
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
  aiReasoningEffort: "low",
};

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
    deleteProfile: vi.fn(),
    saveCredential: vi.fn(),
    clearCredential: vi.fn(),
    refreshModels: vi.fn(),
    testProfile: vi.fn(),
    setFlashcardDefault: vi.fn().mockResolvedValue(undefined),
    discardDraft: vi.fn(),
    reload: vi.fn().mockResolvedValue(undefined),
  };
}

describe("DefaultAiModelSettings", () => {
  it("uses the requested Default AI model wording and shows the API model identity", () => {
    render(
      <DefaultAiModelSettings controller={controller()} settings={settings} update={vi.fn()} />,
    );

    expect(screen.getByRole("heading", { name: "Default AI model" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Local nursing model — Qwen-medical" })).toBeTruthy();
    expect(screen.getByText(/initial-response timeout/u)).toBeTruthy();
    const reasoning = screen.getByRole("combobox", {
      name: "Document reasoning effort",
    }) as HTMLSelectElement;
    expect(reasoning.value).toBe("low");
  });

  it("updates the flashcard generation default without provider fallback", () => {
    const value = controller();
    render(<DefaultAiModelSettings controller={value} settings={settings} update={vi.fn()} />);

    fireEvent.change(screen.getByRole("combobox", { name: "Flashcard generation" }), {
      target: { value: "" },
    });
    expect(value.setFlashcardDefault).toHaveBeenCalledWith(null);
    expect(screen.getByText(/no automatic cross-model fallback/u)).toBeTruthy();
  });

  it("updates the saved document reasoning preference independently of the provider profile", () => {
    const update = vi.fn();
    render(<DefaultAiModelSettings controller={controller()} settings={settings} update={update} />);

    fireEvent.change(screen.getByRole("combobox", { name: "Document reasoning effort" }), {
      target: { value: "minimal" },
    });
    expect(update).toHaveBeenCalledWith({ aiReasoningEffort: "minimal" });
  });
});
