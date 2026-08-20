import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiProviderCatalogEntry, AiProviderProfileDetail } from "../../ipc/aiProfiles";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { AiLlmSettings } from "./AiLlmSettings";

const profileId = "a".repeat(32);

function openRouterCatalog(): AiProviderCatalogEntry[] {
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

function profile(): AiProviderProfileDetail {
  return {
    id: profileId,
    displayName: "OpenRouter",
    providerKind: "openrouter",
    providerLabel: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1/",
    baseUrlEditable: false,
    modelId: "provider/model-000",
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
    revision: 3,
    createdAt: 1,
    updatedAt: 2,
  };
}

function controller(): AiProfilesController {
  const selected = profile();
  const models = Array.from({ length: 411 }, (_, index) => ({
    id: `provider/model-${String(index).padStart(3, "0")}`,
    displayName: `Provider Model ${String(index).padStart(3, "0")}`,
    ownedBy: "provider",
  }));
  models.push({
    id: "zeta/final-model",
    displayName: "Zeta Final Model",
    ownedBy: "zeta",
  });

  return {
    catalog: openRouterCatalog(),
    profiles: [],
    defaults: { flashcardGenerationProfileId: null },
    selectedProfile: selected,
    selectedProfileId: selected.id,
    draft: {
      displayName: selected.displayName,
      providerKind: selected.providerKind,
      baseUrl: selected.baseUrl,
      modelId: selected.modelId,
      authMode: selected.authMode,
      enabled: selected.enabled,
      allowInsecureHttp: selected.allowInsecureHttp,
      requireZeroDataRetention: selected.requireZeroDataRetention,
      requestTimeoutMs: selected.requestTimeoutMs,
    },
    credentialDraft: "",
    credentialPersistence: "secure",
    models,
    modelsLoaded: true,
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
    saveProfile: vi.fn().mockResolvedValue(undefined),
    connectSetup: vi.fn().mockResolvedValue(undefined),
    deleteProfile: vi.fn().mockResolvedValue(undefined),
    saveCredential: vi.fn().mockResolvedValue(undefined),
    clearCredential: vi.fn().mockResolvedValue(undefined),
    refreshModels: vi.fn().mockResolvedValue(undefined),
    testProfile: vi.fn().mockResolvedValue(undefined),
    setFlashcardDefault: vi.fn().mockResolvedValue(undefined),
    discardDraft: vi.fn(),
    reload: vi.fn().mockResolvedValue(undefined),
  };
}

describe("AiLlmSettings", () => {
  it("renders every model returned by a large provider catalog", () => {
    render(
      <OverlayProvider>
        <AiLlmSettings controller={controller()} />
      </OverlayProvider>,
    );

    expect(screen.getByRole("option", { name: "Zeta Final Model" })).toBeTruthy();
    const modelSelect = screen.getByRole("combobox", { name: "Model" });
    expect(modelSelect.querySelectorAll("option")).toHaveLength(413);
  });
});