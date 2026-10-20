import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../../test/factories";
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
    saveProfile: vi.fn().mockResolvedValue(true),
    connectSetup: vi.fn().mockResolvedValue(true),
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

async function openService(name = "OpenRouter") {
  fireEvent.click(screen.getByRole("button", { name: new RegExp(`^${name}`, "u") }));
  return screen.findByRole("dialog");
}

describe("AiLlmSettings", () => {
  it("lists services with their status and keeps model assignment out of the service sheet", async () => {
    renderSettings(controller());

    expect(screen.getByRole("heading", { name: "Services" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add a service" })).toBeTruthy();
    const list = screen.getByRole("list", { name: "Services" });
    expect(within(list).getByText("Key saved")).toBeTruthy();
    expect(within(list).getByText("Ready")).toBeTruthy();

    const sheet = await openService();
    expect(within(sheet).queryByRole("combobox", { name: "Model" })).toBeNull();
    expect(within(sheet).queryByRole("option", { name: "Chat model" })).toBeNull();
    expect((within(sheet).getByLabelText("API key") as HTMLInputElement).type).toBe("password");
  });

  it("keeps connection policy and credentials service-scoped", async () => {
    const value = controller();
    renderSettings(value);
    const sheet = await openService();
    expect(value.selectProfile).toHaveBeenCalledWith(profileId);

    expect((within(sheet).getByRole("textbox", { name: "Name" }) as HTMLInputElement).value).toBe("OpenRouter");
    fireEvent.click(within(sheet).getByText("More options"));
    expect(within(sheet).getByRole("switch", { name: "Use this service" }).getAttribute("aria-checked")).toBe("true");
    const zdr = within(sheet).getByRole("switch", { name: "Only zero-data-retention routes" });
    expect(zdr.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(zdr);
    expect(value.updateDraft).toHaveBeenCalledWith({ requireZeroDataRetention: true });
    fireEvent.click(within(sheet).getByRole("switch", { name: "Keep the key on this device" }));
    expect(value.setCredentialPersistence).toHaveBeenCalledWith("session");
    expect(within(sheet).getByRole("button", { name: "Refresh models" })).toBeTruthy();

    fireEvent.change(within(sheet).getByRole("textbox", { name: "Name" }), { target: { value: "OpenRouter 2" } });
    expect(value.updateDraft).toHaveBeenCalledWith({ displayName: "OpenRouter 2" });
  });

  it("adds a service from the sheet and closes it once connected", async () => {
    const value = controller();
    value.selectedProfile = null;
    value.credentialDraft = "sk-test";
    renderSettings(value);
    fireEvent.click(screen.getByRole("button", { name: "Add a service" }));
    expect(value.beginCreate).toHaveBeenCalledWith("openrouter");
    const sheet = await screen.findByRole("dialog", { name: "Add a service" });
    expect(within(sheet).getByRole("radio", { name: /^OpenRouter/u })).toBeTruthy();
    fireEvent.click(within(sheet).getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(value.connectSetup).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add a service" })).toBeNull());
  });

  it("removes a service from its menu after confirmation", async () => {
    const value = controller();
    renderSettings(value);
    fireEvent.click(screen.getByRole("button", { name: "More for OpenRouter" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
    fireEvent.click(await screen.findByRole("button", { name: "Remove service" }));
    await waitFor(() => expect(value.deleteProfile).toHaveBeenCalledWith(expect.objectContaining({ id: profileId })));
  });

  it("shows the explicit LAN HTTP opt-in only for remote llama.cpp HTTP endpoints", async () => {
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
    required(value.catalog[0], "catalog profile").chatTemplatePolicy = "server_model";
    renderSettings(value);
    const sheet = await openService("LAN llama.cpp");
    const trigger = within(sheet).getByRole("button", { name: "More options" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(trigger);
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    const optIn = within(sheet).getByRole("switch", { name: "Allow an unencrypted network connection" });
    expect(optIn.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(optIn);
    expect(value.updateDraft).toHaveBeenCalledWith({ allowInsecureHttp: true });
  });
});
