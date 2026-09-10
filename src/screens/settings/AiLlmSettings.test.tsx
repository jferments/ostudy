import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AiProfilesController } from "../../controllers/aiProfilesController";
import type { AiProviderProfileDetail } from "../../ipc/aiProfiles";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { AiLlmSettings } from "./AiLlmSettings";

const profileId = "a".repeat(32);

function profile(overrides: Partial<AiProviderProfileDetail> = {}): AiProviderProfileDetail {
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
    revision: 3,
    createdAt: 1,
    updatedAt: 2,
    ...overrides,
  };
}

function controller(selected = profile()): AiProfilesController {
  return {
    catalog: [
      {
        providerKind: selected.providerKind,
        displayName: selected.providerLabel,
        description: "Provider fixture",
        implementationStatus: "available",
        defaultBaseUrl: selected.baseUrl,
        baseUrlEditable: selected.baseUrlEditable,
        authenticationRequired: selected.credentialStatus.authenticationRequired,
        modelIdRequired: true,
        modelDiscoverySupported: true,
        chatTemplatePolicy: "provider_managed",
        defaultTimeoutMs: 60_000,
      },
    ],
    profiles: [{ ...selected }],
    defaults: { flashcardGenerationProfileId: null },
    selectedProfile: selected,
    selectedProfileId: selected.id,
    draft: {
      displayName: selected.displayName,
      providerKind: selected.providerKind,
      baseUrl: selected.baseUrl,
      modelId: "",
      reasoningEffort: "automatic",
      authMode: selected.authMode,
      enabled: selected.enabled,
      allowInsecureHttp: selected.allowInsecureHttp,
      requireZeroDataRetention: selected.requireZeroDataRetention,
      requestTimeoutMs: selected.requestTimeoutMs,
    },
    credentialDraft: "",
    credentialPersistence: "secure",
    models: [
      { id: "one/chat", displayName: "Chat model", ownedBy: "one" },
      { id: "two/embed", displayName: "Embedding model", ownedBy: "two" },
    ],
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
    setTaskDefault: vi.fn().mockResolvedValue(undefined),
    pendingTaskDefault: null,
    retryTaskDefault: vi.fn().mockResolvedValue(undefined),
    discardDraft: vi.fn(),
    reload: vi.fn().mockResolvedValue(undefined),
  };
}

function renderSettings(value: AiProfilesController) {
  render(
    <OverlayProvider>
      <AiLlmSettings controller={value} />
    </OverlayProvider>,
  );
}

describe("AiLlmSettings", () => {
  it("treats this panel as provider/API-key connection management, not model assignment", () => {
    renderSettings(controller());

    expect(screen.getByRole("heading", { name: "AI providers" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add provider" })).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Model" })).toBeNull();
    expect(screen.queryByRole("combobox", { name: "Reasoning effort" })).toBeNull();
    expect(screen.queryByRole("option", { name: "Chat model" })).toBeNull();
    expect(screen.getByLabelText("API key")).toHaveAttribute("type", "password");
  });

  it("keeps connection policy and credentials provider-scoped", () => {
    const value = controller();
    renderSettings(value);
    fireEvent.click(screen.getByText("Advanced settings"));

    expect(screen.getByRole("textbox", { name: "Profile name" })).toHaveValue("OpenRouter");
    expect(screen.getByRole("checkbox", { name: "Enable this provider" })).toBeChecked();
    expect(
      screen.getByRole("checkbox", { name: "Require OpenRouter zero-data-retention routing" }),
    ).not.toBeChecked();
    expect(screen.getByRole("button", { name: "Refresh model catalog" })).toBeTruthy();

    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), {
      target: { value: "OpenRouter 2" },
    });
    expect(value.updateDraft).toHaveBeenCalledWith({ displayName: "OpenRouter 2" });
  });

  it("shows the explicit LAN HTTP opt-in only for remote llama.cpp HTTP endpoints", () => {
    const selected = profile({
      displayName: "LAN llama.cpp",
      providerKind: "llama_cpp",
      providerLabel: "llama.cpp / Unsloth GGUF",
      baseUrl: "http://192.168.1.50:8080/",
      baseUrlEditable: true,
      authMode: "none",
      networkScope: "lan",
      credentialStatus: {
        status: "not_required",
        authenticationRequired: false,
        securePersistenceAvailable: true,
        sessionOnlyAvailable: true,
      },
    });
    const value = controller(selected);
    value.catalog[0].chatTemplatePolicy = "server_model";
    renderSettings(value);
    fireEvent.click(screen.getByText("Advanced settings"));
    const advanced = screen.getByText("Advanced settings").closest("details") as HTMLElement;

    expect(
      within(advanced).getByRole("checkbox", { name: "Allow unencrypted LAN connection" }),
    ).toBeTruthy();
    fireEvent.click(
      within(advanced).getByRole("checkbox", { name: "Allow unencrypted LAN connection" }),
    );
    expect(value.updateDraft).toHaveBeenCalledWith({ allowInsecureHttp: true });
  });
});
